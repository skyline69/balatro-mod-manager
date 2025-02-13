use std::panic;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::WebviewUrl;
use tauri::WebviewWindowBuilder;

#[cfg(not(target_os = "windows"))]
use crate::lovely::ensure_lovely_exists;

use bmm_lib::balamod::find_balatros;
use bmm_lib::cache;
use bmm_lib::cache::Mod;
use bmm_lib::database::Database;
use bmm_lib::database::InstalledMod;
use bmm_lib::errors::AppError;
use bmm_lib::finder::is_balatro_running;
use bmm_lib::finder::is_steam_running;
use bmm_lib::lovely;
use bmm_lib::smods_installer::{ModInstaller, ModType};
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use std::process::Command;

use tauri::Manager;

fn map_error<T>(result: Result<T, AppError>) -> Result<T, String> {
    result.map_err(|e| e.to_string())
}

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
    map_error(cache::save_versions_cache(&mod_type, &versions))
}

#[tauri::command]
async fn load_versions_cache(mod_type: String) -> Result<Option<(Vec<String>, u64)>, String> {
    cache::load_versions_cache(&mod_type)
        .map(|res| {
            res.map(|versions| {
                (
                    versions,
                    match SystemTime::now().duration_since(UNIX_EPOCH) {
                        Ok(dur) => dur,
                        Err(e) => {
                            log::error!("Failed to get current time: {}", e);
                            std::process::exit(1);
                        }
                    }
                    .as_secs(),
                )
            })
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_mods_cache(mods: Vec<Mod>) -> Result<(), String> {
    map_error(cache::save_cache(&mods))
}

#[tauri::command]
async fn clear_cache() -> Result<(), String> {
    map_error(cache::clear_cache())
}

#[tauri::command]
async fn load_mods_cache() -> Result<Option<(Vec<Mod>, u64)>, String> {
    map_error(cache::load_cache())
}

#[tauri::command]
async fn get_lovely_console_status(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.is_lovely_console_enabled())
}

#[tauri::command]
async fn set_lovely_console_status(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.set_lovely_console_status(enabled))
}

#[tauri::command]
async fn refresh_mods_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mod_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?
        .join("Balatro")
        .join("Mods");

    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    let installed_mods = db.get_installed_mods()?;

    let entries = std::fs::read_dir(&mod_dir).map_err(|e| AppError::FileRead {
        path: mod_dir.clone(),
        source: e.to_string(),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| AppError::FileRead {
            path: mod_dir.clone(),
            source: e.to_string(),
        })?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::InvalidState("Invalid filename".to_string()))?;

        if name.contains(".lovely") || name.contains("lovely") {
            continue;
        }

        let ft = entry.file_type().map_err(|e| AppError::FileRead {
            path: path.clone(),
            source: e.to_string(),
        })?;

        match (ft.is_dir(), ft.is_file()) {
            (true, _) => {
                if !installed_mods.iter().any(|m| m.path.contains(name)) {
                    std::fs::remove_dir_all(&path).map_err(|e| AppError::FileWrite {
                        path: path.clone(),
                        source: e.to_string(),
                    })?;
                }
            }
            (_, true) => {
                if !installed_mods.iter().any(|m| m.path.contains(name)) {
                    std::fs::remove_file(&path).map_err(|e| AppError::FileWrite {
                        path: path.clone(),
                        source: e.to_string(),
                    })?;
                }
            }
            _ => continue,
        }
    }
    Ok(())
}

