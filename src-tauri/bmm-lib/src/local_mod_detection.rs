use crate::cache;
use crate::database::Database;
use crate::finder::get_lovely_mods_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::path::PathBuf;

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
            log::error!(
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
            log::error!(
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
    let mod_dir = get_lovely_mods_dir();

    if !mod_dir.exists() {
        return Ok(Vec::new());
    }

    // Get tracked mods from the database for duplicate detection
    let managed_mods = db
        .get_installed_mods()
        .map_err(|e| format!("Failed to get installed mods: {}", e))?;

    // Create a set of normalized managed mod paths for quick lookup
    let managed_paths: HashSet<String> = managed_mods
        .iter()
        .map(|m| normalize_path(&PathBuf::from(&m.path)))
        .collect();

    // Create a set of managed mod names (lowercase) for duplicate detection
    let managed_names: HashSet<String> =
        managed_mods.iter().map(|m| m.name.to_lowercase()).collect();

    let mut manual_mods = Vec::new();
    let mut bundled_dependencies = HashSet::new();

    // Find bundled dependencies in mod packages
    find_bundled_dependencies(&mod_dir, &mut bundled_dependencies)?;

    // Detect mods from filesystem
    let mut all_detected_mods = Vec::new();
    detect_mods_recursive(&mod_dir, &mut all_detected_mods, &bundled_dependencies)?;

    // Process detected mods to find catalog matches and handle duplicates
    for mut mod_info in all_detected_mods {
        let mod_path = normalize_path(&PathBuf::from(&mod_info.path));

        // If this mod is not managed by path, consider it a manual mod
        if !is_path_managed(&mod_path, &managed_paths) {
            // Check for name duplication with managed mods
            let mod_name_lower = mod_info.name.to_lowercase();
            if managed_names.contains(&mod_name_lower) {
                mod_info.is_duplicate = true;
                // Append a suffix to the name
                mod_info.name = format!("{} (Manual)", mod_info.name);
            }

            // Try to find a match in the catalog
            mod_info.catalog_match = find_catalog_match(&mod_info, cached_catalog_mods);

            manual_mods.push(mod_info);
        }
    }

    Ok(manual_mods)
}

fn scan_for_json_files(dir_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut json_files = Vec::new();

    // Read directory entries
    let entries = fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory {}: {}", dir_path.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
            json_files.push(path);
        }
    }

    Ok(json_files)
}

