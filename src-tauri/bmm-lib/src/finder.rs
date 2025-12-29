use crate::database::Database;
use log::error;
use log::info;
#[cfg(target_os = "linux")]
use regex::Regex;
use std::collections::HashSet;
#[cfg(target_os = "windows")]
use std::fs::File;
#[cfg(target_os = "windows")]
use std::io::{BufReader, Read};
#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use sysinfo::System;
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::*;

#[cfg(target_os = "windows")]
fn read_path_from_registry() -> Result<String, std::io::Error> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let steam_path = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")?;

    Ok(steam_path.get_value("InstallPath")?)
}

fn remove_unexisting_paths(paths: &mut Vec<PathBuf>) {
    let mut i = 0;
    while i < paths.len() {
        if !paths[i].exists() {
            paths.remove(i);
        } else {
            i += 1;
        }
    }
    info!("Found {} Balatro installations.", paths.len());
}

#[cfg(target_os = "windows")]
pub fn get_balatro_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = vec![];

    // 1) Respect custom Balatro path from our database first
    if let Ok(db) = Database::new()
        && let Ok(Some(custom_path)) = db.get_installation_path()
    {
        let p = PathBuf::from(&custom_path);
        if p.exists() {
            paths.push(p);
        }
    }

    // 2) Discover Steam libraries (registry + libraryfolders.vdf)
    let steam_path = read_path_from_registry();
    let mut steam_path = steam_path.unwrap_or_else(|_| {
        error!(
            "Could not read steam install path from Registry! Trying standard installation path in C:\\"
        );
        String::from("C:\\Program Files (x86)\\Steam")
    });

    steam_path.push_str("\\steamapps\\libraryfolders.vdf");
    let libraryfolders_path = Path::new(&steam_path);
    if !libraryfolders_path.exists() {
        error!(
            "'{}' not found.",
            libraryfolders_path.to_str().unwrap_or("<invalid path>")
        );
        // Return whatever we have (e.g., custom path), after cleaning
        remove_unexisting_paths(&mut paths);
        dedup_paths_case_insensitive(&mut paths);
        return paths;
    }

    let libraryfolders_file = match File::open(libraryfolders_path) {
        Ok(f) => f,
        Err(e) => {
            error!(
                "Failed to open libraryfolders.vdf at {}: {}",
                libraryfolders_path.to_string_lossy(),
                e
            );
            remove_unexisting_paths(&mut paths);
            dedup_paths_case_insensitive(&mut paths);
            return paths;
        }
    };

    let mut libraryfolders_contents = String::new();
    let mut libraryfolders_reader = BufReader::new(libraryfolders_file);
    if let Err(e) = libraryfolders_reader.read_to_string(&mut libraryfolders_contents) {
        error!("Failed to read libraryfolders.vdf: {}", e);
        remove_unexisting_paths(&mut paths);
        dedup_paths_case_insensitive(&mut paths);
        return paths;
    }

    let lines = libraryfolders_contents.split('\n').collect::<Vec<&str>>();
    for line in lines {
        if line.contains("\t\t\"path\"\t\t") {
            let parts = line.split('\"').collect::<Vec<&str>>();
            if parts.len() > 3 {
                let path = parts[3];
                paths.push(PathBuf::from(path).join("steamapps\\common\\Balatro"));
            }
        }
    }

    remove_unexisting_paths(&mut paths);
    dedup_paths_case_insensitive(&mut paths);
    paths
}

fn dedup_paths_case_insensitive(paths: &mut Vec<PathBuf>) {
    let mut seen: HashSet<String> = HashSet::new();
    paths.retain(|p| {
        let key = p.to_string_lossy().to_string().to_lowercase();
        seen.insert(key)
    });
}

