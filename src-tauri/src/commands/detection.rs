use std::path::{Path, PathBuf};

use crate::state::AppState;
use crate::util::map_error;
use bmm_lib::{cache, database::Database, errors::AppError, local_mod_detection};
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub async fn check_mod_installation(mod_type: String) -> Result<bool, String> {
    let db = map_error(bmm_lib::database::Database::new())?;
    let installed_mods = map_error(db.get_installed_mods())?;

    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(),
    };
    let detected_mods = local_mod_detection::detect_manual_mods_cached(&db, &cached_mods)?;

    let mod_name = mod_type.as_str();
    match mod_name {
        "Steamodded" | "Talisman" => Ok(installed_mods.iter().any(|m| m.name == mod_name)
            || detected_mods.iter().any(|m| m.name == mod_name)),
        _ => Err(AppError::InvalidState("Invalid mod type".to_string()).to_string()),
    }
}

#[tauri::command]
pub async fn refresh_mods_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;

    let installed = map_error(db.get_installed_mods())?;
    for m in installed {
        let mod_dir = PathBuf::from(&m.path);
        if mod_dir.exists() {
            for entry in std::fs::read_dir(&mod_dir)
                .map_err(|e| format!("Failed to read mod directory: {e}"))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
                let path = entry.path();
                if path.is_dir() {
                    let ignore_file_path = path.join(".lovelyignore");
                    if ignore_file_path.exists() {
                        std::fs::remove_file(&ignore_file_path).map_err(|e| {
                            AppError::FileWrite {
                                path: path.clone(),
                                source: e.to_string(),
                            }
                        })?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_detected_local_mods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<local_mod_detection::DetectedMod>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(),
    };
    local_mod_detection::detect_manual_mods_cached(&db, &cached_mods)
}

/// Reindexes mods by syncing the database with the filesystem.
/// Returns (files_removed, db_entries_cleaned). Currently we only clean DB entries.
#[tauri::command]
pub async fn reindex_mods(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(usize, usize), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()))?;

    let result = reindex_db(&db);
    match &result {
        Ok((_files, cleaned)) if *cleaned > 0 => {
            // Best-effort event notify; ignore if there are no listeners
            let _ = app_handle.emit("installed-mods-changed", ());
        }
        _ => {}
    }
    map_error(result)
}

