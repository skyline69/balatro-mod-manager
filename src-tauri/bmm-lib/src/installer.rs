use crate::errors::AppError;
use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs;
use std::io::Read;
use std::io::{self, Cursor};
use std::path::Path;
use std::path::PathBuf;
use tar::Archive;
use zip::ZipArchive;

pub async fn install_mod(url: String, folder_name: Option<String>) -> Result<PathBuf, AppError> {
    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::NetworkRequest {
            url: url.clone(),
            source: e.to_string(),
        })?;

    let file = response
        .bytes()
        .await
        .map_err(|e| AppError::NetworkRequest {
            url: url.clone(),
            source: e.to_string(),
        })?;

    let file_type = infer::get(&file)
        .ok_or_else(|| AppError::InvalidState("Unknown file type".into()))?
        .mime_type();

    let mod_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?
        .join("Balatro")
        .join("Mods");

    let mod_name = {
        if let Some(name) = folder_name.filter(|n| !n.is_empty()) {
            // Use provided folder name if it exists and isn't empty
            name
        } else {
            // Extract from URL as fallback
            let url_name = url
                .split('/')
                .last()
                .and_then(|s| s.split('.').next())
                .unwrap_or("unknown_mod");

            // If the extracted name is too generic (like "main" or "master")
            if url_name == "main" || url_name == "master" || url_name.len() <= 2 {
                // Generate a more unique name with a timestamp
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                format!("mod_{}", timestamp)
            } else {
                url_name.to_string()
            }
        }
    };

    log::info!("Installing mod: {}", url);

    let installed_path = match file_type {
        "application/zip" => handle_zip(file, &mod_dir, &mod_name)?,
        "application/x-tar" => handle_tar(file, &mod_dir)?,
        "application/gzip" => handle_tar_gz(file, &mod_dir)?,
        _ => {
            return Err(AppError::InvalidState(format!(
                "Unsupported file type: {}",
                file_type
            )))
        }
    };

    log::info!("Mod installed successfully at: {:?}", installed_path);
    Ok(installed_path)
}

fn handle_zip(file: bytes::Bytes, mod_dir: &Path, mod_name: &str) -> Result<PathBuf, AppError> {
    let cursor = Cursor::new(file);
    let mut zip = ZipArchive::new(cursor).map_err(|e| AppError::FileWrite {
        path: mod_dir.to_path_buf(),
        source: format!("Invalid zip archive: {}", e),
    })?;

    // Determine if ZIP has root files
    let has_root_files = (0..zip.len()).try_fold(false, |acc, i| -> Result<bool, AppError> {
        let file = zip.by_index(i).map_err(|e| AppError::FileRead {
            path: mod_dir.to_path_buf(),
            source: format!("Zip entry error: {}", e),
        })?;
        Ok(acc || !file.name().contains('/'))
    })?;

    // The target directory where the mod will be installed
    let target_dir = mod_dir.join(mod_name);

    // Remove target directory if it exists
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).map_err(|e| AppError::FileWrite {
            path: target_dir.clone(),
            source: e.to_string(),
        })?;
    }

    if has_root_files {
        // For ZIPs with root files
        fs::create_dir_all(&target_dir).map_err(|e| AppError::DirCreate {
            path: target_dir.clone(),
            source: e.to_string(),
        })?;

        extract_zip_root(&mut zip, &target_dir)?;
    } else {
        // For ZIPs with a folder structure
        // Create temp directory
        let temp_dir = mod_dir.join("temp_extract");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir).map_err(|e| AppError::DirCreate {
                path: temp_dir.clone(),
                source: e.to_string(),
            })?;
        }

        fs::create_dir_all(&temp_dir).map_err(|e| AppError::DirCreate {
            path: temp_dir.clone(),
            source: e.to_string(),
        })?;

        // Extract to temp directory
        extract_zip(&mut zip, &temp_dir)?;

        // Get root directory name
        let root_dir = get_zip_root_dir(&mut zip, &temp_dir)?;
        let source_dir = temp_dir.join(root_dir);

        // Move to target directory
        fs::rename(&source_dir, &target_dir).map_err(|e| AppError::FileWrite {
            path: source_dir.clone(),
            source: format!("Failed to rename directory: {}", e),
        })?;

        // Clean up
        fs::remove_dir_all(&temp_dir).map_err(|e| AppError::DirCreate {
            path: temp_dir.clone(),
            source: e.to_string(),
        })?;
    }

    Ok(target_dir)
}

fn extract_zip_root(
    zip: &mut ZipArchive<Cursor<bytes::Bytes>>,
    path: &PathBuf,
) -> Result<(), AppError> {
    fs::create_dir_all(path).map_err(|e| AppError::DirCreate {
        path: path.clone(),
        source: e.to_string(),
    })?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| AppError::FileRead {
            path: path.clone(),
            source: format!("Zip entry error: {}", e),
        })?;

        let entry_path = path.join(file.name());
        ensure_safe_path(path, &entry_path)?;

        if file.is_dir() {
            fs::create_dir_all(&entry_path).map_err(|e| AppError::DirCreate {
                path: entry_path.clone(),
                source: e.to_string(),
            })?;
        } else {
            create_parent_dir(&entry_path)?;
            copy_file_contents(&mut file, &entry_path)?;
        }
    }
    Ok(())
}

