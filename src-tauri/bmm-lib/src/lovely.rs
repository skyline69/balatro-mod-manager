use std::path::PathBuf;
use std::fs;
use libloading::Library;
use crate::errors::AppError;

pub fn ensure_lovely_exists() -> Result<PathBuf, AppError> {
    #[cfg(target_os = "macos")]
    {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")))?;
        
        let bins_dir = config_dir.join("Balatro").join("bins");
        fs::create_dir_all(&bins_dir).map_err(|e| AppError::DirCreate {
            path: bins_dir.clone(),
            source: e.to_string(),
        })?;
        
        let lovely_path = bins_dir.join("liblovely.dylib");
        
        if !lovely_path.exists() {
            return Err(AppError::MacOsLibrary {
                lib_name: "liblovely.dylib".into(),
                source: "Lovely binary not found. Please install it first.".into(),
            });
        }

        // Validate library loading
        unsafe {
            Library::new(&lovely_path).map_err(|e| AppError::MacOsLibrary {
                lib_name: lovely_path.display().to_string(),
                source: format!("Failed to load library: {}", e),
            })?;
        }

        Ok(lovely_path)
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Err(AppError::InvalidState(
            "Lovely injection is only supported on macOS".into()
        ))
    }
}

