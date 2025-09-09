pub mod cache;
pub mod detection;
pub mod import;
pub mod install;
pub mod lovely;
pub mod mods;
pub mod paths;
pub mod profiles;
pub mod repo;
pub mod report;
pub mod settings;
pub mod system;
pub mod thumbnails;

// Re-export commands so `generate_handler!` can take flat names if desired.
pub use lovely::{check_lovely_update, is_lovely_installed, update_lovely_to_latest};
pub use system::{check_balatro_running, check_steam_running};
