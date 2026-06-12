//! Scanning Mods folder and detecting untracked mods.
//!
//! This module provides functionality to detect mods that are installed locally
//! but not tracked in the database. It scans the Mods directory and attempts to
//! match found mods against the remote catalog.
//!
//! # Features
//!
//! - Scans the Mods directory for installed mods
//! - Matches local mods to catalog entries using fuzzy matching
//! - Caches detection results to avoid repeated filesystem scans
//! - Handles Proton/Wine prefix symlinks on Linux
//!
//! # Detection Logic
//!
//! Mods are identified by:
//! 1. Presence of a `lovely.toml` configuration file
//! 2. Mod metadata in JSON files
//! 3. Directory name matching against the catalog

use crate::cache;
use crate::database::Database;
use crate::finder;
use lazy_static::lazy_static;
#[cfg(target_os = "linux")]
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
#[cfg(target_os = "linux")]
use std::os::unix::fs::symlink;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, UNIX_EPOCH};

// Time-to-live for the detection cache. Even if the fingerprint hasn't
// changed, drop the cached result after this window so a long-running
// process eventually re-scans and picks up subtle changes (e.g. metadata
// edits that don't bump the parent directory's mtime).
const DETECTION_CACHE_TTL_SECS: u64 = 300;

// Simple cache of detected local mods keyed by a lightweight fingerprint of the Mods directory
lazy_static! {
    static ref DETECTION_CACHE: Mutex<Option<(ScanFingerprint, Instant, Vec<DetectedMod>)>> =
        Mutex::new(None);
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScanFingerprint {
    mods_dir: String,
    // checksum that changes when top-level items change (names or mtimes)
    checksum: u64,
}

fn get_dir_mtime(path: &Path) -> u64 {
    path.metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn compute_fingerprint(mods_dir: &Path) -> ScanFingerprint {
    let mut sum: u64 = 1469598103934665603; // FNV offset basis
    let dir_iter = fs::read_dir(mods_dir);
    if let Ok(entries) = dir_iter {
        for entry in entries.flatten() {
            let p = entry.path();
            // Consider directories and top-level `.zip` mod archives (Steamodded
            // loads zipped mods directly). Other files (e.g. .lovelyignore) are
            // ignored as they don't change the detected-mod list.
            let is_dir = p.is_dir();
            let is_zip = !is_dir
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("zip"))
                    .unwrap_or(false);
            if !is_dir && !is_zip {
                continue;
            }
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                let name_lower = name.to_lowercase();
                if name_lower.contains("lovely")
                    || name_lower.starts_with('.')
                    || name_lower == ".git"
                    || name_lower == "node_modules"
                    || name_lower == "__macosx"
                {
                    continue;
                }
                // mix in name hash
                for b in name_lower.as_bytes() {
                    sum = sum.wrapping_mul(1099511628211).wrapping_add(*b as u64);
                }
                // and mtime
                sum = sum
                    .wrapping_mul(1099511628211)
                    .wrapping_add(get_dir_mtime(&p));
            }
        }
    }
    ScanFingerprint {
        mods_dir: normalize_path(&canonicalize_best_effort(mods_dir)),
        checksum: sum,
    }
}

#[cfg(target_os = "linux")]
fn is_proton_mods_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    path_str.contains("compatdata/2379780")
        && (path_str.contains("/Balatro/Mods") || path_str.ends_with("/Balatro/Mods"))
}

/// Finds the steamapps root directory from a game path.
/// Returns None if the path doesn't follow the expected Steam directory structure.
#[cfg(target_os = "linux")]
fn find_steamapps_root(game_path: &Path) -> Option<PathBuf> {
    let common_dir = game_path.parent()?;
    if common_dir.file_name().and_then(|n| n.to_str()) != Some("common") {
        return None;
    }
    let steamapps_dir = common_dir.parent()?;
    if steamapps_dir.file_name().and_then(|n| n.to_str()) != Some("steamapps") {
        return None;
    }
    Some(steamapps_dir.to_path_buf())
}

/// Finds the Steam library where Balatro is actually installed.
/// This queries the cached Balatro paths and returns the steamapps directory
/// for the first valid installation found.
#[cfg(target_os = "linux")]
fn find_balatro_steam_library() -> Option<PathBuf> {
    let balatro_paths = crate::finder::get_balatro_paths_cached();
    for path in balatro_paths {
        if let Some(steamapps) = find_steamapps_root(&path) {
            info!(
                "Found Balatro in Steam library: {} (from game path: {})",
                steamapps.display(),
                path.display()
            );
            return Some(steamapps);
        }
    }
    None
}