/// Internal helper to perform the actual reindexing logic.
/// Returns (files_removed, db_entries_cleaned). Currently we only clean DB entries.
pub fn reindex_db(db: &Database) -> Result<(usize, usize), AppError> {
    let installed = db.get_installed_mods()?;
    let mut cleaned_entries = 0usize;
    for m in installed {
        let path = PathBuf::from(&m.path);
        if !path.exists() {
            // Try to detect a move between Mods <-> Inactive Mods and heal path
            let maybe_new = (|| -> Option<PathBuf> {
                let file_name = Path::new(&m.path).file_name()?.to_owned();
                let config_dir = dirs::config_dir()?;
                let balatro = config_dir.join("Balatro");
                let mods_dir = balatro.join("Mods");
                let inactive_dir = balatro.join("Inactive Mods");
                for cand in [mods_dir.join(&file_name), inactive_dir.join(&file_name)] {
                    if cand.exists() && cand.is_dir() {
                        return Some(cand);
                    }
                }
                None
            })();
            if let Some(new_path) = maybe_new {
                // Best-effort update; if it fails, fall back to removal
                if db
                    .update_installed_mod_path_by_path(&m.path, &new_path.to_string_lossy())
                    .is_ok()
                {
                    continue;
                }
            }
            // Remove if no target found
            db.remove_installed_mod(&m.name)?;
            cleaned_entries += 1;
        }
    }

    // Clear detection cache so next detection reflects changes
    local_mod_detection::clear_detection_cache();

    Ok((0, cleaned_entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn reindex_db_removes_missing_paths() {
        // Redirect config dir to temp
        let td = tempdir().unwrap();
        let original_cfg = std::env::var_os("XDG_CONFIG_HOME");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", td.path());
        if cfg!(target_os = "macos") {
            std::env::set_var("HOME", td.path());
        }

        let db = Database::new().expect("db");

        // Create one existing and one missing mod path
        let mods_dir = dirs::config_dir().unwrap().join("Balatro").join("Mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        let existing = mods_dir.join("Exists");
        std::fs::create_dir_all(&existing).unwrap();
        let missing = mods_dir.join("Missing"); // do not create

        db.add_installed_mod("Existing", existing.to_string_lossy().as_ref(), &[], None)
            .unwrap();
        db.add_installed_mod("Missing", missing.to_string_lossy().as_ref(), &[], None)
            .unwrap();

        let (_files, cleaned) = reindex_db(&db).expect("reindex");
        assert_eq!(cleaned, 1);

        let remaining = db.get_installed_mods().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "Existing");

        // restore env
        match original_cfg {
            Some(val) => std::env::set_var("XDG_CONFIG_HOME", val),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match original_home {
            Some(val) => std::env::set_var("HOME", val),
            None => std::env::remove_var("HOME"),
        }
    }
}

#[tauri::command]
pub async fn delete_manual_mod(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!(
            "Invalid path '{}': Path doesn't exist",
            path.display()
        ));
    }
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    let mods_dir = config_dir.join("Balatro").join("Mods");

    let canonicalized_path = path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize path {}: {}", path.display(), e))?;
    let canonicalized_mods_dir = mods_dir
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize mods directory: {e}"))?;
    if !canonicalized_path.starts_with(&canonicalized_mods_dir) {
        return Err(format!(
            "Path is outside of the mods directory: {}",
            path.display()
        ));
    }

    if path.is_dir() {
        std::fs::remove_dir_all(&path).map_err(|e| format!("Failed to remove directory: {e}"))?
    } else {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to remove file: {e}"))?
    }
    Ok(())
}

#[tauri::command]
pub async fn backup_local_mod(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("Path doesn't exist: {}", path.display()));
    }

    let backup_dir = get_backup_dir()?;
    let backup_id = format!(
        "backup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Failed to get timestamp: {e}"))?
            .as_millis()
    );
    let backup_path = backup_dir.join(backup_id);

    std::fs::create_dir_all(&backup_path)
        .map_err(|e| format!("Failed to create backup directory: {e}"))?;

    if path.is_dir() {
        copy_dir_all(&path, &backup_path.join(path.file_name().unwrap()))
            .map_err(|e| format!("Failed to copy mod to backup: {e}"))?;
    } else {
        std::fs::copy(&path, backup_path.join(path.file_name().unwrap()))
            .map_err(|e| format!("Failed to copy mod file to backup: {e}"))?;
    }

    let metadata = json!({
        "original_path": path.to_string_lossy().to_string(),
        "backup_time": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    std::fs::write(
        backup_path.join("metadata.json"),
        serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {e}"))?,
    )
    .map_err(|e| format!("Failed to write metadata: {e}"))?;

    Ok(())
}

#[tauri::command]
pub async fn restore_from_backup(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let backup_dir = get_backup_dir()?;

    let mut latest_backup = None;
    let mut latest_time = 0;
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {e}"))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let metadata: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&metadata_path)
                    .map_err(|e| format!("Failed to read metadata file: {e}"))?,
            )
            .map_err(|e| format!("Failed to parse metadata: {e}"))?;
            if let Some(original_path) = metadata.get("original_path").and_then(|v| v.as_str()) {
                if original_path == path.to_string_lossy() {
                    if let Some(backup_time) = metadata.get("backup_time").and_then(|v| v.as_u64())
                    {
                        if backup_time > latest_time {
                            latest_time = backup_time;
                            latest_backup = Some(entry.path());
                        }
                    }
                }
            }
        }
    }

    let backup_path = latest_backup.ok_or_else(|| "No backup found for this path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create parent directory: {e}"))?;
    }
    for entry in std::fs::read_dir(&backup_path)
        .map_err(|e| format!("Failed to read backup directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {e}"))?;
        let file_name = entry.file_name();
        if file_name == "metadata.json" {
            continue;
        }
        let dest_path = path.parent().unwrap().join(&file_name);
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dest_path)
                .map_err(|e| format!("Failed to restore directory from backup: {e}"))?;
        } else {
            std::fs::copy(entry.path(), &dest_path)
                .map_err(|e| format!("Failed to restore file from backup: {e}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_backup(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let backup_dir = get_backup_dir()?;
    for entry in std::fs::read_dir(&backup_dir)
        .map_err(|e| format!("Failed to read backup directory: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Failed to read backup entry: {e}"))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let metadata: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&metadata_path)
                    .map_err(|e| format!("Failed to read metadata file: {e}"))?,
            )
            .map_err(|e| format!("Failed to parse metadata: {e}"))?;
            if let Some(original_path) = metadata.get("original_path").and_then(|v| v.as_str()) {
                if original_path == path.to_string_lossy() {
                    std::fs::remove_dir_all(entry.path())
                        .map_err(|e| format!("Failed to remove backup: {e}"))?;
                }
            }
        }
    }
    Ok(())
}

fn get_backup_dir() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("balatro_mod_manager_backups");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create backup directory: {e}"))?;
    Ok(temp_dir)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        if ty.is_dir() {
            copy_dir_all(&path, &dst.join(path.file_name().unwrap()))?;
        } else {
            std::fs::copy(&path, dst.join(path.file_name().unwrap()))?;
        }
    }
    Ok(())
}
