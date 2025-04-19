// TODO: Currently this only gives Profile.jkr's and settings.jkr and blueprint.jkr. Can you now make it give meta.jkr & save.jkr too from the folders?
// src-tauri/src/save_editor.rs
use bmm_lib::balamod; // Use the balamod module from the library crate
use bmm_lib::errors::AppError; // Use the AppError type from the library crate
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

// Keep lazy_static definitions
lazy_static! {
    static ref RETURN_PREFIX: Regex = Regex::new(r"^return ").unwrap();
    static ref STRING_KEYS: Regex = Regex::new(r#"\["(.*?)"\]="#).unwrap();
    static ref NUMBER_KEYS: Regex = Regex::new(r#"\[(\d+)\]="#).unwrap();
    static ref TRAILING_COMMAS: Regex = Regex::new(r#",}"#).unwrap();
    static ref NUMBER_KEY_JSON: Regex = Regex::new(r#""NOSTRING_(\d+)":"#).unwrap();
    static ref STRING_KEY_JSON: Regex = Regex::new(r#""([^"]*?)":"#).unwrap();
}

// --- Internal Helper Functions (no need for pub) ---

fn decompress(data: &[u8]) -> Result<String, AppError> {
    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = String::new();
    decoder
        .read_to_string(&mut decompressed)
        .map_err(|e| AppError::Unknown(format!("Decompression failed: {}", e)))?;
    Ok(decompressed)
}

fn compress(data: &str) -> Result<Vec<u8>, AppError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data.as_bytes())
        .map_err(|e| AppError::Unknown(format!("Compression failed: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| AppError::Unknown(format!("Compression finish failed: {}", e)))
}

fn raw_to_json_string(data: &str) -> String {
    let removed_return = RETURN_PREFIX.replace(data, "");
    let with_string_keys = STRING_KEYS.replace_all(&removed_return, "\"$1\":");
    let with_number_keys = NUMBER_KEYS.replace_all(&with_string_keys, "\"NOSTRING_$1\":");
    TRAILING_COMMAS
        .replace_all(&with_number_keys, "}")
        .to_string()
}

fn raw_to_json_value(data: &str) -> Result<Value, AppError> {
    let json_string = raw_to_json_string(data);
    serde_json::from_str(&json_string).map_err(|e| AppError::Serialization {
        format: "JSON".into(),
        source: format!("Failed to parse generated JSON: {}", e),
    })
}

fn fix_json_arrays_recursive(json: Value) -> Value {
    match json {
        Value::Object(map) => {
            let mut is_array_like = true;
            let mut max_index: i64 = -1;
            let mut keys_indices = Vec::new();

            for key in map.keys() {
                if let Some(index_str) = key.strip_prefix("NOSTRING_") {
                    if let Ok(index) = index_str.parse::<i64>() {
                        if index > 0 {
                            let zero_based_index = index - 1;
                            keys_indices.push((key.clone(), zero_based_index));
                            max_index = max_index.max(zero_based_index);
                            continue;
                        }
                    }
                }
                is_array_like = false;
                break;
            }

            if is_array_like && !keys_indices.is_empty() {
                keys_indices.sort_by_key(|&(_, index)| index);
                let is_sequential = keys_indices
                    .iter()
                    .enumerate()
                    .all(|(i, &(_, index))| i as i64 == index);

                if is_sequential && (keys_indices.len() as i64 == max_index + 1) {
                    let mut array = vec![Value::Null; keys_indices.len()];
                    for (key, index) in keys_indices {
                        if let Some(value) = map.get(&key) {
                            if let Some(elem) = array.get_mut(index as usize) {
                                *elem = fix_json_arrays_recursive(value.clone());
                            }
                        }
                    }
                    Value::Array(array)
                } else {
                    Value::Object(
                        map.into_iter()
                            .map(|(k, v)| (k, fix_json_arrays_recursive(v)))
                            .collect(),
                    )
                }
            } else {
                Value::Object(
                    map.into_iter()
                        .map(|(k, v)| (k, fix_json_arrays_recursive(v)))
                        .collect(),
                )
            }
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(fix_json_arrays_recursive).collect()),
        _ => json,
    }
}

fn process_file(buffer: &[u8]) -> Result<Value, AppError> {
    let decompressed_data = decompress(buffer)?;

    // Validate the file begins with "return "
    if !decompressed_data.starts_with("return ") {
        log::error!(
            "File doesn't start with 'return ' prefix: {:?}",
            decompressed_data.chars().take(20).collect::<String>()
        );
        return Err(AppError::Serialization {
            format: "Lua".into(),
            source: "File does not start with 'return ' prefix".into(),
        });
    }

    let initial_json = raw_to_json_value(&decompressed_data)?;
    Ok(fix_json_arrays_recursive(initial_json))
}

fn find_primary_jkr_file(dir_path: &Path) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir_path) {
        // Create prioritized lists for different file types
        let mut profile_jkr = None;
        let mut save_jkr = None;
        let mut meta_jkr = None;
        let mut other_jkr = None;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().to_lowercase() == "jkr" {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let name_lower = name.to_lowercase();
                            if name_lower == "profile.jkr" {
                                profile_jkr = Some(path.clone());
                            } else if name_lower == "save.jkr" {
                                save_jkr = Some(path.clone());
                            } else if name_lower == "meta.jkr" {
                                meta_jkr = Some(path.clone());
                            } else if other_jkr.is_none() {
                                other_jkr = Some(path.clone());
                            }
                        }
                    }
                }
            }
        }

        // Return files in order of priority
        profile_jkr.or(save_jkr).or(meta_jkr).or(other_jkr)
    } else {
        None
    }
}