/// Syncs individual mod folders from host mods directory to Proton's compat mods directory
/// when the compat directory already exists as a real directory (not a symlink).
#[cfg(target_os = "linux")]
fn sync_proton_mods(host_mods: &Path, compat_mods: &Path) -> Result<(), String> {
    if !host_mods.exists() {
        if let Err(e) = fs::create_dir_all(host_mods) {
            warn!(
                "Failed to create host mods dir {}: {}",
                host_mods.display(),
                e
            );
        }
        return Ok(());
    }

    if !compat_mods.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(host_mods).map_err(|e| {
        format!(
            "Failed to read host mods dir {}: {}",
            host_mods.display(),
            e
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = match path.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dest = compat_mods.join(name);
        if dest.exists() {
            continue;
        }
        if let Err(e) = symlink(&path, &dest) {
            warn!(
                "Failed to link Proton mod {} -> {}: {}",
                dest.display(),
                path.display(),
                e
            );
        }
    }

    Ok(())
}

/// Flatpak-specific version of sync_proton_mods that uses flatpak-spawn to create symlinks on the host.
#[cfg(target_os = "linux")]
fn sync_proton_mods_flatpak(host_mods: &Path, compat_mods: &Path) -> Result<(), String> {
    use std::process::Command;

    // List entries in host_mods directory via flatpak-spawn
    let ls_result = Command::new("flatpak-spawn")
        .args(["--host", "ls", "-1", &host_mods.to_string_lossy()])
        .output();

    let entries = match ls_result {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        Ok(_) => {
            // Directory might not exist or is empty
            return Ok(());
        }
        Err(e) => {
            warn!("Failed to list host mods directory: {}", e);
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        let src = host_mods.join(entry);
        let dest = compat_mods.join(entry);

        // Check if source is a directory
        let is_dir = Command::new("flatpak-spawn")
            .args(["--host", "test", "-d", &src.to_string_lossy()])
            .status();

        if !matches!(is_dir, Ok(status) if status.success()) {
            continue;
        }

        // Check if destination already exists
        let dest_exists = Command::new("flatpak-spawn")
            .args(["--host", "test", "-e", &dest.to_string_lossy()])
            .status();

        if matches!(dest_exists, Ok(status) if status.success()) {
            continue;
        }

        // Create symlink from compat_mods/entry -> host_mods/entry
        let symlink_result = Command::new("flatpak-spawn")
            .args([
                "--host",
                "ln",
                "-s",
                &src.to_string_lossy(),
                &dest.to_string_lossy(),
            ])
            .output();

        match symlink_result {
            Ok(output) if output.status.success() => {
                info!(
                    "Linked Proton mod via flatpak-spawn: {} -> {}",
                    dest.display(),
                    src.display()
                );
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(
                    "Failed to link Proton mod {} -> {}: {}",
                    dest.display(),
                    src.display(),
                    stderr.trim()
                );
            }
            Err(e) => {
                warn!(
                    "Failed to run flatpak-spawn for mod symlink {} -> {}: {}",
                    dest.display(),
                    src.display(),
                    e
                );
            }
        }
    }

    Ok(())
}

/// Ensures symlinks exist so that mods installed to the host data directory
/// are visible to Proton/Wine when running Balatro via Steam.
///
/// This creates a symlink from the Proton prefix mods directory to the host mods directory:
/// `~/.local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods`
/// -> `~/.local/share/Balatro/Mods`
///
/// If the Proton directory already exists as a real directory (not a symlink),
/// it syncs individual mod folders instead.
///
/// # Arguments
/// * `game_path` - Optional path to the Balatro game installation. If provided, the function
///   will try to derive the steamapps directory from it.
#[cfg(target_os = "linux")]
pub fn ensure_proton_mod_dir_link(game_path: Option<&Path>) -> Result<(), String> {
    let is_flatpak = std::env::var_os("FLATPAK_ID").is_some();

    // For Flatpak, we need to use flatpak-spawn to create symlinks on the host
    if is_flatpak {
        return ensure_proton_mod_dir_link_flatpak();
    }

    let host_mods =
        resolve_mods_dir_path().map_err(|e| format!("Could not resolve mods directory: {e}"))?;

    // Find the correct Steam library where Balatro is installed
    let steamapps_dir = if let Some(game_path) = game_path {
        // When game_path is provided, derive from it only
        find_steamapps_root(game_path)
    } else {
        // When not provided, find where Balatro is actually installed
        find_balatro_steam_library()
    };

    let Some(steamapps_dir) = steamapps_dir else {
        info!("No Steam library found, skipping Proton symlink creation");
        return Ok(());
    };

    let compat_mods = steamapps_dir
        .join("compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods");

    // Verify the Steam library actually contains Balatro
    let balatro_game_path = steamapps_dir.join("common/Balatro");
    if !balatro_game_path.exists() {
        warn!(
            "Steam library {} does not contain Balatro, skipping symlink",
            steamapps_dir.display()
        );
        return Ok(());
    }

    // If the paths are the same, nothing to do
    if compat_mods == host_mods {
        return Ok(());
    }

    // Create the parent directory for the compat mods path
    if let Some(parent) = compat_mods.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return Err(format!(
            "Failed to create Proton mods parent {}: {}",
            parent.display(),
            e
        ));
    }

    // Check if compat_mods already exists
    if compat_mods.exists() {
        if compat_mods.is_symlink() {
            // Already a symlink, we're good
            return Ok(());
        }
        // It's a real directory, sync individual mods
        warn!(
            "Proton Mods path already exists and is not a symlink: {}",
            compat_mods.display()
        );
        sync_proton_mods(&host_mods, &compat_mods)?;
        return Ok(());
    }

    // Create the host mods directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&host_mods) {
        warn!(
            "Failed to create host mods dir {}: {}",
            host_mods.display(),
            e
        );
    }

    // Create the symlink
    symlink(&host_mods, &compat_mods).map_err(|e| {
        format!(
            "Failed to link Proton mods dir {} -> {}: {}",
            compat_mods.display(),
            host_mods.display(),
            e
        )
    })?;

    info!(
        "Linked Proton mods dir to host: {} -> {}",
        compat_mods.display(),
        host_mods.display()
    );

    Ok(())
}

/// Flatpak-specific implementation that uses flatpak-spawn to create symlinks on the host.
/// This is needed because the Flatpak sandbox cannot directly access the Steam/Proton prefix.
#[cfg(target_os = "linux")]
fn ensure_proton_mod_dir_link_flatpak() -> Result<(), String> {
    use std::process::Command;

    let Some(home) = dirs::home_dir() else {
        return Err("Could not determine home directory".to_string());
    };

    // The host mods directory (Lovely 0.9.0+ location)
    let host_mods = home.join(".local/share/Balatro/Mods");

    // Common Steam library locations to check
    let mut steam_locations = vec![
        home.join(".local/share/Steam/steamapps"),
        home.join(".steam/steam/steamapps"),
        home.join(".steam/root/steamapps"),
        home.join(".steam/debian-installation/steamapps"),
    ];

    // Also check for SD card / external drive locations (common on Steam Deck)
    // These are typically mounted at /run/media/<user>/<volume>/
    if let Ok(user) = std::env::var("USER") {
        let media_base = PathBuf::from("/run/media").join(&user);

        // Use flatpak-spawn to list mounted volumes
        let ls_result = Command::new("flatpak-spawn")
            .args(["--host", "ls", &media_base.to_string_lossy()])
            .output();

        if let Ok(output) = ls_result
            && output.status.success()
        {
            let volumes = String::from_utf8_lossy(&output.stdout);
            for volume in volumes.lines() {
                let volume = volume.trim();
                if volume.is_empty() {
                    continue;
                }
                // Check common Steam library paths on external drives
                steam_locations.push(media_base.join(volume).join("steamapps"));
                steam_locations.push(media_base.join(volume).join("SteamLibrary/steamapps"));
            }
        }
    }

    // Find which Steam location has Balatro installed
    let mut compat_mods_path: Option<PathBuf> = None;
    for steamapps in &steam_locations {
        let balatro_path = steamapps.join("common/Balatro");
        let compat_path = steamapps
            .join("compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods");

        // Check if Balatro exists at this location using flatpak-spawn
        let check_result = Command::new("flatpak-spawn")
            .args(["--host", "test", "-d", &balatro_path.to_string_lossy()])
            .status();

        if let Ok(status) = check_result
            && status.success()
        {
            compat_mods_path = Some(compat_path);
            info!("Found Balatro installation at {}", steamapps.display());
            break;
        }
    }

    let Some(compat_mods) = compat_mods_path else {
        info!("No Steam Balatro installation found from Flatpak, skipping Proton symlink");
        return Ok(());
    };

    // Check if symlink already exists
    let check_symlink = Command::new("flatpak-spawn")
        .args(["--host", "test", "-L", &compat_mods.to_string_lossy()])
        .status();

    if let Ok(status) = check_symlink
        && status.success()
    {
        info!("Proton symlink already exists: {}", compat_mods.display());
        return Ok(());
    }

    // Check if it exists as a regular directory
    let check_dir = Command::new("flatpak-spawn")
        .args(["--host", "test", "-d", &compat_mods.to_string_lossy()])
        .status();

    if let Ok(status) = check_dir
        && status.success()
    {
        warn!(
            "Proton Mods path exists as directory, not creating symlink: {}",
            compat_mods.display()
        );
        // Sync individual mod folders instead
        sync_proton_mods_flatpak(&host_mods, &compat_mods)?;
        return Ok(());
    }

    // Create the host mods directory if it doesn't exist
    let mkdir_result = Command::new("flatpak-spawn")
        .args(["--host", "mkdir", "-p", &host_mods.to_string_lossy()])
        .status();

    if let Err(e) = mkdir_result {
        warn!("Failed to create host mods directory: {}", e);
    }

    // Create parent directory for the compat mods path
    if let Some(parent) = compat_mods.parent() {
        let mkdir_parent = Command::new("flatpak-spawn")
            .args(["--host", "mkdir", "-p", &parent.to_string_lossy()])
            .status();

        if let Err(e) = mkdir_parent {
            return Err(format!(
                "Failed to create Proton mods parent directory: {}",
                e
            ));
        }
    }

    // Create the symlink using flatpak-spawn
    let symlink_result = Command::new("flatpak-spawn")
        .args([
            "--host",
            "ln",
            "-s",
            &host_mods.to_string_lossy(),
            &compat_mods.to_string_lossy(),
        ])
        .output();

    match symlink_result {
        Ok(output) => {
            if output.status.success() {
                info!(
                    "Created Proton symlink via flatpak-spawn: {} -> {}",
                    compat_mods.display(),
                    host_mods.display()
                );
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!(
                    "Failed to create Proton symlink: {}",
                    stderr.trim()
                ))
            }
        }
        Err(e) => Err(format!("Failed to run flatpak-spawn for symlink: {}", e)),
    }
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn ensure_proton_mod_dir_link(_game_path: Option<&Path>) -> Result<(), String> {
    Ok(())
}

/// Migrates mods from the legacy ~/.config/Balatro/Mods location to the new
/// ~/.local/share/Balatro/Mods location used by Lovely 0.9.0+.
///
/// Returns Ok(true) if migration was performed, Ok(false) if no migration needed.
#[cfg(target_os = "linux")]
pub fn migrate_legacy_mods_dir() -> Result<bool, String> {
    let Some(home) = dirs::home_dir() else {
        return Ok(false);
    };

    let legacy_mods = home.join(".config/Balatro/Mods");
    let new_mods = home.join(".local/share/Balatro/Mods");

    // No migration needed if legacy doesn't exist or is a symlink
    if !legacy_mods.exists() || legacy_mods.is_symlink() {
        return Ok(false);
    }

    // Check if legacy has any mods
    let legacy_entries: Vec<_> = fs::read_dir(&legacy_mods)
        .map_err(|e| format!("Failed to read legacy mods dir: {e}"))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .collect();

    if legacy_entries.is_empty() {
        return Ok(false);
    }

    info!(
        "Found {} mods in legacy location {}, migrating to {}",
        legacy_entries.len(),
        legacy_mods.display(),
        new_mods.display()
    );

    // Create new mods directory
    fs::create_dir_all(&new_mods)
        .map_err(|e| format!("Failed to create new mods dir {}: {e}", new_mods.display()))?;

    // Move each mod folder to new location
    let mut migrated = 0;
    for entry in legacy_entries {
        let src = entry.path();
        let name = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dest = new_mods.join(name);

        // Skip if destination already exists
        if dest.exists() {
            warn!(
                "Skipping migration of {}: already exists in new location",
                name.to_string_lossy()
            );
            continue;
        }

        // Try rename first (fast, same filesystem)
        if fs::rename(&src, &dest).is_ok() {
            info!("Migrated mod: {}", name.to_string_lossy());
            migrated += 1;
            continue;
        }

        // Fall back to copy + delete for cross-filesystem
        if let Err(e) = copy_dir_recursive(&src, &dest) {
            warn!("Failed to migrate {}: {e}", name.to_string_lossy());
            continue;
        }
        if let Err(e) = fs::remove_dir_all(&src) {
            warn!(
                "Failed to remove old mod dir after copy {}: {e}",
                src.display()
            );
        }
        info!("Migrated mod (copy): {}", name.to_string_lossy());
        migrated += 1;
    }

    // Create symlink from old location to new for backwards compatibility
    // (in case user has other tools pointing to old location)
    if migrated > 0 {
        // Remove empty legacy dir and replace with symlink
        let legacy_is_empty = fs::read_dir(&legacy_mods)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);

        if legacy_is_empty {
            if let Err(e) = fs::remove_dir(&legacy_mods) {
                warn!("Failed to remove empty legacy mods dir: {e}");
            } else if let Err(e) = symlink(&new_mods, &legacy_mods) {
                warn!("Failed to create backwards-compat symlink: {e}");
            } else {
                info!(
                    "Created symlink for backwards compatibility: {} -> {}",
                    legacy_mods.display(),
                    new_mods.display()
                );
            }
        }
    }

    info!("Migration complete: {} mods moved", migrated);
    Ok(migrated > 0)
}

