// src/errors.rs
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum AppError {
    // Database errors
    DatabaseInit(String),
    DatabaseQuery(String),
    DatabaseTransaction(String),

    // File system errors
    FileRead {
        path: PathBuf,
        source: String,
    },
    FileWrite {
        path: PathBuf,
        source: String,
    },
    DirCreate {
        path: PathBuf,
        source: String,
    },
    DirNotFound(PathBuf),

    // System errors
    SystemTime(String),
    ProcessExecution(String),

    // Application state
    LockPoisoned(String),
    InvalidState(String),

    // Mod management
    ModInstall {
        mod_name: String,
        source: String,
    },
    ModConflict {
        mod_name: String,
        conflicts: Vec<String>,
    },
    ModNotFound {
        mod_name: String,
        version: String,
    },

    // Network/API
    NetworkRequest {
        url: String,
        source: String,
    },
    ApiLimitExceeded,
    InvalidApiResponse(String),

    // Platform specific
    MacOsLibrary {
        lib_name: String,
        source: String,
    },

    // Configuration
    InvalidConfig {
        key: String,
        value: String,
    },
    PathValidation {
        path: PathBuf,
        reason: String,
    },

    // UI/Window
    WindowCreation(String),
    DialogError(String),

    // Serialization
    Serialization {
        format: String,
        source: String,
    },

    // Miscellaneous
    Unknown(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DatabaseInit(msg) => write!(f, "Database initialization failed: {}", msg),
            AppError::DatabaseQuery(msg) => write!(f, "Database query error: {}", msg),

            AppError::FileRead { path, source } => {
                write!(f, "Failed to read file '{}': {}", path.display(), source)
            }

            AppError::ModInstall { mod_name, source } => {
                write!(f, "Failed to install mod '{}': {}", mod_name, source)
            }

            AppError::NetworkRequest { url, source } => {
                write!(f, "Network request to '{}' failed: {}", url, source)
            }

            AppError::MacOsLibrary { lib_name, source } => {
                write!(f, "MacOS library '{}' error: {}", lib_name, source)
            }

            AppError::PathValidation { path, reason } => {
                write!(f, "Invalid path '{}': {}", path.display(), reason)
            }

            // Handle all variants similarly
            _ => write!(f, "{:?}", self),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // Implement if needed for error chaining
    }
}

// Conversion implementations
impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::DatabaseQuery(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::FileRead {
            path: PathBuf::from("unknown"),
            source: err.to_string(),
        }
    }
}

impl From<SystemTimeError> for AppError {
    fn from(err: SystemTimeError) -> Self {
        AppError::SystemTime(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::NetworkRequest {
            url: err.url().map(|u| u.to_string()).unwrap_or_default(),
            source: err.to_string(),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::WindowCreation(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Unknown(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization {
            format: "JSON".to_string(),
            source: err.to_string(),
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        AppError::LockPoisoned(format!("Mutex poison error: {}", err))
    }
}

// For Tauri command result compatibility
impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        format!("{}", err)
    }
}

// Additional constructors
impl AppError {
    pub fn invalid_path(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        AppError::PathValidation {
            path: path.into(),
            reason: reason.into(),
        }
    }

    pub fn mod_install_error(mod_name: impl Into<String>, source: impl Into<String>) -> Self {
        AppError::ModInstall {
            mod_name: mod_name.into(),
            source: source.into(),
        }
    }

    pub fn config_error(key: impl Into<String>, value: impl Into<String>) -> Self {
        AppError::InvalidConfig {
            key: key.into(),
            value: value.into(),
        }
    }
}