fn get_zip_root_dir(
    zip: &mut ZipArchive<Cursor<bytes::Bytes>>,
    mod_dir: &Path,
) -> Result<String, AppError> {
    let first_entry = zip.by_index(0).map_err(|e| AppError::FileRead {
        path: mod_dir.to_path_buf(),
        source: format!("Zip entry error: {}", e),
    })?;

    let name_parts: Vec<&str> = first_entry.name().split('/').collect();
    name_parts
        .first()
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::InvalidState("Empty zip archive".into()))
}

fn extract_zip(zip: &mut ZipArchive<Cursor<bytes::Bytes>>, mod_dir: &Path) -> Result<(), AppError> {
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| AppError::FileRead {
            path: mod_dir.to_path_buf(),
            source: format!("Zip entry error: {}", e),
        })?;

        let entry_path = mod_dir.join(file.mangled_name());
        ensure_safe_path(mod_dir, &entry_path)?;

        if file.is_dir() {
            fs::create_dir_all(&entry_path).map_err(|e| AppError::DirCreate {
                path: entry_path.clone(),
                source: e.to_string(),
            })?;
        } else {
            create_parent_dir(&entry_path)?;
            copy_file_contents(&mut file, &entry_path)?;
        }
    }
    Ok(())
}

fn handle_tar(file: bytes::Bytes, mod_dir: &PathBuf) -> Result<PathBuf, AppError> {
    let cursor = Cursor::new(file);
    let mut tar = Archive::new(cursor);
    extract_tar(&mut tar, mod_dir)
}

fn handle_tar_gz(file: bytes::Bytes, mod_dir: &PathBuf) -> Result<PathBuf, AppError> {
    let cursor = Cursor::new(file);
    let gz = GzDecoder::new(cursor);
    let mut tar = Archive::new(gz);
    extract_tar(&mut tar, mod_dir)
}

fn extract_tar(tar: &mut Archive<impl Read>, mod_dir: &PathBuf) -> Result<PathBuf, AppError> {
    let entries = tar.entries().map_err(|e| AppError::FileRead {
        path: mod_dir.clone(),
        source: format!("Tar entry error: {}", e),
    })?;

    let mut installed_path = mod_dir.clone();
    for entry in entries {
        let mut entry = entry.map_err(|e| AppError::FileRead {
            path: mod_dir.clone(),
            source: format!("Tar entry error: {}", e),
        })?;

        let path = mod_dir.join(entry.path().map_err(|e| AppError::FileRead {
            path: mod_dir.clone(),
            source: format!("Invalid path in tar: {}", e),
        })?);

        ensure_safe_path(mod_dir, &path)?;

        if installed_path == *mod_dir {
            installed_path = path.parent().unwrap_or(mod_dir).to_path_buf();
        }

        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&path).map_err(|e| AppError::DirCreate {
                path: path.clone(),
                source: e.to_string(),
            })?;
        } else {
            create_parent_dir(&path)?;
            copy_file_contents(&mut entry, &path)?;
        }
    }

    Ok(installed_path)
}

fn create_parent_dir(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::DirCreate {
            path: parent.to_path_buf(),
            source: e.to_string(),
        })
    } else {
        Ok(())
    }
}

fn copy_file_contents(reader: &mut impl io::Read, path: &PathBuf) -> Result<(), AppError> {
    let mut output = fs::File::create(path).map_err(|e| AppError::FileWrite {
        path: path.clone(),
        source: e.to_string(),
    })?;

    io::copy(reader, &mut output).map_err(|e| AppError::FileWrite {
        path: path.clone(),
        source: e.to_string(),
    })?;

    Ok(())
}

fn ensure_safe_path(base: &Path, path: &Path) -> Result<(), AppError> {
    if !path.starts_with(base) {
        Err(AppError::PathValidation {
            path: path.to_path_buf(),
            reason: "Path traversal attempt detected".into(),
        })
    } else {
        Ok(())
    }
}

pub fn uninstall_mod(path: PathBuf) -> Result<(), AppError> {
    log::info!("Uninstalling mod: {:?}", path);

    let mods_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?
        .join("Balatro")
        .join("Mods");

    validate_uninstall_path(&path, &mods_dir)?;

    if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
        if dir_name.starts_with("Steamodded-smods-") {
            log::info!("Uninstalling Steamodded variant: {}", dir_name);
        }
    }

    fs::remove_dir_all(&path).map_err(|e| AppError::FileWrite {
        path,
        source: e.to_string(),
    })
}

fn validate_uninstall_path(path: &PathBuf, mods_dir: &PathBuf) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::PathValidation {
            path: path.clone(),
            reason: "Path doesn't exist".into(),
        });
    }

    if path == mods_dir {
        return Err(AppError::InvalidState(
            "Blocked attempt to delete Mods directory".into(),
        ));
    }

    if !path.starts_with(mods_dir) {
        return Err(AppError::PathValidation {
            path: path.clone(),
            reason: "Path outside Mods directory".into(),
        });
    }

    Ok(())
}