fn find_catalog_match(
    local_mod: &DetectedMod,
    catalog_mods: &[cache::Mod],
) -> Option<CatalogMatch> {
    // Special case for Steamodded
    let local_id_lower = local_mod.id.to_lowercase();
    let local_name_lower = local_mod.name.to_lowercase();

    // Get directory name for additional checking
    let dir_name_lower = Path::new(&local_mod.path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Enhanced Steamodded detection
    if local_id_lower == "steamodded" || 
       local_name_lower == "steamodded" ||
       local_id_lower.contains("steamodded") || 
       local_name_lower.contains("steamodded") ||
       local_id_lower == "smods" || 
       local_name_lower == "smods" ||
       dir_name_lower.starts_with("smods") ||  // Match anything starting with "smods"
       dir_name_lower.contains("steamodded")
    {
        // Find Steamodded in the catalog
        for catalog_mod in catalog_mods {
            if catalog_mod.title.to_lowercase() == "steamodded" {
                return Some(CatalogMatch {
                    title: catalog_mod.title.clone(),
                    catalog_id: catalog_mod.title.clone(),
                    download_url: catalog_mod.download_url.clone(),
                    version: catalog_mod.version.clone(),
                });
            }
        }
    }

    // Rest of the function remains the same...
    // 1. Try exact ID match (most specific)
    for catalog_mod in catalog_mods {
        let catalog_id_lower = catalog_mod.title.replace(" ", "").to_lowercase();
        if catalog_id_lower == local_id_lower {
            return Some(create_match(catalog_mod));
        }
    }

    // 2. Try exact name match
    for catalog_mod in catalog_mods {
        let catalog_name_lower = catalog_mod.title.to_lowercase();
        if catalog_name_lower == local_name_lower {
            return Some(create_match(catalog_mod));
        }
    }

    // 3. Try directory name match (already handled above for Steamodded)
    if let Some(dir_name) = Path::new(&local_mod.path)
        .file_name()
        .and_then(|n| n.to_str())
    {
        let dir_name_lower = dir_name.to_lowercase();
        for catalog_mod in catalog_mods {
            let catalog_name_lower = catalog_mod.title.to_lowercase();
            if catalog_name_lower == dir_name_lower {
                return Some(create_match(catalog_mod));
            }
        }
    }

    // 4. Try substring matching in either direction
    for catalog_mod in catalog_mods {
        let catalog_name_lower = catalog_mod.title.to_lowercase();

        if local_name_lower.contains(&catalog_name_lower)
            || catalog_name_lower.contains(&local_name_lower)
        {
            return Some(create_match(catalog_mod));
        }
    }

    // 5. Try similarity matching (edit distance)
    for catalog_mod in catalog_mods {
        let catalog_name_lower = catalog_mod.title.to_lowercase();

        // Calculate similarity ratio
        if is_similar(&local_name_lower, &catalog_name_lower)
            || is_similar(&local_id_lower, &catalog_name_lower.replace(" ", ""))
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

// Helper function to determine if two strings are similar enough
fn is_similar(a: &str, b: &str) -> bool {
    // If strings are very different in length, they're probably not similar
    let len_diff = (a.len() as isize - b.len() as isize).abs();
    if len_diff > 3 {
        return false;
    }

    // For short strings, allow fewer differences
    let max_distance = if a.len() < 5 || b.len() < 5 { 1 } else { 2 };

    // Simple implementation of edit distance calculation
    calculate_edit_distance(a, b) <= max_distance
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
    for j in 0..=n {
        matrix[0][j] = j;
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
fn find_bundled_dependencies(dir: &Path, bundled_deps: &mut HashSet<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Skip lovely-related directories
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let lower_name = file_name.to_lowercase();
            if lower_name.contains("lovely") {
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

        // Recursively check subdirectories (limited depth)
        let depth = count_path_depth(&path, dir);
        if depth <= 3 {
            // Increase depth to make sure we find nested packages
            find_bundled_dependencies(&path, bundled_deps)?;
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
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            // Add this dependency's normalized path to our set
            let normalized_path = normalize_path(&path);
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
    detected_mods: &mut Vec<DetectedMod>,
    bundled_deps: &HashSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Skip lovely-related directories
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            let lower_name = file_name.to_lowercase();
            if lower_name.contains("lovely") {
                continue;
            }
        }

        // Skip bundled dependencies
        let normalized_path = normalize_path(&path);
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
            detect_mods_recursive(&path, detected_mods, bundled_deps)?;
            continue;
        }

        // Regular directory, recursively scan up to 2 levels deep
        let depth = count_path_depth(&path, dir);
        if depth <= 2 {
            detect_mods_recursive(&path, detected_mods, bundled_deps)?;
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

fn count_path_depth(path: &Path, base_path: &Path) -> usize {
    let path_str = path.to_string_lossy();
    let base_str = base_path.to_string_lossy();

    if !path_str.starts_with(&*base_str) {
        return 0;
    }

    let relative = &path_str[base_str.len()..];
    relative
        .chars()
        .filter(|&c| c == std::path::MAIN_SEPARATOR)
        .count()
}

fn detect_mod_in_directory(mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    // Get directory name
    let dir_name = mod_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Invalid directory name: {}", mod_path.display()))?;

    // Check for Thunderstore manifest.json first
    let manifest_path = mod_path.join("manifest.json");
    if manifest_path.exists() {
        if let Some(detected_mod) = parse_thunderstore_manifest(&manifest_path, mod_path)? {
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
    // Scan for all JSON files and check if any of them are valid mod configs
    let json_files = scan_for_json_files(mod_path)?;
    for json_path in json_files {
        if let Some(detected_mod) = parse_mod_json(&json_path, mod_path)? {
            return Ok(Some(detected_mod));
        }
    }

    // Look for any Lua file with the same name as the directory
    let lua_path = mod_path.join(format!("{}.lua", dir_name));
    if lua_path.exists() {
        if let Some(detected_mod) = parse_mod_lua_header(&lua_path, mod_path)? {
            return Ok(Some(detected_mod));
        }
    }

    // Special handling for mod packages that have a structure like:
    // ModName/Mods/ModName/ModName.lua
    let potential_mod_dir = mod_path.join("Mods").join(dir_name);
    let potential_lua_path = potential_mod_dir.join(format!("{}.lua", dir_name));

    if potential_lua_path.exists() {
        if let Some(detected_mod) = parse_mod_lua_header(&potential_lua_path, mod_path)? {
            return Ok(Some(detected_mod));
        }
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
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("lua") {
            if let Some(detected_mod) = parse_mod_lua_header(&path, mod_path)? {
                return Ok(Some(detected_mod));
            }
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

/// JSON schema for mod configuration
#[derive(Debug, Serialize, Deserialize)]
struct ModJson {
    id: String,
    name: String,
    author: Vec<String>,
    description: String,
    prefix: String,
    main_file: String,
    #[serde(default)]
    priority: i32,
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

fn default_badge_color() -> String {
    "666665".to_string()
}

fn default_text_color() -> String {
    "FFFFFF".to_string()
}

/// Parse mod info from JSON file
fn parse_mod_json(json_path: &Path, mod_path: &Path) -> Result<Option<DetectedMod>, String> {
    let file = match File::open(json_path) {
        Ok(file) => file,
        Err(e) => {
            log::error!("Failed to open JSON file {}: {}", json_path.display(), e);
            return Ok(None);
        }
    };

    let mod_json: ModJson = match serde_json::from_reader(file) {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to parse JSON file {}: {}", json_path.display(), e);
            return Ok(None);
        }
    };

    // Check if ID is valid (not one of the disallowed values)
    let disallowed_ids = ["Steamodded", "Lovely", "Balatro"];
    if disallowed_ids.contains(&mod_json.id.as_str()) {
        log::info!("Mod {} has a disallowed ID: {}", mod_json.name, mod_json.id);
        return Ok(None);
    }

    Ok(Some(DetectedMod {
        name: mod_json.name,
        id: mod_json.id,
        author: mod_json.author,
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

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .take(20) // Only check first 20 lines for efficiency
        .map(|line| line.map_err(|e| format!("Failed to read line: {}", e)))
        .collect::<Result<Vec<String>, String>>()?;

    if lines.is_empty() {
        return Ok(None);
    }

    // Check if any line has the header marker
    let has_header = lines
        .iter()
        .any(|line| line.trim() == "--- STEAMODDED HEADER");
    if !has_header {
        // Try to infer mod info from filename if no header
        if let Some(mod_name) = mod_path.file_name().and_then(|n| n.to_str()) {
            // Simple inference based on directory name
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
        return Ok(None);
    }

    // Parse the rest as before...
    let mut name = String::new();
    let mut id = String::new();
    let mut author = Vec::new();
    let mut description = String::new();
    let mut prefix = String::new();
    let mut version = None;
    let mut dependencies = Vec::new();
    let mut conflicts = Vec::new();

    // Parse the header lines
    for line in &lines {
        let line = line.trim();
        if !line.starts_with("---") {
            continue;
        }

        let line = &line[3..].trim();

        if let Some(value) = line.strip_prefix("MOD_NAME:") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("MOD_ID:") {
            id = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("MOD_AUTHOR:") {
            // Parse author list [Author1, Author2, ...]
            if let Some(author_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                author = author_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
            }
        } else if let Some(value) = line.strip_prefix("MOD_DESCRIPTION:") {
            description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("PREFIX:") {
            prefix = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("VERSION:") {
            version = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("DEPENDENCIES:") {
            // Parse dependencies list
            if let Some(deps_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                dependencies = deps_str.split(',').map(|s| s.trim().to_string()).collect();
            }
        } else if let Some(value) = line.strip_prefix("CONFLICTS:") {
            // Parse conflicts list
            if let Some(conf_str) = value
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                conflicts = conf_str.split(',').map(|s| s.trim().to_string()).collect();
            }
        }
    }

    // If we couldn't find required fields, try to infer from the directory/file name
    if name.is_empty() {
        if let Some(file_name) = lua_path.file_stem().and_then(|s| s.to_str()) {
            name = file_name.to_string();
        }
    }

    if id.is_empty() {
        if let Some(file_name) = lua_path.file_stem().and_then(|s| s.to_str()) {
            id = file_name.replace(" ", "");
        }
    }

    if author.is_empty() {
        author.push("Unknown".to_string());
    }

    if description.is_empty() {
        description = format!("Local mod found in {}", mod_path.display());
    }

    // If prefix is empty, use first 4 letters of ID
    if prefix.is_empty() && !id.is_empty() {
        if id.len() >= 4 {
            prefix = id[0..4].to_lowercase();
        } else {
            prefix = id.to_lowercase();
        }
    }

    Ok(Some(DetectedMod {
        name,
        id,
        author,
        description,
        prefix,
        version,
        path: mod_path.to_string_lossy().to_string(),
        dependencies,
        conflicts,
        catalog_match: None,
        is_duplicate: false,
    }))
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