#[cfg(target_os = "macos")]
pub fn get_balatro_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = vec![];

    // Prefer custom DB path first
    if let Ok(db) = Database::new()
        && let Ok(Some(custom_path)) = db.get_installation_path()
    {
        let p = PathBuf::from(&custom_path);
        if p.exists() {
            paths.push(p);
        }
    }
    match home::home_dir() {
        Some(path) => {
            let mut path = path;
            path.push("Library/Application Support/Steam/steamapps/common/Balatro");
            paths.push(path);
        }
        None => error!("Impossible to get your home dir!"),
    }
    remove_unexisting_paths(&mut paths);
    dedup_paths_case_insensitive(&mut paths);
    paths
}

#[cfg(target_os = "linux")]
fn parse_libraryfolders(library_path: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let contents = match std::fs::read_to_string(library_path) {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Failed to read libraryfolders.vdf at {}: {}",
                library_path.to_string_lossy(),
                e
            );
            return results;
        }
    };

    // Matches lines like: "path"    "/mnt/games/SteamLibrary"
    let re = Regex::new(r#""path"\s*"([^"]+)""#).unwrap();
    for caps in re.captures_iter(&contents) {
        if let Some(path_match) = caps.get(1) {
            let path_str = path_match.as_str();
            // Normalize escaped backslashes in case they appear
            let cleaned = path_str.replace("\\\\", "\\");
            results.push(PathBuf::from(cleaned).join("steamapps/common/Balatro"));
        }
    }

    results
}

#[cfg(target_os = "linux")]
pub fn get_balatro_paths() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = vec![];

    // Prefer custom DB path first
    if let Ok(db) = Database::new()
        && let Ok(Some(custom_path)) = db.get_installation_path()
    {
        let p = PathBuf::from(&custom_path);
        if p.exists() {
            paths.push(p);
        }
    }

    let mut steam_roots: Vec<PathBuf> = Vec::new();
    let mut seen_roots: HashSet<String> = HashSet::new();
    let mut add_root = |root: PathBuf| {
        let key = root.to_string_lossy().to_string().to_lowercase();
        if seen_roots.insert(key) {
            steam_roots.push(root);
        }
    };

    match home::home_dir() {
        Some(home) => {
            add_root(home.join(".local/share/Steam"));
            add_root(home.join(".steam/steam"));
            add_root(home.join(".var/app/com.valvesoftware.Steam/data/Steam"));
            // Snap installs keep the real Steam data under this prefix.
            add_root(home.join("snap/steam/common/.local/share/Steam"));
        }
        None => error!("Impossible to get your home dir!"),
    }

    for root in steam_roots {
        let steamapps = root.join("steamapps");
        paths.push(steamapps.join("common/Balatro"));

        let libraryfolders = steamapps.join("libraryfolders.vdf");
        if libraryfolders.exists() {
            paths.extend(parse_libraryfolders(&libraryfolders));
        }
    }

    remove_unexisting_paths(&mut paths);
    dedup_paths_case_insensitive(&mut paths);
    paths
}

pub fn is_steam_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let system = System::new_all();
        let x = system
            .processes_by_exact_name(std::ffi::OsStr::new("steam.exe"))
            .next()
            .is_some();
        x
    }

    #[cfg(target_family = "unix")]
    {
        use libproc::proc_pid::name;
        use libproc::processes;

        if let Ok(pids) = processes::pids_by_type(processes::ProcFilter::All) {
            for pid in pids {
                if let Ok(name) = name(pid as i32)
                    && name.to_lowercase().contains("steam")
                {
                    return true;
                }
            }
        }
        false
    }
}

pub fn get_installed_mods() -> Vec<String> {
    let mut installed_mods_paths: Vec<PathBuf> = vec![];

    let mod_dir = crate::mods_dir();

    if !mod_dir.exists() {
        return vec![];
    }

    match mod_dir.read_dir() {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    installed_mods_paths.push(entry.path());
                }
            }
        }
        Err(e) => {
            error!("Failed to read mods directory {}: {}", mod_dir.display(), e);
            return vec![];
        }
    }

    let res: Vec<String> = installed_mods_paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();

    res.iter()
        .filter(|p| !p.contains(".lovely") && !p.contains("lovely"))
        .cloned()
        .collect()
}