/// Recursively copy a directory.
#[cfg(target_os = "linux")]
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create dir {}: {e}", dest.display()))?;

    for entry in
        fs::read_dir(src).map_err(|e| format!("Failed to read dir {}: {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("Failed to copy {}: {e}", src_path.display()))?;
        }
    }
    Ok(())
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn migrate_legacy_mods_dir() -> Result<bool, String> {
    Ok(false)
}

pub fn mod_dir_candidates() -> Result<Vec<PathBuf>, String> {
    // Lovely 0.9.0+ uses XDG data directory (~/.local/share/Balatro/Mods) on Linux native
    #[cfg(target_os = "linux")]
    let primary = dirs::data_dir()
        .ok_or_else(|| "Could not find data directory".to_string())?
        .join("Balatro")
        .join("Mods");

    #[cfg(not(target_os = "linux"))]
    let primary = dirs::config_dir()
        .ok_or_else(|| "Could not find config directory".to_string())?
        .join("Balatro")
        .join("Mods");

    let mut candidates = Vec::new();
    let is_flatpak = std::env::var_os("FLATPAK_ID").is_some();

    #[cfg(target_os = "linux")]
    {
        // Prefer Proton's compatdata mods folder when available (Steam install).
        if !is_flatpak {
            let mut compat_candidates = Vec::new();
            for balatro_path in finder::get_balatro_paths_cached() {
                if let Some(steamapps) = balatro_path.parent().and_then(|p| p.parent()) {
                    compat_candidates.push(steamapps.join(
                        "compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods",
                    ));
                }
            }
            if compat_candidates.is_empty()
                && let Some(home) = dirs::home_dir()
            {
                compat_candidates.push(home.join(
                    ".local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods",
                ));
                compat_candidates.push(home.join(
                    ".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/compatdata/2379780/pfx/drive_c/users/steamuser/AppData/Roaming/Balatro/Mods",
                ));
            }
            candidates.extend(compat_candidates);
        }
    }

    // In Flatpak, prefer the host data path first so we open the real mods folder.
    if is_flatpak && let Some(home) = dirs::home_dir() {
        // New Lovely 0.9.0+ location (XDG data dir)
        candidates.push(
            home.join(".local")
                .join("share")
                .join("Balatro")
                .join("Mods"),
        );
        // Legacy location for migration (Lovely < 0.9.0)
        candidates.push(home.join(".config").join("Balatro").join("Mods"));
    }

    candidates.push(primary.clone());

    // Host XDG_DATA_HOME (Lovely 0.9.0+ location)
    if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        let xdg_path = PathBuf::from(xdg).join("Balatro").join("Mods");
        if xdg_path != primary {
            candidates.push(xdg_path);
        }
    }

    // Legacy: Host XDG_CONFIG_HOME (Lovely < 0.9.0)
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        let xdg_path = PathBuf::from(xdg).join("Balatro").join("Mods");
        if xdg_path != primary {
            candidates.push(xdg_path);
        }
    }

    // Legacy: Host ~/.config (Lovely < 0.9.0)
    if let Some(home) = dirs::home_dir() {
        let host_config = home.join(".config").join("Balatro").join("Mods");
        if host_config != primary {
            candidates.push(host_config);
        }
    }

    Ok(candidates)
}

pub fn resolve_mods_dir_path() -> Result<PathBuf, String> {
    let candidates = mod_dir_candidates()?;

    #[cfg(target_os = "linux")]
    {
        let is_flatpak = std::env::var_os("FLATPAK_ID").is_some();

        if is_flatpak {
            // In Flatpak, prefer the host data path (~/.local/share/Balatro/Mods) for Lovely 0.9.0+.
            // Create it if it doesn't exist so mods are installed where Balatro
            // (running via Steam/Proton outside the sandbox) can find them.
            if let Some(home) = dirs::home_dir() {
                let host_mods = home
                    .join(".local")
                    .join("share")
                    .join("Balatro")
                    .join("Mods");
                if !host_mods.exists()
                    && let Err(e) = fs::create_dir_all(&host_mods)
                {
                    log::warn!(
                        "Failed to create host mods directory {}: {}",
                        host_mods.display(),
                        e
                    );
                }
                if host_mods.exists() {
                    return Ok(host_mods);
                }
            }
        } else {
            // Non-Flatpak Linux: prefer Proton compatdata paths if they exist
            let compat_candidates: Vec<PathBuf> = candidates
                .iter()
                .filter(|p| is_proton_mods_path(p))
                .cloned()
                .collect();
            if let Some(existing) = compat_candidates.iter().find(|p| p.exists()) {
                return Ok(existing.clone());
            }
        }
    }

    if let Some(existing) = candidates.iter().find(|p| p.exists()) {
        return Ok(existing.clone());
    }
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| "Could not determine Mods directory".to_string())
}

pub fn detect_manual_mods_cached(
    db: &Database,
    cached_catalog_mods: &[cache::Mod],
) -> Result<Vec<DetectedMod>, String> {
    let mods_dir = resolve_mods_dir_path()?;

    let fp = compute_fingerprint(&mods_dir);
    if let Ok(mut guard) = DETECTION_CACHE.lock() {
        if let Some((cached_fp, cached_at, cached_mods)) = &*guard
            && cached_fp == &fp
            && cached_at.elapsed().as_secs() < DETECTION_CACHE_TTL_SECS
        {
            return Ok(cached_mods.clone());
        }
        // Miss or expired: compute fresh
        let fresh = detect_manual_mods(db, cached_catalog_mods)?;
        *guard = Some((fp, Instant::now(), fresh.clone()));
        Ok(fresh)
    } else {
        // In the unlikely event of a poisoned mutex, fall back to direct scan
        detect_manual_mods(db, cached_catalog_mods)
    }
}

