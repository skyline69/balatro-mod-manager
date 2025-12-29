mod bmi;
pub mod commands;
mod lfs;
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
        .plugin(tauri_plugin_os::init())
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
        .plugin(tauri_plugin_opener::init())
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

            // Ensure the Mods directory exists so installs and detection work on fresh setups.
            if let Some(cfg_dir) = dirs::config_dir() {
                let mods_dir = cfg_dir.join("Balatro").join("Mods");
                if let Err(e) = std::fs::create_dir_all(&mods_dir) {
                    log::warn!(
                        "Failed to create mods directory at {}: {}",
                        mods_dir.display(),
                        e
                    );
                }
            } else {
                log::warn!("Could not resolve config directory to create Mods folder");
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
                            if let Ok(meta) = path.metadata()
                                && let Ok(mtime) = meta.modified()
                                && let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH)
                            {
                                sum = sum.wrapping_mul(1099511628211).wrapping_add(dur.as_secs());
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

            // Ensure the main window is visible/focused on startup even if a saved state
            // or window manager starts it hidden/minimized.
            match app.get_webview_window("main") {
                Some(window) => {
                    if let Err(e) = window.show() {
                        log::warn!("Failed to show main window: {e}");
                    }
                    if let Err(e) = window.unminimize() {
                        log::warn!("Failed to unminimize main window: {e}");
                    }
                    if let Err(e) = window.set_focus() {
                        log::warn!("Failed to focus main window: {e}");
                    }
                    #[cfg(target_os = "linux")]
                    clamp_window_to_monitor(&window);
                }
                None => log::warn!("Main window not found during setup; UI may remain hidden"),
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
            commands::repo::list_repo_mods,
            commands::repo::get_repo_file,
            commands::repo::get_repo_thumbnail_url,
            commands::repo::fetch_repo_mods,
            commands::repo::get_cached_installed_thumbnail,
            commands::repo::get_cached_thumbnail_by_title,
            commands::repo::get_cached_thumbnails_map,
            commands::repo::cache_thumbnail_from_url,
            commands::repo::get_description_cached_or_remote,
            commands::repo::get_cached_description_by_title,
            commands::repo::batch_fetch_thumbnails_repo,
            commands::thumbnails::enqueue_thumbnails,
            commands::thumbnails::enqueue_thumbnail,
            commands::report::submit_report,
            commands::report::get_latest_log,
            commands::mods::is_mod_enabled,
            commands::mods::toggle_mod_enabled,
            commands::mods::is_mod_enabled_by_path,
            commands::mods::toggle_mod_enabled_by_path,
            commands::mods::enabled_state_map,
            commands::cache::mod_update_available,
            commands::cache::mods_updates_map,
            commands::cache::mods_state_summary,
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
            commands::external::open_external_url,
            exit_application
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        log::error!("Failed to run application: {e}");
        log::logger().flush();
        std::process::exit(1);
    }
}

#[cfg(target_os = "linux")]
fn clamp_window_to_monitor(window: &tauri::WebviewWindow) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let monitor_size = monitor.size();
    let padding = 48u32;
    let max_w = monitor_size.width.saturating_sub(padding);
    let max_h = monitor_size.height.saturating_sub(padding);
    let Ok(window_size) = window.outer_size() else {
        return;
    };
    if window_size.width <= max_w && window_size.height <= max_h {
        return;
    }
    let new_size = tauri::PhysicalSize::new(max_w, max_h);
    if let Err(e) = window.set_size(tauri::Size::Physical(new_size)) {
        log::warn!("Failed to clamp window size: {e}");
        return;
    }
    let monitor_pos = monitor.position();
    let centered_x = monitor_pos.x + ((monitor_size.width - max_w) / 2) as i32;
    let centered_y = monitor_pos.y + ((monitor_size.height - max_h) / 2) as i32;
    let new_pos = tauri::PhysicalPosition::new(centered_x, centered_y);
    if let Err(e) = window.set_position(tauri::Position::Physical(new_pos)) {
        log::warn!("Failed to clamp window position: {e}");
    }
}