pub fn is_balatro_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let system = System::new_all();
        let x = system
            .processes_by_exact_name(std::ffi::OsStr::new("Balatro.exe"))
            .next()
            .is_some();
        x
    }

    #[cfg(target_family = "unix")]
    {
        use libproc::proc_pid::name;
        use libproc::processes;

        let balatro_roots: Vec<_> = crate::finder::get_balatro_paths()
            .into_iter()
            .map(|p| p.canonicalize().unwrap_or(p))
            .collect();

        let current_pid = std::process::id() as i32;
        let self_exe_path = std::env::current_exe().ok();
        let self_exe_name = self_exe_path
            .as_ref()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));

        if let Ok(pids) = processes::pids_by_type(processes::ProcFilter::All) {
            for pid in pids {
                let pid = pid as i32;
                if pid == current_pid {
                    continue;
                }

                if let Ok(name) = name(pid) {
                    let name_lower = name.to_lowercase();

                    #[cfg(target_os = "linux")]
                    {
                        // Skip processes whose executable path matches our own (covers AppImage/wrapped launches).
                        if let (Some(self_path), Ok(proc_path)) = (
                            &self_exe_path,
                            std::fs::read_link(format!("/proc/{pid}/exe")),
                        ) && proc_path == *self_path
                        {
                            continue;
                        }
                    }

                    // Linux can truncate binary names; skip anything that matches our own executable name or obvious variants.
                    if let Some(self_name) = self_exe_name.as_deref()
                        && (name_lower == self_name
                            || self_name.starts_with(&name_lower)
                            || name_lower.starts_with(self_name))
                    {
                        continue;
                    }
                    if name_lower.contains("balatro-mod")
                        || name_lower.contains("balatro mod")
                        || name_lower.contains("balatro_mod")
                        || name_lower == "bmm"
                    {
                        continue;
                    }

                    if name_lower.contains("balatro") {
                        return true;
                    }
                    if name_lower == "love"
                        && let Ok(cwd) = std::fs::read_link(format!("/proc/{pid}/cwd"))
                    {
                        let cwd = cwd.canonicalize().unwrap_or(cwd);
                        if balatro_roots.contains(&cwd) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use tempfile::tempdir;

    fn set_var(key: &str, val: impl AsRef<OsStr>) {
        unsafe { std::env::set_var(key, val) };
    }

    fn remove_var(key: &str) {
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn test_get_installed_mods_filters_lovely_dirs() {
        let tmp = tempdir().unwrap();
        // Redirect config dir to temp using XDG_CONFIG_HOME and HOME (for macOS)
        let original_cfg = std::env::var_os("XDG_CONFIG_HOME");
        let original_home = std::env::var_os("HOME");
        set_var("XDG_CONFIG_HOME", tmp.path());
        if cfg!(target_os = "macos") {
            set_var("HOME", tmp.path());
        }

        let mods_root = dirs::config_dir().unwrap().join("Balatro").join("Mods");
        std::fs::create_dir_all(&mods_root).unwrap();

        // Create sample mod directories
        let keep_a = mods_root.join("CoolMod");
        let keep_b = mods_root.join("Another");
        let ignore_a = mods_root.join(".lovely");
        let ignore_b = mods_root.join("lovely");

        std::fs::create_dir_all(&keep_a).unwrap();
        std::fs::create_dir_all(&keep_b).unwrap();
        std::fs::create_dir_all(&ignore_a).unwrap();
        std::fs::create_dir_all(&ignore_b).unwrap();

        let mut mods = super::get_installed_mods();
        mods.sort();

        // Should only include the two non-lovely directories
        assert_eq!(mods.len(), 2);
        assert!(mods.iter().any(|p| p.ends_with("CoolMod")));
        assert!(mods.iter().any(|p| p.ends_with("Another")));

        // restore environment
        match original_cfg {
            Some(val) => set_var("XDG_CONFIG_HOME", val),
            None => remove_var("XDG_CONFIG_HOME"),
        }
        match original_home {
            Some(val) => set_var("HOME", val),
            None => remove_var("HOME"),
        }
    }
}