/// Clears the in-process detection cache so next call re-scans the filesystem.
pub fn clear_detection_cache() {
    if let Ok(mut guard) = DETECTION_CACHE.lock() {
        *guard = None;
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DetectedMod {
    pub name: String,
    pub id: String,
    pub author: Vec<String>,
    pub description: String,
    pub prefix: String,
    pub version: Option<String>,
    pub path: String,
    pub dependencies: Vec<String>,
    pub conflicts: Vec<String>,
    pub catalog_match: Option<CatalogMatch>,
    pub is_duplicate: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogMatch {
    pub title: String,
    pub catalog_id: String,
    pub download_url: String, // Changed from downloadURL to match field names
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ThunderstoreManifest {
    name: String,
    #[serde(rename = "version_number")]
    version_number: Option<String>,
    #[serde(rename = "website_url")]
    website_url: Option<String>,
    description: Option<String>,
    dependencies: Option<Vec<String>>,
}

// Add this function to parse Thunderstore manifest files
fn parse_thunderstore_manifest(
    manifest_path: &Path,
    mod_path: &Path,
) -> Result<Option<DetectedMod>, String> {
    let file = match File::open(manifest_path) {
        Ok(file) => file,
        Err(e) => {
            log::debug!(
                "Failed to open manifest file {}: {}",
                manifest_path.display(),
                e
            );
            return Ok(None);
        }
    };

    let manifest: ThunderstoreManifest = match serde_json::from_reader(file) {
        Ok(json) => json,
        Err(e) => {
            log::debug!(
                "Failed to parse manifest file {}: {}",
                manifest_path.display(),
                e
            );
            return Ok(None);
        }
    };

    // Special handling for Steamodded manifest
    if manifest.name.to_lowercase() == "steamodded" {
        return Ok(Some(DetectedMod {
            name: "Steamodded".to_string(),
            id: "Steamodded".to_string(),
            author: vec!["Steamodded Team".to_string()],
            description: manifest
                .description
                .unwrap_or_else(|| "A Balatro Modding Framework".to_string()),
            prefix: "smod".to_string(),
            version: manifest.version_number,
            path: mod_path.to_string_lossy().to_string(),
            dependencies: manifest.dependencies.unwrap_or_default(),
            conflicts: Vec::new(),
            catalog_match: None,
            is_duplicate: false,
        }));
    }

    // For other manifests, create a generic mod entry
    Ok(Some(DetectedMod {
        name: manifest.name.clone(),
        id: manifest.name.replace(" ", ""),
        author: vec!["Unknown".to_string()], // Thunderstore manifest doesn't specify authors directly
        description: manifest
            .description
            .unwrap_or_else(|| format!("Mod found in {}", mod_path.display())),
        prefix: if manifest.name.len() >= 4 {
            manifest.name[0..4].to_lowercase()
        } else {
            manifest.name.to_lowercase()
        },
        version: manifest.version_number,
        path: mod_path.to_string_lossy().to_string(),
        dependencies: manifest.dependencies.unwrap_or_default(),
        conflicts: Vec::new(),
        catalog_match: None,
        is_duplicate: false,
    }))
}

pub fn detect_manual_mods(
    db: &Database,
    cached_catalog_mods: &[cache::Mod],
) -> Result<Vec<DetectedMod>, String> {
    let mod_dir = resolve_mods_dir_path()?;

    log::debug!("Scanning for manual mods in: {:?}", mod_dir);

    if !mod_dir.exists() {
        log::debug!("Mods directory does not exist, returning empty list");
        return Ok(Vec::new());
    }

    // Get tracked mods from the database for duplicate detection
    let managed_mods = db
        .get_installed_mods()
        .map_err(|e| format!("Failed to get installed mods: {e}"))?;

    // Create a set of normalized managed mod paths for quick lookup
    let managed_paths: HashSet<String> = managed_mods
        .iter()
        .map(|m| normalize_path(&canonicalize_best_effort(&PathBuf::from(&m.path))))
        .collect();

    // Create a set of managed mod names (lowercase) for duplicate detection
    let managed_names: HashSet<String> =
        managed_mods.iter().map(|m| m.name.to_lowercase()).collect();
    let managed_catalog_names = managed_names.clone();

    let mut manual_mods = Vec::new();
    let mut bundled_dependencies = HashSet::new();

    // Find bundled dependencies in mod packages
    find_bundled_dependencies(&mod_dir, &mod_dir, 0, &mut bundled_dependencies)?;
    log::debug!(
        "Found {} bundled dependencies in mod packages",
        bundled_dependencies.len()
    );

    // Detect mods from filesystem
    let mut all_detected_mods = Vec::new();
    detect_mods_recursive(
        &mod_dir,
        &mod_dir,
        0,
        &mut all_detected_mods,
        &bundled_dependencies,
    )?;
    log::debug!("Detected {} mods before filtering", all_detected_mods.len());

    // Detect Talisman installed at Balatro root (outside Mods)
    for install_path in finder::get_balatro_paths_cached() {
        let talisman_path = install_path.join("Talisman");
        if talisman_path.exists() && talisman_path.is_dir() {
            let mut talisman = DetectedMod {
                name: "Talisman".to_string(),
                id: "Talisman".to_string(),
                author: vec!["Talisman".to_string()],
                description: "Balatro mod loader".to_string(),
                prefix: "tali".to_string(),
                version: None,
                path: talisman_path.to_string_lossy().to_string(),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                catalog_match: None,
                is_duplicate: false,
            };
            let mod_name_lower = talisman.name.to_lowercase();
            if managed_names.contains(&mod_name_lower) {
                talisman.is_duplicate = true;
                talisman.name = format!("{} (Manual)", talisman.name);
            }
            // Catalog match will be done below with the index
            all_detected_mods.push(talisman);
        }
    }

    // Build catalog index once for O(1) lookups instead of O(n) per mod
    let catalog_index = CatalogIndex::new(cached_catalog_mods);

    // Process detected mods to find catalog matches and handle duplicates
    let total_detected = all_detected_mods.len();
    for mut mod_info in all_detected_mods {
        let mod_path = normalize_path(&canonicalize_best_effort(&PathBuf::from(&mod_info.path)));

        // If this mod is not managed by path, consider it a manual mod
        if !is_path_managed(&mod_path, &managed_paths) {
            // Check for name duplication with managed mods
            let mod_name_lower = mod_info.name.to_lowercase();
            if managed_names.contains(&mod_name_lower) {
                mod_info.is_duplicate = true;
                // Append a suffix to the name
                mod_info.name = format!("{} (Manual)", mod_info.name);
            }

            // Try to find a match in the catalog using indexed lookup
            mod_info.catalog_match = find_catalog_match_indexed(&mod_info, &catalog_index);

            // If this mod matches a catalog entry that is already installed, skip it to avoid duplicates
            if let Some(cat) = &mod_info.catalog_match
                && managed_catalog_names.contains(&cat.title.to_lowercase())
            {
                continue;
            }

            manual_mods.push(mod_info);
        }
    }

    log::info!(
        "Found {} manual mods after filtering (excluded {} managed mods)",
        manual_mods.len(),
        total_detected - manual_mods.len()
    );

    Ok(manual_mods)
}

fn scan_for_json_files(dir_path: &Path) -> Result<Vec<PathBuf>, String> {
    // Prefer likely config filenames first; reduces noise and speeds scanning
    let preferred = [
        "mod.json",
        "main.json",
        "info.json",
        "config.json",
        // manifest.json is handled separately but keep as fallback
        "manifest.json",
    ];

    let mut preferred_files = Vec::new();
    let mut fallback_json = Vec::new();

    let entries = fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory {}: {}", dir_path.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && preferred.iter().any(|p| name.eq_ignore_ascii_case(p))
        {
            preferred_files.push(path.clone());
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            fallback_json.push(path);
        }
    }

    if !preferred_files.is_empty() {
        Ok(preferred_files)
    } else {
        Ok(fallback_json)
    }
}

/// Pre-computed lookup index for catalog matching
struct CatalogIndex<'a> {
    /// Map from lowercase title to catalog mod
    by_title_lower: HashMap<String, &'a cache::Mod>,
    /// Map from compact ID (no spaces, lowercase) to catalog mod
    by_id_lower: HashMap<String, &'a cache::Mod>,
    /// Map from compacted name (alphanumeric only) to catalog mod
    by_compact: HashMap<String, &'a cache::Mod>,
    /// All catalog mods for similarity fallback
    all_mods: &'a [cache::Mod],
}

impl<'a> CatalogIndex<'a> {
    fn new(catalog_mods: &'a [cache::Mod]) -> Self {
        let mut by_title_lower = HashMap::with_capacity(catalog_mods.len());
        let mut by_id_lower = HashMap::with_capacity(catalog_mods.len());
        let mut by_compact = HashMap::with_capacity(catalog_mods.len());

        for catalog_mod in catalog_mods {
            let title_lower = catalog_mod.title.to_lowercase();
            let id_lower = catalog_mod.title.replace(' ', "").to_lowercase();
            let title_compact = compact(&title_lower);

            by_title_lower.insert(title_lower, catalog_mod);
            by_id_lower.insert(id_lower, catalog_mod);
            by_compact.insert(title_compact, catalog_mod);
        }

        Self {
            by_title_lower,
            by_id_lower,
            by_compact,
            all_mods: catalog_mods,
        }
    }
}

fn find_catalog_match_indexed(
    local_mod: &DetectedMod,
    index: &CatalogIndex,
) -> Option<CatalogMatch> {
    let local_id_lower = local_mod.id.to_lowercase();
    let local_name_lower = local_mod.name.to_lowercase();
    let local_id_compact = compact(&local_id_lower);
    let local_name_compact = compact(&local_name_lower);

    // Get directory name for additional checking
    let dir_name_lower = Path::new(&local_mod.path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Enhanced Steamodded detection
    let is_steamodded = local_id_lower == "steamodded"
        || local_name_lower == "steamodded"
        || local_id_lower.contains("steamodded")
        || local_name_lower.contains("steamodded")
        || local_id_lower == "smods"
        || local_name_lower == "smods"
        || dir_name_lower.starts_with("smods")
        || dir_name_lower.contains("steamodded");

    if is_steamodded && let Some(catalog_mod) = index.by_title_lower.get("steamodded") {
        return Some(create_match(catalog_mod));
    }

    // Special case for Talisman
    let is_talisman = local_id_lower == "talisman"
        || local_name_lower == "talisman"
        || local_id_lower.contains("talisman")
        || local_name_lower.contains("talisman");

    if is_talisman && let Some(catalog_mod) = index.by_title_lower.get("talisman") {
        return Some(create_match(catalog_mod));
    }

    // 1. Try exact ID match (O(1) lookup)
    if let Some(catalog_mod) = index.by_id_lower.get(&local_id_lower) {
        return Some(create_match(catalog_mod));
    }

    // 2. Try exact name match (O(1) lookup)
    if let Some(catalog_mod) = index.by_title_lower.get(&local_name_lower) {
        return Some(create_match(catalog_mod));
    }

    // 3. Try directory name match (O(1) lookup)
    if !dir_name_lower.is_empty()
        && let Some(catalog_mod) = index.by_title_lower.get(&dir_name_lower)
    {
        return Some(create_match(catalog_mod));
    }

    // 4. Try compacted name match (O(1) lookup)
    if let Some(catalog_mod) = index.by_compact.get(&local_id_compact) {
        return Some(create_match(catalog_mod));
    }
    if let Some(catalog_mod) = index.by_compact.get(&local_name_compact) {
        return Some(create_match(catalog_mod));
    }

    // 5. Fallback to similarity matching (still O(n) but only for unmatched mods)
    for catalog_mod in index.all_mods {
        let catalog_name_lower = catalog_mod.title.to_lowercase();
        let catalog_name_compact = compact(&catalog_name_lower);

        if is_similar(&local_name_compact, &catalog_name_compact)
            || is_similar(&local_id_compact, &catalog_name_compact)
        {
            return Some(create_match(catalog_mod));
        }
    }

    None
}

// Helper function to create a catalog match object
fn create_match(catalog_mod: &cache::Mod) -> CatalogMatch {
    CatalogMatch {
        title: catalog_mod.title.clone(),
        catalog_id: catalog_mod.title.clone(),
        download_url: catalog_mod.download_url.clone(),
        version: catalog_mod.version.clone(),
    }
}

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
}

// Helper function to determine if two strings are similar enough
fn is_similar(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }

    if a == b {
        return true;
    }

    let distance = calculate_edit_distance(a, b);
    let max_len = a.len().max(b.len()) as f32;
    let min_len = a.len().min(b.len());

    // Only allow single typo tolerance
    if distance <= 1 {
        return true;
    }

    // For similarity matching, require:
    // 1. Higher similarity threshold (0.85 instead of 0.82)
    // 2. Maximum edit distance of 2
    // 3. Strings must be at least 6 characters to avoid short-string false positives
    // 4. Length difference shouldn't be more than 2 characters
    let similarity = 1.0 - (distance as f32 / max_len);
    let len_diff = (a.len() as i32 - b.len() as i32).abs();

    similarity >= 0.85 && distance <= 2 && min_len >= 6 && len_diff <= 2
}

// Calculate Levenshtein distance between two strings
fn calculate_edit_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    let m = s1_chars.len();
    let n = s2_chars.len();

    // Handle edge cases
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Create a matrix of size (m+1) x (n+1)
    let mut matrix = vec![vec![0; n + 1]; m + 1];

    // Initialize first column
    for (i, row) in matrix.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }

    // Initialize first row
    for (j, cell) in matrix[0].iter_mut().enumerate().take(n + 1) {
        *cell = j;
    }

    // Fill in the rest of the matrix
    for i in 1..=m {
        for j in 1..=n {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };

            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1, // deletion
                    matrix[i][j - 1] + 1, // insertion
                ),
                matrix[i - 1][j - 1] + cost, // substitution
            );
        }
    }

    matrix[m][n]
}

