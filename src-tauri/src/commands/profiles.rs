use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::state::AppState;
use bmm_lib::{errors::AppError, local_mod_detection};

#[derive(Serialize)]
pub struct SimpleModDir {
    pub name: String,     // folder name
    pub path: String,     // absolute path
    pub dir_name: String, // folder name (same as name)
    pub active: bool,     // true if under Mods, false if under Inactive Mods
}

fn config_dirs() -> Result<(PathBuf, PathBuf), String> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| "Could not find config directory".to_string())?;
    let balatro = config_dir.join("Balatro");
    let mods = balatro.join("Mods");
    let inactive = balatro.join("Inactive Mods");
    Ok((mods, inactive))
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
    }
    Ok(())
}

fn list_dirs_in(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in
        fs::read_dir(root).map_err(|e| format!("Failed to read {}: {}", root.display(), e))?
    {
        let e = entry.map_err(|e| format!("ReadDir entry error: {}", e))?;
        let p = e.path();
        if p.is_dir() {
            // filter out lovely-related directories and hidden/system dirs
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains("lovely") || name_lower.starts_with('.') {
                    continue;
                }
            }
            out.push(p);
        }
    }
    Ok(out)
}

fn unique_target_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let mut i = 1u32;
    loop {
        let new = dir.join(format!("{} ({})", name, i));
        if !new.exists() {
            return new;
        }
        i += 1;
    }
}

fn move_dir(from: &Path, to_dir: &Path) -> Result<PathBuf, String> {
    ensure_dir(to_dir)?;
    let name = from
        .file_name()
        .ok_or_else(|| format!("Invalid directory: {}", from.display()))?
        .to_string_lossy()
        .to_string();
    let target = unique_target_path(to_dir, &name);
    fs::rename(from, &target).map_err(|e| {
        format!(
            "Failed to move '{}' to '{}': {}",
            from.display(),
            target.display(),
            e
        )
    })?;
    Ok(target)
}

#[tauri::command]
pub async fn list_installed_mod_dirs() -> Result<Vec<SimpleModDir>, String> {
    let (mods_dir, inactive_dir) = config_dirs()?;
    let mut out: Vec<SimpleModDir> = Vec::new();
    for p in list_dirs_in(&mods_dir)? {
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(SimpleModDir {
            name: name.clone(),
            dir_name: name,
            path: p.to_string_lossy().to_string(),
            active: true,
        });
    }
    for p in list_dirs_in(&inactive_dir)? {
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        out.push(SimpleModDir {
            name: name.clone(),
            dir_name: name,
            path: p.to_string_lossy().to_string(),
            active: false,
        });
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct ModProfileDto {
    pub id: i64,
    pub name: String,
    pub mods: Vec<String>,
}

#[tauri::command]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModProfileDto>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    let profiles = db
        .list_profiles()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| ModProfileDto {
            id: p.id,
            name: p.name,
            mods: p.mods,
        })
        .collect();
    Ok(profiles)
}

#[tauri::command]
pub async fn create_profile(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<i64, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.create_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.delete_profile(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_profile(
    state: tauri::State<'_, AppState>,
    id: i64,
    new_name: String,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.rename_profile(id, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_profile_mods(
    state: tauri::State<'_, AppState>,
    id: i64,
    mod_dir_names: Vec<String>,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.set_profile_mods(id, &mod_dir_names)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_active_profile_id(
    state: tauri::State<'_, AppState>,
) -> Result<Option<i64>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.get_active_profile_id().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_active_profile_id(
    state: tauri::State<'_, AppState>,
    id: Option<i64>,
) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    db.set_active_profile_id(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_profile(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    let allowed_set: HashSet<String> = db
        .get_profile_mods(id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();

    let (mods_dir, inactive_dir) = config_dirs()?;
    ensure_dir(&mods_dir)?;
    ensure_dir(&inactive_dir)?;

    // 1) Move disallowed mods from Mods -> Inactive Mods
    for p in list_dirs_in(&mods_dir)? {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if !allowed_set.contains(name) {
                let old = p.clone();
                let newp = move_dir(&p, &inactive_dir)?;
                db.update_installed_mod_path_by_path(
                    &old.to_string_lossy(),
                    &newp.to_string_lossy(),
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    // 2) Move allowed mods from Inactive Mods -> Mods
    for p in list_dirs_in(&inactive_dir)? {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if allowed_set.contains(name) {
                let old = p.clone();
                let newp = move_dir(&p, &mods_dir)?;
                db.update_installed_mod_path_by_path(
                    &old.to_string_lossy(),
                    &newp.to_string_lossy(),
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    // 3) Invalidate detection cache (filesystem layout changed)
    local_mod_detection::clear_detection_cache();

    // 4) Mark active profile
    db.set_active_profile_id(Some(id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_active_profile(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Move everything from Inactive Mods back to Mods and clear active profile id
    let (mods_dir, inactive_dir) = config_dirs()?;
    ensure_dir(&mods_dir)?;
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::LockPoisoned("Database lock poisoned".to_string()).to_string())?;
    if inactive_dir.exists() {
        for p in list_dirs_in(&inactive_dir)? {
            let old = p.clone();
            let newp = move_dir(&p, &mods_dir)?;
            db.update_installed_mod_path_by_path(&old.to_string_lossy(), &newp.to_string_lossy())
                .map_err(|e| e.to_string())?;
        }
    }
    // Invalidate detection cache after moving all directories back
    local_mod_detection::clear_detection_cache();
    db.set_active_profile_id(None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_active_profile_if_set(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let id_opt = {
        let db = state.db.lock().map_err(|_| {
            AppError::LockPoisoned("Database lock poisoned".to_string()).to_string()
        })?;
        db.get_active_profile_id().map_err(|e| e.to_string())?
    };
    if let Some(id) = id_opt {
        apply_profile(state, id).await
    } else {
        Ok(())
    }
}