// --- Public Struct for Frontend ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SaveDirectoryInfo {
    // <-- Make pub
    name: String,
    path: String,
    jkr_file_path: Option<String>,
    parsable: bool,
    error_message: Option<String>,
}

// --- Tauri Commands ---

#[tauri::command]
pub async fn list_save_directories() -> Result<Vec<SaveDirectoryInfo>, String> {
    // Get the base save directory
    let save_dir = balamod::get_balatro_save_directory().map_err(|e| e.to_string())?;
    log::info!("Scanning save directory: {}", save_dir.display());

    if !save_dir.exists() {
        log::warn!("Save directory does not exist: {}", save_dir.display());
        return Ok(vec![]);
    }

    let mut results = Vec::new();

    // DIRECT FILE CHECK: First, check if .jkr files are directly in the save directory
    let mut direct_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().to_lowercase() == "jkr" {
                        log::info!("Found direct .jkr file: {}", path.display());
                        direct_files.push(path);
                    }
                }
            }
        }
    }

    // If we found any direct .jkr files, add a virtual save directory for them
    if !direct_files.is_empty() {
        for jkr_path in direct_files {
            let file_name = jkr_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Check if parsable
            let mut parsable = false;
            let mut error_msg = None;

            log::info!("Checking direct file: {}", jkr_path.display());
            match fs::read(&jkr_path) {
                Ok(content) => match process_file(&content) {
                    Ok(_) => {
                        parsable = true;
                        log::info!("Successfully parsed {}", jkr_path.display());
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        log::warn!("Failed to parse {}: {}", jkr_path.display(), err_str);
                        error_msg = Some(format!("Unparsable: {}", err_str));
                    }
                },
                Err(e) => {
                    let err_str = format!("Read error: {}", e);
                    log::warn!("Failed to read {}: {}", jkr_path.display(), err_str);
                    error_msg = Some(err_str);
                }
            }

            results.push(SaveDirectoryInfo {
                name: format!("Main Save - {}", file_name), // Give it a descriptive name
                path: save_dir.to_string_lossy().to_string(),
                jkr_file_path: Some(jkr_path.to_string_lossy().to_string()),
                parsable,
                error_message: error_msg,
            });
        }
    }

    // Track numbered directories for sorting
    let mut numbered_dirs = Vec::new();
    let mut other_dirs = Vec::new();

    // SUBDIRECTORY CHECK: Then check all subdirectories
    if let Ok(entries) = fs::read_dir(&save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                log::info!("Checking directory: {}", dir_name);

                // First, explicitly look for profile.jkr and save.jkr
                let mut specific_files = Vec::new();
                let mut found_profile = false;
                let mut found_save = false;

                if let Ok(dir_entries) = fs::read_dir(&path) {
                    for file_entry in dir_entries.flatten() {
                        let file_path = file_entry.path();
                        if file_path.is_file() {
                            if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
                                let name_lower = name.to_lowercase();
                                if name_lower == "profile.jkr" {
                                    log::info!("Found profile.jkr in directory {}", dir_name);
                                    specific_files.push((file_path.clone(), true)); // prioritize profile.jkr
                                    found_profile = true;
                                } else if name_lower == "save.jkr" {
                                    log::info!("Found save.jkr in directory {}", dir_name);
                                    specific_files.push((file_path.clone(), false)); // save.jkr is secondary
                                    found_save = true;
                                }
                            }
                        }
                    }
                }

                // Choose the best file to display (profile.jkr preferred)
                let chosen_file = if !specific_files.is_empty() {
                    // Sort prioritizing profile.jkr
                    specific_files.sort_by(|a, b| b.1.cmp(&a.1));
                    Some(specific_files[0].0.clone())
                } else {
                    // Fall back to the first .jkr file if no specific ones found
                    find_primary_jkr_file(&path)
                };

                // Process the directory with the chosen file
                if let Some(jkr_path) = chosen_file {
                    let mut parsable = false;
                    let mut error_msg = None;

                    match fs::read(&jkr_path) {
                        Ok(content) => match process_file(&content) {
                            Ok(_) => {
                                parsable = true;
                                log::info!("Successfully parsed {}", jkr_path.display());
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                log::warn!("Failed to parse {}: {}", jkr_path.display(), err_str);
                                error_msg = Some(format!("Unparsable: {}", err_str));
                            }
                        },
                        Err(e) => {
                            let err_str = format!("Read error: {}", e);
                            log::warn!("Failed to read {}: {}", jkr_path.display(), err_str);
                            error_msg = Some(err_str);
                        }
                    }

                    let dir_info = SaveDirectoryInfo {
                        name: match dir_name.parse::<usize>() {
                            Ok(num) => format!("Profile {}", num),
                            Err(_) => dir_name.clone(),
                        },
                        path: path.to_string_lossy().to_string(),
                        jkr_file_path: Some(jkr_path.to_string_lossy().to_string()),
                        parsable,
                        error_message: error_msg,
                    };

                    // Add to appropriate list (numbered or other)
                    if dir_name.parse::<usize>().is_ok() {
                        let num = dir_name.parse::<usize>().unwrap_or(0);
                        log::info!("Found numbered profile directory: {} ({})", dir_name, num);

                        // Save profile type for better display
                        let profile_type = if found_profile {
                            " (profile.jkr)"
                        } else if found_save {
                            " (save.jkr)"
                        } else {
                            ""
                        };

                        numbered_dirs.push((num, dir_info, profile_type.to_string()));
                    } else {
                        log::info!("Found non-numbered directory: {}", dir_name);
                        other_dirs.push(dir_info);
                    }
                } else {
                    log::warn!("No .jkr file found in directory {}", dir_name);
                }
            }
        }
    }

    // Sort numbered directories by their number
    numbered_dirs.sort_by_key(|&(num, _, _)| num);

    // Add sorted numbered directories first
    for (_, mut dir_info, profile_type) in numbered_dirs {
        // Enhance the name with the profile type information
        dir_info.name = format!("{}{}", dir_info.name, profile_type);
        results.push(dir_info);
    }

    // Then add other directories
    results.extend(other_dirs);

    log::info!("Found {} save directories with .jkr files", results.len());
    Ok(results)
}