#[tauri::command]
async fn launch_balatro(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    let path_str = db
        .get_installation_path()?
        .ok_or_else(|| AppError::InvalidState("No installation path set".to_string()))?;
    let lovely_console_enabled = db.is_lovely_console_enabled()?;
    let path = PathBuf::from(path_str);

    #[cfg(target_os = "macos")]
    {
        let lovely_path = map_error(ensure_lovely_exists())?;
        let balatro_executable = path.join("Balatro.app/Contents/MacOS/love");

        if lovely_console_enabled {
            // If the console is disabled, add the flag
            let disable_arg = if !lovely_console_enabled {
                " --disable-console"
            } else {
                ""
            };
            // Instead of using double quotes which cause conflicts in AppleScript,
            // wrap the file paths in single quotes.
            let command_line = format!(
                "cd '{}' && DYLD_INSERT_LIBRARIES='{}' '{}'{}",
                path.display(),
                lovely_path.display(),
                balatro_executable.display(),
                disable_arg
            );

            // Construct the AppleScript command to run the command_line in Terminal.
            let applescript = format!(
                "tell application \"Terminal\" to do script \"{}\"",
                command_line
            );

            Command::new("osascript")
                .arg("-e")
                .arg(applescript)
                .spawn()
                .map_err(|e| AppError::ProcessExecution(e.to_string()))?;
        } else {
            let mut command = Command::new(path.join("Balatro.app/Contents/MacOS/love"));
            command
                .env("DYLD_INSERT_LIBRARIES", lovely_path)
                .current_dir(&path);
            command
                .spawn()
                .map_err(|e| AppError::ProcessExecution(e.to_string()))?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Paths for the executable and the version DLL.
        let exe_path = path.join("Balatro.exe");
        let dll_path = path.join("version.dll");

        // At launch, if a version.dll doesn't exist in the game directory,create it.
        if !dll_path.exists() {
            // Write the embedded version.dll (replace lovely::EMBEDDED_DLL with your DLL's data) to the game folder.
            std::fs::write(&dll_path, lovely::EMBEDDED_DLL)
                .map_err(|e| format!("Failed to write version.dll: {}", e))?;
            log::debug!("Written version.dll to {}", dll_path.display());
        }

        let mut child: std::process::Child;

        // Launch the game normally.
        if lovely_console_enabled {
            child = Command::new(&exe_path)
                .current_dir(&path)
                .spawn()
                .map_err(|e| format!("Failed to launch Balatro.exe: {}", e))?;
        } else {
            child = Command::new(&exe_path)
                .current_dir(&path)
                .arg("--disable-console")
                .spawn()
                .map_err(|e| format!("Failed to launch Balatro.exe: {}", e))?;
        }

        log::debug!("Launched Balatro from {}", exe_path.display());

        // Spawn a background thread that waits for the game process to exit.
        // Once the game is closed, remove the version.dll file.
        std::thread::spawn(move || match child.wait() {
            Ok(status) => {
                log::debug!("Balatro exited with status: {:?}", status);
                if let Err(e) = std::fs::remove_file(&dll_path) {
                    log::error!(
                        "Failed to remove version.dll after game exit at {}: {}",
                        dll_path.display(),
                        e
                    );
                } else {
                    log::debug!("Removed version.dll after game exit");
                }
            }
            Err(e) => {
                log::error!("Error waiting for Balatro process: {}", e);
            }
        });
    }

    Ok(())
}

#[tauri::command]
async fn check_mod_installation(mod_type: String) -> Result<bool, String> {
    let db = map_error(Database::new())?;
    let installed_mods = map_error(db.get_installed_mods())?;

    Ok(match mod_type.as_str() {
        "Steamodded" => installed_mods.iter().any(|m| m.name == "Steamodded"),
        "Talisman" => installed_mods.iter().any(|m| m.name == "Talisman"),
        _ => return Err(AppError::InvalidState("Invalid mod type".to_string()).to_string()),
    })
}

#[tauri::command]
async fn check_existing_installation(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    if let Some(path) = db.get_installation_path()? {
        let path_buf = PathBuf::from(&path);
        if bmm_lib::balamod::Balatro::from_custom_path(path_buf).is_some() {
            Ok(Some(path))
        } else {
            map_error(db.remove_installation_path())?;
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn install_mod(url: String) -> Result<PathBuf, String> {
    map_error(bmm_lib::installer::install_mod(url).await)
}

#[tauri::command]
async fn get_installed_mods_from_db(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InstalledMod>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
    map_error(db.get_installed_mods())
}

#[tauri::command]
async fn add_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.add_installed_mod(&name, &path))
}

#[tauri::command]
async fn remove_installed_mod(
    state: tauri::State<'_, AppState>,
    name: String,
    path: String,
) -> Result<(), String> {
    bmm_lib::installer::uninstall_mod(PathBuf::from(path))?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.remove_installed_mod(&name))
}

#[tauri::command]
async fn get_balatro_path(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    map_error(db.get_installation_path())
}

#[tauri::command]
async fn set_balatro_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let db = match state.db.lock() {
        Ok(db) => db,
        Err(e) => return Err(e.to_string()),
    };
    map_error(db.set_installation_path(&path))
}

#[tauri::command]
async fn find_steam_balatro(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let balatros = find_balatros();
    if let Some(path) = balatros.first() {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        map_error(db.set_installation_path(&path.path.to_string_lossy()))?;
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
    match std::fs::exists(PathBuf::from(path)) {
        Ok(exists) => exists,
        Err(e) => {
            log::error!("Failed to check path existence: {}", e);
            false
        }
    }
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
        map_error(db.set_installation_path(&path.to_string_lossy()))?;
    }

    Ok(is_valid)
}

#[tauri::command]
async fn open_image_popup(app: tauri::AppHandle, image_url: String, title: String) {
    let _popup = match WebviewWindowBuilder::new(
        &app,
        "image_popup",
        WebviewUrl::App(format!("image-popup.html?image={}", image_url).into()),
    )
    .title(title)
    .inner_size(800.0, 600.0)
    .center()
    .build()
    {
        Ok(popup) => popup,
        Err(e) => {
            log::error!("Failed to open image popup: {}", e);
            return;
        }
    };
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
            // let app_handle = app.handle();

            // Initialize database with error handling
            let db = map_error(Database::new())?;

            app.manage(AppState { db: Mutex::new(db) });

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
            load_versions_cache,
            set_lovely_console_status,
            get_lovely_console_status,
            clear_cache
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("Failed to run application: {}", e);
        std::process::exit(1);
    }
}
