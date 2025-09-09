use std::path::{Path, PathBuf};

use crate::state::AppState;
use bmm_lib::errors::AppError;
use std::fs;

fn profile_dirs() -> Result<(PathBuf, PathBuf), String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    let balatro = config_dir.join("Balatro");
    let mods = balatro.join("Mods");
    let inactive = balatro.join("Inactive Mods");
    Ok((mods, inactive))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create {}: {}", path.display(), e))?;
    }
    Ok(())
}

fn unique_target_path(dir: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    if !target.exists() {
        return target;
    }
    let mut i = 1u32;
    loop {
        let alt = dir.join(format!("{} ({})", name, i));
        if !alt.exists() {
            return alt;
        }
        i += 1;
    }
}

#[tauri::command]
pub async fn is_mod_enabled(
    state: tauri::State<'_, AppState>,
    mod_name: String,
) -> Result<bool, String> {
    // Resolve mod path from DB within a short scope
    let path = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
        let installed_mods = db.get_installed_mods()?;
        installed_mods
            .iter()
            .find(|m| m.name == mod_name)
            .ok_or_else(|| format!("Mod not found: {mod_name}"))?
            .path
            .clone()
    };
    is_mod_enabled_by_path(path).await
}

#[tauri::command]
pub async fn toggle_mod_enabled(
    state: tauri::State<'_, AppState>,
    mod_name: String,
    enabled: bool,
) -> Result<(), String> {
    let path = {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
        let installed_mods = db.get_installed_mods()?;
        installed_mods
            .iter()
            .find(|m| m.name == mod_name)
            .ok_or_else(|| format!("Mod not found: {mod_name}"))?
            .path
            .clone()
    };
    toggle_mod_enabled_by_path(state, path, enabled).await
}

#[tauri::command]
pub async fn is_mod_enabled_by_path(mod_path: String) -> Result<bool, String> {
    let path = PathBuf::from(&mod_path);
    let canonical = path
        .canonicalize()
        .map_err(|_| format!("Path does not exist: {mod_path}"))?;
    let (mods_dir, inactive_dir) = profile_dirs()?;
    let mods_canon = mods_dir.canonicalize().unwrap_or(mods_dir);
    let inactive_canon = inactive_dir.canonicalize().unwrap_or(inactive_dir);
    if canonical.starts_with(&mods_canon) {
        Ok(true)
    } else if canonical.starts_with(&inactive_canon) {
        Ok(false)
    } else {
        // Default: treat as enabled if outside managed dirs
        Ok(true)
    }
}

#[tauri::command]
pub async fn toggle_mod_enabled_by_path(
    state: tauri::State<'_, AppState>,
    mod_path: String,
    enabled: bool,
) -> Result<(), String> {
    let path = PathBuf::from(&mod_path);
    if !path.exists() {
        return Err(format!("Mod path does not exist: {mod_path}"));
    }

    let (mods_dir, inactive_dir) = profile_dirs()?;
    ensure_dir(&mods_dir)?;
    ensure_dir(&inactive_dir)?;

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize {}: {}", path.display(), e))?;
    let current_parent = canonical
        .parent()
        .ok_or_else(|| "Invalid mod path".to_string())?;
    let name = canonical
        .file_name()
        .ok_or_else(|| "Invalid mod path".to_string())?
        .to_string_lossy()
        .to_string();

    let mods_canon = mods_dir.canonicalize().unwrap_or(mods_dir.clone());
    let inactive_canon = inactive_dir.canonicalize().unwrap_or(inactive_dir.clone());

    if enabled {
        if current_parent.starts_with(&inactive_canon) {
            let target = unique_target_path(&mods_canon, &name);
            fs::rename(&canonical, &target).map_err(|e| format!("Move failed: {}", e))?;
            // Update DB path so detection doesn't treat it as manual
            let db = state
                .db
                .lock()
                .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
            db.update_installed_mod_path_by_path(
                &canonical.to_string_lossy(),
                &target.to_string_lossy(),
            )
            .map_err(|e| e.to_string())?;
        }
    } else if current_parent.starts_with(&mods_canon) {
        let target = unique_target_path(&inactive_canon, &name);
        fs::rename(&canonical, &target).map_err(|e| format!("Move failed: {}", e))?;
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;
        db.update_installed_mod_path_by_path(
            &canonical.to_string_lossy(),
            &target.to_string_lossy(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