#[tauri::command]
pub async fn load_save_file(path: String) -> Result<Value, String> {
    log::info!("Loading save file: {}", path);

    // Try to read the file
    match std::fs::read(&path) {
        Ok(file_content) => {
            // Try to process the file
            match process_file(&file_content) {
                Ok(data) => Ok(data),
                Err(e) => Err(format!("Failed to process save file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

#[tauri::command]
pub async fn save_modified_file(path: String, data: Value) -> Result<(), String> {
    log::info!("Saving modified file: {}", path);

    // Generate Lua code from JSON data
    let lua_data = match generate_lua_save(&data) {
        Ok(lua) => lua,
        Err(e) => {
            log::error!("Failed to generate Lua save data: {}", e);
            return Err(format!("Failed to generate Lua save: {}", e));
        }
    };

    // Compress the data
    let compressed_data = match compress(&lua_data) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Failed to compress Lua data: {}", e);
            return Err(format!("Failed to compress save: {}", e));
        }
    };

    // Write directly to the file
    if let Err(e) = std::fs::write(&path, &compressed_data) {
        log::error!("Failed to write save file: {}", e);
        return Err(format!("Failed to write save file: {}", e));
    }

    log::info!("Successfully saved {}", path);
    Ok(())
}

// This is a completely new function for emergency saving
fn generate_lua_save(json: &Value) -> Result<String, String> {
    fn serialize_value(value: &Value, _indent: usize) -> String {
        match value {
            Value::Null => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => {
                // Properly escape the string
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                format!("\"{}\"", escaped)
            }
            Value::Array(arr) => {
                if arr.is_empty() {
                    return "{}".to_string();
                }

                let mut result = String::from("{");
                for (i, item) in arr.iter().enumerate() {
                    // In Lua, arrays are 1-indexed
                    result.push_str(&format!(
                        "[{}]={},",
                        i + 1,
                        serialize_value(item, _indent + 2)
                    ));
                }
                // Remove trailing comma
                if result.ends_with(',') {
                    result.pop();
                }
                result.push('}');
                result
            }
            Value::Object(map) => {
                if map.is_empty() {
                    return "{}".to_string();
                }

                let mut result = String::from("{");

                for (key, val) in map {
                    // Handle special numeric key format
                    if let Some(numeric_str) = key.strip_prefix("NOSTRING_") {
                        if let Ok(num) = numeric_str.parse::<u64>() {
                            result.push_str(&format!(
                                "[{}]={},",
                                num,
                                serialize_value(val, _indent + 2)
                            ));
                            continue;
                        }
                    }

                    // Check if the key is a valid Lua identifier
                    let is_valid_identifier = key.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && !key.is_empty()
                        && !key.chars().next().unwrap().is_ascii_digit();

                    if is_valid_identifier {
                        result.push_str(&format!("{}={},", key, serialize_value(val, _indent + 2)));
                    } else {
                        // Escape the key string
                        let escaped_key = key.replace('\\', "\\\\").replace('"', "\\\"");
                        result.push_str(&format!(
                            "[\"{}\"]={},",
                            escaped_key,
                            serialize_value(val, _indent + 2)
                        ));
                    }
                }

                // Remove trailing comma
                if result.ends_with(',') {
                    result.pop();
                }
                result.push('}');
                result
            }
        }
    }

    Ok(format!("return {}", serialize_value(json, 0)))
}

#[tauri::command]
pub async fn get_balatro_save_path() -> Result<String, String> {
    // Use balamod function from bmm_lib
    balamod::get_balatro_save_directory()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_file_last_modified(path: String) -> Result<u64, String> {
    let metadata =
        std::fs::metadata(&path).map_err(|e| format!("Failed to get metadata: {}", e))?;

    let modified = metadata
        .modified()
        .map_err(|e| format!("Failed to get modification time: {}", e))?;

    let timestamp = modified
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to convert to timestamp: {}", e))?
        .as_millis() as u64;

    Ok(timestamp)
}
