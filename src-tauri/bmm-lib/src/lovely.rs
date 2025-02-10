use crate::errors::AppError;
#[cfg(target_os = "macos")]
use libloading::Library;
#[cfg(target_os = "macos")]
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub const EMBEDDED_DLL: &[u8] = include_bytes!("../../resources/version.dll");

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

    #[cfg(target_os = "windows")]
    {
        // Get Balatro installation paths for validation
        let balatro_paths = crate::finder::get_balatro_paths();
        if balatro_paths.is_empty() {
            return Err(AppError::DirNotFound(PathBuf::from("Balatro installation")));
        }

        // Return the path to the first valid installation
        return Ok(balatro_paths[0].join("Balatro.exe"));
    }

    // #[cfg(not(target_os = "macos"))]
    // if not macOS or Windows
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(AppError::InvalidState(
            "Lovely injection is not supported on this platform.".into(),
        ))
    }
}