fn is_path_managed(path: &str, managed_paths: &HashSet<String>) -> bool {
    // Direct path match
    if managed_paths.contains(path) {
        return true;
    }

    // Check if this path is a subdirectory of a managed path
    for managed_path in managed_paths {
        if path.starts_with(managed_path) {
            return true;
        }
    }

    // Check if a managed path is a subdirectory of this path
    for managed_path in managed_paths {
        if managed_path.starts_with(path) {
            return true;
        }
    }

    false
}
fn find_bundled_dependencies(
    dir: &Path,
    _root: &Path,
    depth: usize,
    bundled_deps: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Skip lovely-related and hidden/noisy directories
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let lower_name = file_name.to_lowercase();
            if lower_name.contains("lovely") || lower_name == "bmm-compat" {
                continue;
            }
            if lower_name.starts_with('.')
                || lower_name == ".git"
                || lower_name == "node_modules"
                || lower_name == "__macosx"
            {
                continue;
            }
        }

        // Check if this directory contains a "Mods" subdirectory
        let mods_subdir = path.join("Mods");
        if mods_subdir.exists() && mods_subdir.is_dir() {
            // This is likely a mod package with bundled dependencies
            // Mark all mods in the Mods subdirectory as bundled dependencies
            mark_bundled_dependencies(&mods_subdir, bundled_deps)?;
        }

        // Recursively check subdirectories (limited depth from root)
        const MAX_DEPTH_BUNDLED: usize = 3;
        if depth < MAX_DEPTH_BUNDLED {
            find_bundled_dependencies(&path, _root, depth + 1, bundled_deps)?;
        }
    }

    Ok(())
}

/// Mark all mods in a Mods subdirectory as bundled dependencies
fn mark_bundled_dependencies(
    mods_dir: &Path,
    bundled_deps: &mut HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(mods_dir)
        .map_err(|e| format!("Failed to read directory {}: {}", mods_dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            // Add this dependency's normalized path to our set
            let normalized_path = normalize_path(&canonicalize_best_effort(&path));
            bundled_deps.insert(normalized_path);

            // Log for debugging
            log::debug!("Found bundled dependency: {}", path.display());
        }
    }

    Ok(())
}

/// Recursively scan for mods in directories
fn detect_mods_recursive(
    dir: &Path,
    _root: &Path,
    depth: usize,
    detected_mods: &mut Vec<DetectedMod>,
    bundled_deps: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        // Detect zipped mods (Steamodded loads `.zip` archives without extracting).
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("zip"))
                .unwrap_or(false)
        {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let lower = file_name.to_lowercase();
                // Skip Lovely's own archive and hidden/system files.
                if lower.contains("lovely") || lower.starts_with('.') {
                    continue;
                }
            }
            if let Some(detected_mod) = detect_mod_in_zip(&path)? {
                detected_mods.push(detected_mod);
            }
            continue;
        }

        if !path.is_dir() {
            continue;
        }

        // Skip lovely-related directories
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let lower_name = file_name.to_lowercase();
            if lower_name.contains("lovely") || lower_name == "bmm-compat" {
                continue;
            }
            // Skip hidden/system/noisy dirs
            if lower_name.starts_with('.')
                || lower_name == ".git"
                || lower_name == "node_modules"
                || lower_name == "__macosx"
            {
                continue;
            }
        }

        // Skip bundled dependencies
        let normalized_path = normalize_path(&canonicalize_best_effort(&path));
        if bundled_deps.contains(&normalized_path) {
            log::debug!("Skipping bundled dependency: {}", path.display());
            continue;
        }

        // Check if this directory is a mod
        if let Some(detected_mod) = detect_mod_in_directory(&path)? {
            detected_mods.push(detected_mod);
            continue;
        }

        // If this is a "Mods" directory, recursively scan it
        if path.file_name().and_then(|n| n.to_str()) == Some("Mods") {
            detect_mods_recursive(&path, _root, depth + 1, detected_mods, bundled_deps)?;
            continue;
        }

        // Regular directory, recursively scan up to MAX_DEPTH from root
        const MAX_DEPTH: usize = 2;
        if depth < MAX_DEPTH {
            detect_mods_recursive(&path, _root, depth + 1, detected_mods, bundled_deps)?;
        }
    }

    Ok(())
}

