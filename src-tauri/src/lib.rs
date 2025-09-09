pub mod commands;
mod models;
mod state;
mod thumb_queue;
mod util;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Emitter, Manager};
use tauri_plugin_window_state::StateFlags;

use bmm_lib::{
    database::Database, discord_rpc::DiscordRpcManager, errors::AppError, local_mod_detection,
};

use crate::models::Payload;
use crate::state::AppState;
use crate::util::map_error;

#[tauri::command]
fn exit_application(app_handle: tauri::AppHandle) {
    app_handle.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            app.emit("single-instance", Payload { args: argv, cwd })
                .unwrap();
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_prevent_default::init())
        .setup(|app| {
            let db = map_error(Database::new())?;
            let discord_rpc = DiscordRpcManager::new();
            let discord_rpc_enabled = db.is_discord_rpc_enabled().unwrap_or(true);
            discord_rpc.set_enabled(discord_rpc_enabled);
            app.manage(AppState {
                db: Mutex::new(db),
                discord_rpc: Mutex::new(discord_rpc),
                thumbs: crate::thumb_queue::ThumbnailManager::new(),
            });

            // Remove legacy GitHub-based local clone directory if it exists.
            if let Some(cfg_dir) = dirs::config_dir() {
                let legacy_repo = cfg_dir.join("Balatro").join("mod_index");
                if legacy_repo.exists() {
                    match std::fs::remove_dir_all(&legacy_repo) {
                        Ok(()) => log::info!(
                            "Removed legacy GitHub repo directory: {}",
                            legacy_repo.display()
                        ),
                        Err(e) => log::warn!(
                            "Failed to remove legacy repo directory {}: {}",
                            legacy_repo.display(),
                            e
                        ),
                    }
                }
            }

            tauri::async_runtime::spawn(async move {
                let db = match Database::new() {
                    Ok(db) => db,
                    Err(e) => {
                        log::warn!("Lovely check: failed to open DB: {e}");
                        return;
                    }
                };
                match db.get_lovely_version() {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => {
                        log::info!(
                            "lovely_version missing; UI will prompt to install/update Lovely"
                        );
                    }
                }
            });

            // Periodically validate the mod database in a very cheap, incremental sweep.
            // Runs every few seconds and checks only a small batch of entries each tick.
            // Clone a handle that is 'static so we can emit events from the background task.
            let handle_for_events = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                use std::time::Duration;
                use tokio::time::sleep;

                const REINDEX_TICK_SECS: u64 = 1; // 1s polling for quick updates
                const REINDEX_BATCH_SIZE: usize = 5; // small batch to keep cost negligible

                // Snapshot of installed mods to sweep over between refreshes
                let mut snapshot: Vec<(String, String)> = Vec::new(); // (name, path)
                let mut cursor_idx: usize = 0;

                // Open a dedicated DB connection for this background task.
                // Using a separate connection avoids borrowing app state and remains lightweight.
                let db = match Database::new() {
                    Ok(db) => db,
                    Err(e) => {
                        log::warn!("Auto reindex: failed to open DB: {}", e);
                        return;
                    }
                };

                // Lightweight fingerprint of Mods directory to detect additions/removals
                fn mods_dir_fingerprint() -> Option<u64> {
                    let config_dir = dirs::config_dir()?;
                    let mods_dir = config_dir.join("Balatro").join("Mods");
                    if !mods_dir.exists() {
                        return Some(0);
                    }
                    let mut sum: u64 = 1469598103934665603; // FNV offset basis
                    let rd = std::fs::read_dir(&mods_dir).ok()?;
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if !path.is_dir() {
                            continue;
                        }
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let lower = name.to_lowercase();
                            if lower.contains("lovely")
                                || lower.starts_with('.')
                                || matches!(lower.as_str(), ".git" | "node_modules" | "__macosx")
                            {
                                continue;
                            }
                            for b in lower.as_bytes() {
                                sum = sum.wrapping_mul(1099511628211).wrapping_add(*b as u64);
                            }
                            if let Ok(meta) = path.metadata() {
                                if let Ok(mtime) = meta.modified() {
                                    if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
                                        sum = sum
                                            .wrapping_mul(1099511628211)
                                            .wrapping_add(dur.as_secs());
                                    }
                                }
                            }
                        }
                    }
                    Some(sum)
                }
                let mut last_fp = mods_dir_fingerprint();

                loop {
                    sleep(Duration::from_secs(REINDEX_TICK_SECS)).await;

                    // Refresh snapshot when exhausted or empty
                    if cursor_idx >= snapshot.len() {
                        let mods: Vec<(String, String)> = match db
                            .get_installed_mods()
                            .map(|v| v.into_iter().map(|m| (m.name, m.path)).collect())
                        {
                            Ok(v) => v,
                            Err(e) => {
                                log::warn!("Auto reindex: failed to load mods: {}", e);
                                continue;
                            }
                        };
                        snapshot = mods;
                        cursor_idx = 0;
                    }

                    if snapshot.is_empty() {
                        continue; // nothing to do
                    }

                    let end = (cursor_idx + REINDEX_BATCH_SIZE).min(snapshot.len());
                    let mut cleaned = 0usize;
                    for (name, path) in &snapshot[cursor_idx..end] {
                        if !std::path::Path::new(path).exists() {
                            // Remove missing entry from DB
                            match db.remove_installed_mod(name) {
                                Ok(()) => {
                                    cleaned += 1;
                                }
                                Err(e) => {
                                    log::warn!("Auto reindex: failed to remove '{}': {}", name, e)
                                }
                            }
                        }
                    }
                    cursor_idx = end;

                    // Detect Mods dir fingerprint changes (additions/removals/renames)
                    let cur_fp = mods_dir_fingerprint();
                    let fp_changed = cur_fp.is_some() && cur_fp != last_fp;
                    if fp_changed {
                        last_fp = cur_fp;
                        // Clear cache so next detection reflects changes and notify UI
                        local_mod_detection::clear_detection_cache();
                        let _ = handle_for_events.emit("installed-mods-changed", ());
                    }

                    if cleaned > 0 {
                        // Clear detection cache so next detection reflects changes
                        local_mod_detection::clear_detection_cache();
                        log::info!(
                            "Auto reindex: cleaned {} database entr{} (batch)",
                            cleaned,
                            if cleaned == 1 { "y" } else { "ies" }
                        );

                        // Notify UI to refresh installed mods in real-time
                        // Ignore errors if there are no listeners
                        let _ = handle_for_events.emit("installed-mods-changed", ());
                    }
                }
            });

            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|_| AppError::DirNotFound(PathBuf::from("app data directory")))?;
            std::fs::create_dir_all(&app_dir).map_err(|e| AppError::DirCreate {
                path: app_dir.clone(),
                source: e.to_string(),
            })?;
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::paths::find_steam_balatro,
            commands::paths::check_custom_balatro,
            commands::paths::check_existing_installation,
            commands::paths::get_balatro_path,
            commands::paths::set_balatro_path,
            commands::paths::get_mods_folder,
            commands::paths::open_directory,
            commands::install::launch_balatro,
            commands::system::check_steam_running,
            commands::system::check_balatro_running,
            commands::system::get_app_version,
            commands::install::get_installed_mods_from_db,
            commands::install::install_mod,
            commands::install::add_installed_mod,
            commands::install::remove_installed_mod,
            commands::install::get_steamodded_versions,
            commands::install::install_steamodded_version,
            commands::install::install_talisman_version,
            commands::install::get_talisman_versions,
            commands::install::get_latest_steamodded_release,
            commands::paths::verify_path_exists,
            commands::paths::path_exists,
            commands::detection::check_mod_installation,
            commands::detection::refresh_mods_folder,
            commands::cache::save_mods_cache,
            commands::cache::load_mods_cache,
            commands::cache::clear_cache,
            commands::cache::save_versions_cache,
            commands::cache::load_versions_cache,
            commands::settings::get_lovely_console_status,
            commands::settings::set_lovely_console_status,
            commands::lovely::check_lovely_update,
            commands::lovely::update_lovely_to_latest,
            commands::lovely::is_lovely_installed,
            commands::settings::get_background_state,
            commands::settings::set_background_state,
            commands::settings::get_discord_rpc_status,
            commands::settings::set_discord_rpc_status,
            commands::settings::set_security_warning_acknowledged,
            commands::settings::is_security_warning_acknowledged,
            commands::cache::get_last_fetched,
            commands::cache::update_last_fetched,
            commands::repo::list_gitlab_mods,
            commands::repo::get_gitlab_file,
            commands::repo::get_gitlab_thumbnail_url,
            commands::repo::fetch_gitlab_mods_archive,
            commands::repo::fetch_gitlab_mods,
            commands::repo::fetch_gitlab_mods_meta_only,
            commands::repo::get_cached_installed_thumbnail,
            commands::repo::get_cached_thumbnail_by_title,
            commands::repo::cache_thumbnail_from_url,
            commands::repo::get_description_cached_or_remote,
            commands::repo::get_cached_description_by_title,
            commands::repo::batch_fetch_thumbnails_lfs,
            commands::thumbnails::enqueue_thumbnails,
            commands::thumbnails::enqueue_thumbnail,
            commands::report::submit_report,
            commands::report::get_latest_log,
            // Profiles
            commands::profiles::list_installed_mod_dirs,
            commands::profiles::list_profiles,
            commands::profiles::create_profile,
            commands::profiles::delete_profile,
            commands::profiles::rename_profile,
            commands::profiles::set_profile_mods,
            commands::profiles::get_active_profile_id,
            commands::profiles::set_active_profile_id,
            commands::profiles::apply_profile,
            commands::profiles::clear_active_profile,
            commands::profiles::apply_active_profile_if_set,
            commands::mods::is_mod_enabled,
            commands::mods::toggle_mod_enabled,
            commands::mods::is_mod_enabled_by_path,
            commands::mods::toggle_mod_enabled_by_path,
            commands::cache::mod_update_available,
            commands::install::cascade_uninstall,
            commands::install::force_remove_mod,
            commands::install::get_dependents,
            commands::import::process_dropped_file,
            commands::import::process_mod_archive,
            commands::detection::get_detected_local_mods,
            commands::detection::reindex_mods,
            commands::detection::delete_manual_mod,
            commands::detection::backup_local_mod,
            commands::detection::restore_from_backup,
            commands::detection::remove_backup,
            exit_application
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        log::error!("Failed to run application: {e}");
        log::logger().flush();
        std::process::exit(1);
    }
}
