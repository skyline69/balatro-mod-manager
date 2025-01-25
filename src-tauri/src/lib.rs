use std::panic;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

use crate::lovely::ensure_lovely_exists;
use bmm_lib::balamod::find_balatros;
use bmm_lib::cache;
use bmm_lib::cache::Mod;
use bmm_lib::database::Database;
use bmm_lib::database::InstalledMod;
use bmm_lib::finder::is_balatro_running;
use bmm_lib::finder::is_steam_running;
use bmm_lib::lovely;
use bmm_lib::smods_installer::{ModInstaller, ModType};
use std::time::UNIX_EPOCH;
use std::time::SystemTime;

use std::process::Command;

use tauri::Manager;

// Create a state s;tructure to hold the database
struct AppState {
    db: Mutex<Database>,
}

#[tauri::command]
async fn check_steam_running() -> bool {
    is_steam_running()
}

#[tauri::command]
async fn check_balatro_running() -> bool {
    is_balatro_running()
}

#[tauri::command]
async fn save_versions_cache(mod_type: String, versions: Vec<String>) -> Result<(), String> {
    cache::save_versions_cache(&mod_type, &versions).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_versions_cache(mod_type: String) -> Result<Option<(Vec<String>, u64)>, String> {
    cache::load_versions_cache(&mod_type)
        .map(|res| {
            res.map(|versions| {
                (
                    versions,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                )
            })
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_mods_cache(mods: Vec<Mod>) -> Result<(), String> {
    cache::save_cache(&mods).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_mods_cache() -> Result<Option<(Vec<Mod>, u64)>, String> {
    cache::load_cache().map_err(|e| e.to_string())
}

#[tauri::command]
async fn refresh_mods_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mod_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("Balatro")
        .join("Mods");

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let installed_mods = db.get_installed_mods().map_err(|e| e.to_string())?;

    let entries = std::fs::read_dir(&mod_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid name")?;

        // Skip .lovely and lovely
        if name.contains(".lovely") || name.contains("lovely") {
            continue;
        }

        match entry.file_type().map_err(|e| e.to_string())? {
            // Handle directories
            ft if ft.is_dir() => {
                let mod_exists = installed_mods.iter().any(|m| m.path.contains(name));
                if !mod_exists {
                    log::info!("Removing untracked mod directory: {}", name);
                    std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
                }
            }
            // Handle files
            ft if ft.is_file() => {
                let mod_exists = installed_mods.iter().any(|m| m.path.contains(name));
                if !mod_exists {
                    log::info!("Removing untracked mod file: {}", name);
                    std::fs::remove_file(&path).map_err(|e| e.to_string())?;
                }
            }
            // Skip other types (symlinks, etc.)
            _ => continue,
        }
    }

    Ok(())
}

// Add launch command
#[tauri::command]
async fn launch_balatro(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let path_str = db
        .get_installation_path()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No Balatro installation path set".to_string())?;

    let path = PathBuf::from(&path_str);

    #[cfg(target_os = "macos")]
    {
        let lovely_path = ensure_lovely_exists()?;
        let balatro_exe = path.join("Balatro.app/Contents/MacOS/love");

        let mut command = Command::new(&balatro_exe);
        command.env("DYLD_INSERT_LIBRARIES", &lovely_path);
        command.current_dir(&path);

        command
            .spawn()
            .map_err(|e| format!("Failed to launch Balatro: {}", e))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut command = Command::new(&path);
        command
            .spawn()
            .map_err(|e| format!("Failed to launch Balatro: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
async fn check_mod_installation(mod_type: String) -> Result<bool, String> {
    let db = Database::new().map_err(|e| e.to_string())?;
    let installed_mods = db.get_installed_mods().map_err(|e| e.to_string())?;

    match mod_type.as_str() {
        "Steamodded" => Ok(installed_mods.iter().any(|m| m.name == "Steamodded")),
        "Talisman" => Ok(installed_mods.iter().any(|m| m.name == "Talisman")),
        _ => Err("Invalid mod type".to_string()),
    }
}

#[tauri::command]
async fn check_existing_installation(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(path) = db.get_installation_path().map_err(|e| e.to_string())? {
        // Verify the path still exists and is valid
        let path_buf = PathBuf::from(&path);
        if bmm_lib::balamod::Balatro::from_custom_path(path_buf).is_some() {
            return Ok(Some(path));
        } else {
            db.remove_installation_path().map_err(|e| e.to_string())?;
        }
    }
    Ok(None)
}
#[tauri::command]
async fn install_mod(url: String) -> Result<PathBuf, String> {
    bmm_lib::installer::install_mod(url).await
}
#[tauri::command]
async fn get_installed_mods_from_db(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InstalledMod>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_installed_mods().map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_installed_mod(&name, &path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    bmm_lib::installer::uninstall_mod(PathBuf::from(path));
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_installed_mod(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_balatro_path(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_installation_path().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_balatro_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.set_installation_path(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn find_steam_balatro(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let balatros = find_balatros();
    if let Some(path) = balatros.first() {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_installation_path(&path.path.to_string_lossy())
            .map_err(|e| e.to_string())?;
    }

    Ok(balatros
        .iter()
        .map(|b| b.path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
async fn get_steamodded_versions() -> Result<Vec<String>, String> {
    let installer = ModInstaller::new(ModType::Steamodded);
    installer
        .get_available_versions()
        .await
        .map(|versions| versions.into_iter().map(|v| v.to_string()).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_steamodded_version(version: String) -> Result<String, String> {
    let installer = ModInstaller::new(ModType::Steamodded);
    installer
        .install_version(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_talisman_versions() -> Result<Vec<String>, String> {
    let installer = ModInstaller::new(ModType::Talisman);
    installer
        .get_available_versions()
        .await
        .map(|versions| versions.into_iter().map(|v| v.to_string()).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_talisman_version(version: String) -> Result<String, String> {
    let installer = ModInstaller::new(ModType::Talisman);
    installer
        .install_version(&version)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn verify_path_exists(path: String) -> bool {
    std::fs::exists(PathBuf::from(path)).unwrap()
}

#[tauri::command]
async fn check_custom_balatro(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<bool, String> {
    let path = PathBuf::from(&path);
    let is_valid = bmm_lib::balamod::Balatro::from_custom_path(path.clone()).is_some();

    if is_valid {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.set_installation_path(&path.to_string_lossy())
            .map_err(|e| e.to_string())?;
    }

    Ok(is_valid)
}

#[tauri::command]
async fn open_image_popup(app: tauri::AppHandle, image_url: String, title: String) {
    let _popup = WebviewWindowBuilder::new(
        &app,
        "image_popup",
        WebviewUrl::App(format!("image-popup.html?image={}", image_url).into()),
    )
    .title(title)
    .inner_size(800.0, 600.0)
    .center()
    .build()
    .unwrap();
}

// #[tauri::command]
// async fn get_installed_mods() -> Vec<String> {
//     bmm_lib::finder::get_installed_mods()
// }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up panic hook with proper logging
    panic::set_hook(Box::new(|panic_info| {
        log::error!("Application crashed: {:?}", panic_info);
    }));

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Get app handle first
            let app_handle = app.handle();

            // Initialize database with error handling
            let db =
                Database::new().map_err(|e| format!("Failed to initialize database: {}", e))?;
            app.manage(AppState { db: Mutex::new(db) });

            // Create required directories using path resolver
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|e| format!("Failed to get app data directory: {}", e))?;

            std::fs::create_dir_all(&app_dir)
                .map_err(|e| format!("Failed to create app directory: {}", e))?;

            #[cfg(target_os = "macos")]
            {
                lovely::ensure_lovely_exists()
                    .map_err(|e| format!("Failed to setup lovely: {}", e))?;
            }

            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            find_steam_balatro,
            check_custom_balatro,
            check_existing_installation,
            get_balatro_path,
            set_balatro_path,
            launch_balatro,
            check_steam_running,
            check_balatro_running,
            open_image_popup,
            get_installed_mods_from_db,
            install_mod,
            add_installed_mod,
            remove_installed_mod,
            get_steamodded_versions,
            install_steamodded_version,
            install_talisman_version,
            get_talisman_versions,
            verify_path_exists,
            check_mod_installation,
            refresh_mods_folder,
            save_mods_cache,
            load_mods_cache,
            save_versions_cache,
            load_versions_cache
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("Failed to run application: {}", e);
        std::process::exit(1);
    }
}