/// Normalize path for case-insensitive comparison on Windows
fn normalize_path(path: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        path.to_string_lossy().to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_string_lossy().to_string()
    }
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn detect_mod_in_directory(mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    // Get directory name
    let dir_name = mod_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid directory name: {}", mod_path.display()))?;

    // Check for Thunderstore manifest.json first
    let manifest_path = mod_path.join("manifest.json");
    if manifest_path.exists()
        && let Some(detected_mod) = parse_thunderstore_manifest(&manifest_path, mod_path)?
    {
        // If this is Steamodded, return it immediately
        if detected_mod.name.to_lowercase() == "steamodded" {
            return Ok(Some(detected_mod));
        }

        // For other mods, we'll store it and continue checking other formats
        // in case there's a more detailed mod definition
        let thunderstore_mod = detected_mod;

        // Check for other JSON files that might have more information
        let json_files = scan_for_json_files(mod_path)?;
        for json_path in &json_files {
            // Skip the manifest we already processed
            if json_path == &manifest_path {
                continue;
            }

            if let Some(detected_mod) = parse_mod_json(json_path, mod_path)? {
                return Ok(Some(detected_mod));
            }
        }

        // If we didn't find a better mod definition, use the Thunderstore one
        return Ok(Some(thunderstore_mod));
    }

    // Special handling for Steamodded with various folder names
    let dir_name_lower = dir_name.to_lowercase();
    if dir_name_lower == "steamodded" || 
       dir_name_lower == "smods" || 
       dir_name_lower == "smods_main" ||
       dir_name_lower.starts_with("smods-") ||  // Catch version-specific folders
       dir_name_lower.contains("steamodded")
    {
        // Check for any JSON/Lua files that might confirm this is Steamodded
        if is_likely_steamodded(mod_path)? {
            // Set up a basic Steamodded detected mod
            return Ok(Some(DetectedMod {
                name: "Steamodded".to_string(),
                id: "Steamodded".to_string(),
                author: vec!["Steamodded Team".to_string()],
                description: "Balatro Mod Loader".to_string(),
                prefix: "smod".to_string(),
                version: None, // Version will be filled from catalog match if available
                path: mod_path.to_string_lossy().to_string(),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                catalog_match: None,
                is_duplicate: false,
            }));
        }
    }

    // Continue with regular detection...
    // Scan for JSON files and check if any of them are valid mod configs
    let json_files = scan_for_json_files(mod_path)?;
    for json_path in json_files {
        if let Some(detected_mod) = parse_mod_json(&json_path, mod_path)? {
            return Ok(Some(detected_mod));
        }
    }

    // Look for any Lua file with the same name as the directory
    let lua_path = mod_path.join(format!("{dir_name}.lua"));
    if lua_path.exists()
        && let Some(detected_mod) = parse_mod_lua_header(&lua_path, mod_path)?
    {
        return Ok(Some(detected_mod));
    }

    // Special handling for mod packages that have a structure like:
    // ModName/Mods/ModName/ModName.lua
    let potential_mod_dir = mod_path.join("Mods").join(dir_name);
    let potential_lua_path = potential_mod_dir.join(format!("{dir_name}.lua"));

    if potential_lua_path.exists()
        && let Some(detected_mod) = parse_mod_lua_header(&potential_lua_path, mod_path)?
    {
        return Ok(Some(detected_mod));
    }

    // If we have a Mods directory with content, this might be a mod package
    let mods_dir = mod_path.join("Mods");
    if mods_dir.exists() && mods_dir.is_dir() {
        // Look for a README.md or similar to infer the mod name
        let readme_path = mod_path.join("README.md");
        let readme_alt_path = mod_path.join("README.MD");

        if readme_path.exists() || readme_alt_path.exists() {
            // This looks like a mod package - create a mod entry for it
            return Ok(Some(DetectedMod {
                name: dir_name.to_string(),
                id: dir_name.replace(" ", ""),
                author: vec!["Unknown".to_string()],
                description: format!("Mod package found in {}", mod_path.display()),
                prefix: if dir_name.len() >= 4 {
                    dir_name[0..4].to_lowercase()
                } else {
                    dir_name.to_lowercase()
                },
                version: None,
                path: mod_path.to_string_lossy().to_string(),
                dependencies: Vec::new(),
                conflicts: Vec::new(),
                catalog_match: None,
                is_duplicate: false,
            }));
        }
    }

    // If no direct match found, check all Lua files in the directory
    for entry in fs::read_dir(mod_path)
        .map_err(|e| format!("Failed to read mod directory {}: {}", mod_path.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some("lua")
            && let Some(detected_mod) = parse_mod_lua_header(&path, mod_path)?
        {
            return Ok(Some(detected_mod));
        }
    }

    // No mod configuration found
    Ok(None)
}

// Helper function to check if a directory is likely to be Steamodded
fn is_likely_steamodded(path: &Path) -> Result<bool, String> {
    // Look for typical Steamodded files
    let steamodded_indicators = [
        "api.lua",
        "smods.lua",
        "loader.lua",
        "init.lua",
        "manifest.json",
    ];

    for indicator in &steamodded_indicators {
        if path.join(indicator).exists() {
            return Ok(true);
        }
    }

    // Check subdirectories for "localization" folder which is common in Steamodded
    if path.join("localization").exists() && path.join("localization").is_dir() {
        return Ok(true);
    }

    // Look for common Steamodded directories
    if path.join("data").exists()
        && path.join("data").is_dir()
        && path.join("lib").exists()
        && path.join("lib").is_dir()
    {
        return Ok(true);
    }

    // Not enough evidence
    Ok(false)
}

/// Deserialize a number field that may be an integer or float, coercing to i64.
/// Many mod authors write floats (e.g., -1000000.0 or 2.7e+27) where i64 is expected.
fn deserialize_lenient_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct LenientI64Visitor;

    impl<'de> Visitor<'de> for LenientI64Visitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an integer or float")
        }

        fn visit_i64<E>(self, value: i64) -> Result<i64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<i64, E>
        where
            E: de::Error,
        {
            // Clamp to i64::MAX if too large
            Ok(value.min(i64::MAX as u64) as i64)
        }

        fn visit_f64<E>(self, value: f64) -> Result<i64, E>
        where
            E: de::Error,
        {
            // Clamp to i64 range and convert
            if value >= i64::MAX as f64 {
                Ok(i64::MAX)
            } else if value <= i64::MIN as f64 {
                Ok(i64::MIN)
            } else {
                Ok(value as i64)
            }
        }
    }

    deserializer.deserialize_any(LenientI64Visitor)
}

/// JSON schema for mod configuration
#[derive(Debug, Serialize, Deserialize)]
struct ModJson {
    id: String,
    name: String,
    #[serde(default)]
    author: AuthorField,
    description: String,
    prefix: String,
    main_file: String,
    #[serde(default, deserialize_with = "deserialize_lenient_i64")]
    priority: i64,
    #[serde(default = "default_badge_color")]
    badge_colour: String,
    #[serde(default = "default_text_color")]
    badge_text_colour: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
    #[serde(default)]
    provides: Vec<String>,
    #[serde(default)]
    dump_loc: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum AuthorField {
    String(String),
    Array(Vec<String>),
}

impl Default for AuthorField {
    fn default() -> Self {
        AuthorField::Array(Vec::new())
    }
}

fn default_badge_color() -> String {
    "666665".to_string()
}

fn default_text_color() -> String {
    "FFFFFF".to_string()
}

/// Strip trailing commas from JSON content to handle lenient JSON authored by mod creators.
/// Matches `,` followed by optional whitespace and then `]` or `}`.
fn strip_trailing_commas(json: &str) -> String {
    // Use lazy_static regex for efficiency
    use regex::Regex;
    lazy_static::lazy_static! {
        static ref TRAILING_COMMA: Regex = Regex::new(r",(\s*[}\]])").unwrap();
    }
    TRAILING_COMMA.replace_all(json, "$1").into_owned()
}

/// Parse mod info from JSON file
fn parse_mod_json(json_path: &Path, mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    let content = match std::fs::read_to_string(json_path) {
        Ok(s) => s,
        Err(e) => {
            log::debug!("Failed to read JSON file {}: {}", json_path.display(), e);
            return Ok(None);
        }
    };

    parse_mod_json_content(&content, mod_path)
}

/// Parse mod info from raw JSON content (shared by on-disk files and zip entries).
fn parse_mod_json_content(content: &str, mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    // Strip trailing commas (common in hand-authored JSON)
    let sanitized = strip_trailing_commas(content);

    let mod_json: ModJson = match serde_json::from_str(&sanitized) {
        Ok(json) => json,
        Err(e) => {
            log::debug!("Failed to parse mod JSON in {}: {}", mod_path.display(), e);
            return Ok(None);
        }
    };

    // Check if ID is valid (not one of the disallowed values)
    let disallowed_ids = ["Steamodded", "Lovely", "Balatro"];
    if disallowed_ids.contains(&mod_json.id.as_str()) {
        log::info!("Mod {} has a disallowed ID: {}", mod_json.name, mod_json.id);
        return Ok(None);
    }

    let authors: Vec<String> = match mod_json.author {
        AuthorField::String(s) => vec![s],
        AuthorField::Array(v) => v,
    };

    Ok(Some(DetectedMod {
        name: mod_json.name,
        id: mod_json.id,
        author: authors,
        description: mod_json.description,
        prefix: mod_json.prefix,
        version: mod_json.version,
        path: mod_path.to_string_lossy().to_string(),
        dependencies: mod_json.dependencies,
        conflicts: mod_json.conflicts,
        catalog_match: None,
        is_duplicate: false,
    }))
}

fn parse_mod_lua_header(lua_path: &Path, mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    let file = match File::open(lua_path) {
        Ok(file) => file,
        Err(e) => {
            log::error!("Failed to open Lua file {}: {}", lua_path.display(), e);
            return Ok(None);
        }
    };

    // Read up to the first 20 lines using lossy UTF-8 decoding to
    // tolerate files authored with non-UTF-8 encodings (e.g., CP-1252).
    let mut reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    for _ in 0..20 {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break, // EOF
            Ok(_) => {
                // Convert bytes to string lossily and trim newline characters
                let mut s = String::from_utf8_lossy(&buf).into_owned();
                if s.ends_with('\n') {
                    s.pop();
                    if s.ends_with('\r') {
                        s.pop();
                    }
                }
                lines.push(s);
            }
            Err(e) => {
                // Do not fail mod detection due to encoding/IO hiccup; log and stop scanning
                log::warn!(
                    "Failed to read line lossily from {}: {}",
                    lua_path.display(),
                    e
                );
                break;
            }
        }
    }

    if lines.is_empty() {
        return Ok(None);
    }

    match parse_steamodded_header_lines(&lines) {
        Some(fields) => {
            // Header present: fall back to the Lua file's stem for missing fields.
            let name_hint = lua_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            Ok(Some(finalize_lua_mod(
                fields,
                &mod_path.to_string_lossy(),
                name_hint,
            )))
        }
        None => {
            // No header: infer mod info from the directory name.
            if let Some(mod_name) = mod_path.file_name().and_then(|n| n.to_str()) {
                return Ok(Some(DetectedMod {
                    name: mod_name.to_string(),
                    id: mod_name.to_string().replace(" ", ""),
                    author: vec!["Unknown".to_string()],
                    description: format!("Local mod found in {}", mod_path.display()),
                    prefix: if mod_name.len() >= 4 {
                        mod_name[0..4].to_lowercase()
                    } else {
                        mod_name.to_lowercase()
                    },
                    version: None,
                    path: mod_path.to_string_lossy().to_string(),
                    dependencies: Vec::new(),
                    conflicts: Vec::new(),
                    catalog_match: None,
                    is_duplicate: false,
                }));
            }
            Ok(None)
        }
    }
}

/// Fields parsed from a Steamodded Lua header block.
struct LuaHeaderFields {
    name: String,
    id: String,
    author: Vec<String>,
    description: String,
    prefix: String,
    version: Option<String>,
    dependencies: Vec<String>,
    conflicts: Vec<String>,
}

/// Parse `--- KEY: value` Steamodded header lines. Returns `None` when the
/// `--- STEAMODDED HEADER` marker is absent.
fn parse_steamodded_header_lines(lines: &[String]) -> Option<LuaHeaderFields> {
    let has_header = lines
        .iter()
        .any(|line| line.trim() == "--- STEAMODDED HEADER");
    if !has_header {
        return None;
    }

    let mut fields = LuaHeaderFields {
        name: String::new(),
        id: String::new(),
        author: Vec::new(),
        description: String::new(),
        prefix: String::new(),
        version: None,
        dependencies: Vec::new(),
        conflicts: Vec::new(),
    };

    for line in lines {
        let line = line.trim();
        if !line.starts_with("---") {
            continue;
        }

        let line = &line[3..].trim();

        if let Some(value) = line.strip_prefix("MOD_NAME:") {
            fields.name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("MOD_ID:") {
            fields.id = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("MOD_AUTHOR:") {
            // Parse author list [Author1, Author2, ...] and strip quotes
            if let Some(author_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                fields.author = author_str
                    .split(',')
                    .map(|s| strip_quotes(s.trim()))
                    .collect();
            }
        } else if let Some(value) = line.strip_prefix("MOD_DESCRIPTION:") {
            fields.description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("PREFIX:") {
            fields.prefix = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("VERSION:") {
            fields.version = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("DEPENDENCIES:") {
            // Parse dependencies list
            if let Some(deps_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                fields.dependencies = deps_str
                    .split(',')
                    .map(|s| strip_quotes(s.trim()))
                    .collect();
            }
        } else if let Some(value) = line.strip_prefix("CONFLICTS:") {
            // Parse conflicts list
            if let Some(conf_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                fields.conflicts = conf_str
                    .split(',')
                    .map(|s| strip_quotes(s.trim()))
                    .collect();
            }
        }
    }

    Some(fields)
}

/// Build a `DetectedMod` from parsed header fields, inferring any missing
/// required fields from `name_hint` and using `path_str` as the mod path.
fn finalize_lua_mod(mut fields: LuaHeaderFields, path_str: &str, name_hint: &str) -> DetectedMod {
    if fields.name.is_empty() {
        fields.name = name_hint.to_string();
    }

    if fields.id.is_empty() {
        fields.id = name_hint.replace(" ", "");
    }

    if fields.author.is_empty() {
        fields.author.push("Unknown".to_string());
    }

    if fields.description.is_empty() {
        fields.description = format!("Local mod found in {path_str}");
    }

    // If prefix is empty, use first 4 letters of ID
    if fields.prefix.is_empty() && !fields.id.is_empty() {
        fields.prefix = if fields.id.len() >= 4 {
            fields.id[0..4].to_lowercase()
        } else {
            fields.id.to_lowercase()
        };
    }

    DetectedMod {
        name: fields.name,
        id: fields.id,
        author: fields.author,
        description: fields.description,
        prefix: fields.prefix,
        version: fields.version,
        path: path_str.to_string(),
        dependencies: fields.dependencies,
        conflicts: fields.conflicts,
        catalog_match: None,
        is_duplicate: false,
    }
}

/// Read a single zip entry's contents as a (lossy) UTF-8 string, skipping
/// entries that are missing or unreasonably large.
fn read_zip_entry_to_string<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    use std::io::Read;
    // Guard against pathologically large metadata entries.
    const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024;
    let mut file = archive.by_name(name).ok()?;
    if file.size() > MAX_ENTRY_BYTES {
        return None;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Detect a mod packaged as a `.zip` archive placed directly in the Mods folder.
///
/// Steamodded/Lovely load such archives without extracting them, but BMM
/// previously only scanned directories, so zipped mods were invisible in the
/// manager (issue #386). We peek inside the archive for Steamodded metadata
/// (`metadata.json`/`mod.json`) or a Lua header, falling back to the archive
/// file name so the mod is still surfaced.
fn detect_mod_in_zip(zip_path: &Path) -> Result<Option<DetectedMod>, String> {
    let file = match File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            log::debug!("Failed to open zip {}: {}", zip_path.display(), e);
            return Ok(None);
        }
    };
    let mut archive = match zip::ZipArchive::new(BufReader::new(file)) {
        Ok(a) => a,
        Err(e) => {
            log::debug!("Failed to read zip {}: {}", zip_path.display(), e);
            return Ok(None);
        }
    };

    // Collect candidate metadata files inside the archive.
    let mut json_candidates: Vec<String> = Vec::new();
    let mut lua_candidates: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let name = match archive.by_index(i) {
            Ok(f) if f.is_file() => f.name().to_string(),
            _ => continue,
        };
        let base = name.rsplit('/').next().unwrap_or(&name).to_lowercase();
        if base.ends_with(".json") {
            json_candidates.push(name);
        } else if base.ends_with(".lua") {
            lua_candidates.push(name);
        }
    }

    // A zip with neither Lua nor JSON is almost certainly not a Balatro mod.
    if json_candidates.is_empty() && lua_candidates.is_empty() {
        return Ok(None);
    }

    // Prefer Steamodded metadata/config JSON, then any other JSON.
    json_candidates.sort_by_key(|n| {
        let base = n.rsplit('/').next().unwrap_or(n).to_lowercase();
        match base.as_str() {
            "metadata.json" | "mod.json" => 0,
            "manifest.json" => 2, // Thunderstore manifest: lowest priority
            _ => 1,
        }
    });

    for entry_name in &json_candidates {
        let Some(content) = read_zip_entry_to_string(&mut archive, entry_name) else {
            continue;
        };
        if let Some(detected) = parse_mod_json_content(&content, zip_path)? {
            return Ok(Some(detected));
        }
    }

    // Fall back to a Lua file carrying a Steamodded header.
    for entry_name in &lua_candidates {
        if let Some(content) = read_zip_entry_to_string(&mut archive, entry_name) {
            let lines: Vec<String> = content.lines().take(20).map(|s| s.to_string()).collect();
            if let Some(fields) = parse_steamodded_header_lines(&lines) {
                let name_hint = Path::new(entry_name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                return Ok(Some(finalize_lua_mod(
                    fields,
                    &zip_path.to_string_lossy(),
                    name_hint,
                )));
            }
        }
    }

    // Last resort: surface the mod using the archive's file name.
    let stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Mod")
        .to_string();
    Ok(Some(DetectedMod {
        name: stem.clone(),
        id: stem.replace(' ', ""),
        author: vec!["Unknown".to_string()],
        description: format!("Zipped mod found in {}", zip_path.display()),
        prefix: if stem.len() >= 4 {
            stem[0..4].to_lowercase()
        } else {
            stem.to_lowercase()
        },
        version: None,
        path: zip_path.to_string_lossy().to_string(),
        dependencies: Vec::new(),
        conflicts: Vec::new(),
        catalog_match: None,
        is_duplicate: false,
    }))
}

fn strip_quotes(s: &str) -> String {
    let mut out = s.trim().to_string();
    if (out.starts_with('"') && out.ends_with('"'))
        || (out.starts_with('\'') && out.ends_with('\''))
    {
        out.remove(0);
        out.pop();
    }
    out
}

/// Get all detected mods and mark which ones are tracked in the database
pub fn get_all_detected_mods(db: &Database) -> Result<Vec<DetectedMod>, String> {
    // Load cached catalog mods if available
    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(), // Empty vector if no cache
    };

    detect_manual_mods(db, &cached_mods)
}

/// Checks which detected mods are not already tracked in the database
pub fn get_untracked_mods(db: &Database) -> Result<Vec<DetectedMod>, String> {
    // Load cached catalog mods if available
    let cached_mods = match cache::load_cache() {
        Ok(Some((mods, _))) => mods,
        _ => Vec::new(), // Empty vector if no cache
    };

    detect_manual_mods(db, &cached_mods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::tempdir;

    fn make_catalog_mod(title: &str) -> cache::Mod {
        cache::Mod {
            title: title.into(),
            description: String::new(),
            image: String::new(),
            categories: vec![],
            colors: cache::ColorPair {
                color1: String::new(),
                color2: String::new(),
            },
            installed: false,
            requires_steamodded: false,
            requires_talisman: false,
            publisher: String::new(),
            repo: String::new(),
            download_url: String::new(),
            folderName: None,
            version: None,
        }
    }

    fn write_file(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn test_calculate_edit_distance_and_similarity() {
        assert_eq!(super::calculate_edit_distance("kitten", "sitting"), 3);
        assert_eq!(super::calculate_edit_distance("mod", "mod"), 0);
        assert_eq!(super::calculate_edit_distance("mod", "mad"), 1);

        // Similar short strings (<= 1 difference allowed)
        assert!(super::is_similar("mod", "mad"));
        assert!(!super::is_similar("mod", "maps"));
        assert!(!super::is_similar("batro", "jambatro"));
        assert!(!super::is_similar("qualatro", "furlatro"));

        // Longer strings (<= 2 differences allowed)
        assert!(super::is_similar("steamodded", "steamodddd"));
        assert!(!super::is_similar("balatro_mod", "completely_different"));
    }

    #[test]
    fn test_is_path_managed_direct_and_nested() {
        let base = "/tmp/base".to_string();
        let nested = format!("{}/child", base);
        let cousin = "/tmp/other".to_string();

        let mut managed = HashSet::new();
        managed.insert(base.clone());

        assert!(super::is_path_managed(&base, &managed));
        assert!(super::is_path_managed(&nested, &managed)); // managed parent
        assert!(!super::is_path_managed(&cousin, &managed));

        // If the managed path is nested under the given path, also true
        assert!(super::is_path_managed("/tmp", &managed));
    }

    #[test]
    fn test_find_catalog_match_including_steamodded_special_cases() {
        let catalog = vec![cache::Mod {
            title: "Steamodded".into(),
            description: "Loader".into(),
            image: "".into(),
            categories: vec![],
            colors: cache::ColorPair {
                color1: "".into(),
                color2: "".into(),
            },
            installed: false,
            requires_steamodded: false,
            requires_talisman: false,
            publisher: "".into(),
            repo: "".into(),
            download_url: "https://example/steamodded.zip".into(),
            folderName: None,
            version: Some("1.0.0".into()),
        }];

        // Various local identifiers that should resolve to Steamodded
        for (name, id, dir) in [
            ("Steamodded", "Steamodded", "Steamodded"),
            ("smods", "smods", "smods_main"),
            ("My Steamodded", "my-steamodded", "has_steamodded_here"),
        ] {
            let local = DetectedMod {
                name: name.into(),
                id: id.into(),
                author: vec![],
                description: String::new(),
                prefix: String::new(),
                version: None,
                path: format!("/mods/{dir}"),
                dependencies: vec![],
                conflicts: vec![],
                catalog_match: None,
                is_duplicate: false,
            };

            let index = super::CatalogIndex::new(&catalog);
            let m =
                super::find_catalog_match_indexed(&local, &index).expect("should match steamodded");
            assert_eq!(m.title, "Steamodded");
            assert_eq!(m.catalog_id, "Steamodded");
            assert_eq!(m.download_url, "https://example/steamodded.zip");
            assert_eq!(m.version.as_deref(), Some("1.0.0"));
        }
    }

    #[test]
    fn test_find_catalog_match_ignores_loose_similarity() {
        let catalog = vec![make_catalog_mod("Jambatro"), make_catalog_mod("Furlatro")];

        for (name, id, path) in [
            ("Batro", "Batro", "/mods/Batro"),
            ("Qualatro", "qualatro", "/mods/qualatro"),
        ] {
            let local = DetectedMod {
                name: name.into(),
                id: id.into(),
                author: vec![],
                description: String::new(),
                prefix: String::new(),
                version: None,
                path: path.into(),
                dependencies: vec![],
                conflicts: vec![],
                catalog_match: None,
                is_duplicate: false,
            };

            let index = super::CatalogIndex::new(&catalog);
            assert!(
                super::find_catalog_match_indexed(&local, &index).is_none(),
                "{name} should not match catalog entry"
            );
        }
    }

    #[test]
    fn test_detect_mod_in_directory_from_json_and_lua() {
        let td = tempdir().unwrap();
        let mod_dir = td.path().join("Test Mod");
        std::fs::create_dir_all(&mod_dir).unwrap();

        // JSON-based mod
        let json = r#"{
            "id": "TestMod",
            "name": "Test Mod",
            "author": ["Alice", "Bob"],
            "description": "Test description",
            "prefix": "test",
            "main_file": "Test Mod.lua",
            "version": "0.1.0",
            "dependencies": ["Steamodded"],
            "conflicts": []
        }"#;
        write_file(&mod_dir.join("mod.json"), json);

        let json_detected = super::detect_mod_in_directory(&mod_dir)
            .unwrap()
            .expect("JSON mod should be detected");
        assert_eq!(json_detected.name, "Test Mod");
        assert_eq!(json_detected.id, "TestMod");
        assert_eq!(json_detected.author, vec!["Alice", "Bob"]);
        assert_eq!(json_detected.prefix, "test");
        assert_eq!(json_detected.version.as_deref(), Some("0.1.0"));
        assert_eq!(json_detected.dependencies, vec!["Steamodded"]);

        // Lua-header-based mod (in a new dir)
        let lua_mod_dir = td.path().join("LuaBased");
        std::fs::create_dir_all(&lua_mod_dir).unwrap();
        let lua = "\
--- STEAMODDED HEADER\n\
--- MOD_NAME: LuaBased\n\
--- MOD_ID: LuaBased\n\
--- MOD_AUTHOR: [Charlie]\n\
--- MOD_DESCRIPTION: Simple\n\
--- PREFIX: lua\n\
--- VERSION: 1.2.3\n";
        write_file(&lua_mod_dir.join("LuaBased.lua"), lua);

        let lua_detected = super::detect_mod_in_directory(&lua_mod_dir)
            .unwrap()
            .expect("Lua header mod should be detected");
        assert_eq!(lua_detected.name, "LuaBased");
        assert_eq!(lua_detected.id, "LuaBased");
        assert_eq!(lua_detected.author, vec!["Charlie"]);
        assert_eq!(lua_detected.prefix, "lua");
        assert_eq!(lua_detected.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn test_detect_mod_in_zip_from_metadata_json() {
        use std::io::Write;

        let td = tempdir().unwrap();
        let zip_path = td.path().join("Joshi-sDecks-main.zip");
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();

        zw.add_directory("Joshi-sDecks-main/", opts).unwrap();
        zw.start_file("Joshi-sDecks-main/metadata.json", opts)
            .unwrap();
        zw.write_all(
            br#"{"id":"joshi's decks","name":"Joshi's Decks","author":["Joshi"],"description":"A collection of various decks.","prefix":"JoDe","main_file":"main.lua","version":"0.1.1"}"#,
        )
        .unwrap();
        zw.start_file("Joshi-sDecks-main/main.lua", opts).unwrap();
        zw.write_all(b"SMODS.Atlas{}\n").unwrap();
        zw.finish().unwrap();

        let detected = super::detect_mod_in_zip(&zip_path)
            .unwrap()
            .expect("zip mod should be detected");
        assert_eq!(detected.name, "Joshi's Decks");
        assert_eq!(detected.id, "joshi's decks");
        assert_eq!(detected.prefix, "JoDe");
        assert_eq!(detected.version.as_deref(), Some("0.1.1"));
        assert_eq!(detected.author, vec!["Joshi"]);
        assert_eq!(detected.path, zip_path.to_string_lossy());
    }

    #[test]
    fn test_detect_mod_in_zip_lua_header_and_non_mod() {
        use std::io::Write;

        let td = tempdir().unwrap();

        // Zip whose only metadata is a Steamodded Lua header.
        let lua_zip = td.path().join("HeaderMod.zip");
        let f = std::fs::File::create(&lua_zip).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("HeaderMod/HeaderMod.lua", opts).unwrap();
        zw.write_all(
            b"--- STEAMODDED HEADER\n--- MOD_NAME: Header Mod\n--- MOD_ID: HeaderMod\n--- PREFIX: hdr\n--- VERSION: 2.0.0\n",
        )
        .unwrap();
        zw.finish().unwrap();

        let detected = super::detect_mod_in_zip(&lua_zip)
            .unwrap()
            .expect("lua-header zip should be detected");
        assert_eq!(detected.name, "Header Mod");
        assert_eq!(detected.id, "HeaderMod");
        assert_eq!(detected.prefix, "hdr");
        assert_eq!(detected.version.as_deref(), Some("2.0.0"));

        // Zip with neither Lua nor JSON should not be treated as a mod.
        let junk_zip = td.path().join("backup.zip");
        let f = std::fs::File::create(&junk_zip).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file("notes.txt", opts).unwrap();
        zw.write_all(b"not a mod").unwrap();
        zw.finish().unwrap();

        assert!(
            super::detect_mod_in_zip(&junk_zip).unwrap().is_none(),
            "non-mod zip should be ignored"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_find_steamapps_root_valid_paths() {
        // Standard Steam library path
        let standard_path = Path::new("/home/user/.local/share/Steam/steamapps/common/Balatro");
        let result = super::find_steamapps_root(standard_path);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/home/user/.local/share/Steam/steamapps")
        );

        // External drive / SD card path
        let external_path =
            Path::new("/run/media/user/SD512/SteamLibrary/steamapps/common/Balatro");
        let result = super::find_steamapps_root(external_path);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/run/media/user/SD512/SteamLibrary/steamapps")
        );

        // Flatpak Steam path
        let flatpak_path = Path::new(
            "/home/user/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/Balatro",
        );
        let result = super::find_steamapps_root(flatpak_path);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            PathBuf::from(
                "/home/user/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps"
            )
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_find_steamapps_root_invalid_paths() {
        // Path not in common directory
        let invalid_path = Path::new("/home/user/Games/Balatro");
        assert!(super::find_steamapps_root(invalid_path).is_none());

        // Path with wrong parent directory name
        let wrong_parent = Path::new("/home/user/.local/share/Steam/steamapps/games/Balatro");
        assert!(super::find_steamapps_root(wrong_parent).is_none());

        // Path without steamapps
        let no_steamapps = Path::new("/home/user/.local/share/Steam/common/Balatro");
        assert!(super::find_steamapps_root(no_steamapps).is_none());
    }
}
