// src-tauri/src/save_editor.rs
use bmm_lib::balamod; // Use the balamod module from the library crate
use bmm_lib::errors::AppError; // Use the AppError type from the library crate
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
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

fn fix_lua_arrays_recursive(json: Value) -> Value {
    match json {
        Value::Array(arr) => {
            let mut map = serde_json::Map::new();
            for (i, v) in arr.into_iter().enumerate() {
                let key = format!("NOSTRING_{}", i + 1);
                map.insert(key, fix_lua_arrays_recursive(v));
            }
            Value::Object(map)
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, fix_lua_arrays_recursive(v)))
                .collect(),
        ),
        _ => json,
    }
}

fn json_value_to_raw(data: &Value) -> Result<String, AppError> {
    let json_string = serde_json::to_string(data).map_err(|e| AppError::Serialization {
        format: "JSON".into(),
        source: format!("Failed to stringify JSON: {}", e),
    })?;

    let with_number_keys =
        NUMBER_KEY_JSON.replace_all(&json_string, |caps: &Captures| format!("[{}]=", &caps[1]));

    let with_string_keys = STRING_KEY_JSON.replace_all(&with_number_keys, |caps: &Captures| {
        let key = &caps[1];
        // Use is_some_and here
        let is_simple_identifier = key.chars().all(|c| c.is_alphanumeric() || c == '_')
            && key
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_');

        if is_simple_identifier {
            format!("{}=", key)
        } else {
            let escaped_key = key.replace('\"', "\\\"");
            format!("[\"{}\"]=", escaped_key)
        }
    });

    Ok(format!("return {}", with_string_keys))
}

fn process_file(buffer: &[u8]) -> Result<Value, AppError> {
    let decompressed_data = decompress(buffer)?;
    let initial_json = raw_to_json_value(&decompressed_data)?;
    Ok(fix_json_arrays_recursive(initial_json))
}

fn process_json(json: Value) -> Result<Vec<u8>, AppError> {
    let lua_like_json = fix_lua_arrays_recursive(json);
    let raw_data = json_value_to_raw(&lua_like_json)?;
    compress(&raw_data)
}

// Helper function for list_save_directories (defined before use)
fn find_primary_jkr_file(dir_path: &Path) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir_path) {
        let mut first_jkr: Option<PathBuf> = None;
        // Apply flatten
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "jkr" {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if name == "profile.jkr" || name == "save.jkr" || name == "meta.jkr" {
                                return Some(path);
                            }
                        }
                        if first_jkr.is_none() {
                            first_jkr = Some(path);
                        }
                    }
                }
            }
        }
        first_jkr
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
    // Use balamod function from bmm_lib
    let save_dir = balamod::get_balatro_save_directory().map_err(|e| e.to_string())?;
    log::info!("Scanning save directory: {}", save_dir.display());

    if !save_dir.exists() {
        log::warn!("Save directory does not exist.");
        return Ok(vec![]);
    }

    let mut results = Vec::new();

    // Apply flatten
    for entry in fs::read_dir(&save_dir)
        .map_err(|e| format!("Failed to read save directory: {}", e))?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            log::debug!("Checking directory: {}", dir_name);

            let primary_jkr_path = find_primary_jkr_file(&path); // Use the helper

            let mut parsable = false;
            let mut error_msg = None;
            let jkr_file_display_path = primary_jkr_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string());

            if let Some(ref jkr_path) = primary_jkr_path {
                // Fix E0282: Use display() on the PathBuf inside the Option
                log::debug!("Found JKR file: {}", jkr_path.display());
                match fs::read(jkr_path) {
                    Ok(content) => match process_file(&content) {
                        // Use internal process_file
                        Ok(_) => {
                            parsable = true;
                            log::debug!("Successfully parsed {}", jkr_path.display());
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            log::warn!("Failed to parse {}: {}", jkr_path.display(), err_str);
                            error_msg = Some(format!("Unparsable: {}", err_str));
                        }
                    },
                    Err(e) => {
                        let err_str = format!("Read error: {}", e.to_string());
                        log::warn!("Failed to read {}: {}", jkr_path.display(), err_str);
                        error_msg = Some(err_str);
                    }
                }
            } else {
                log::debug!("No .jkr file found in directory {}", dir_name);
                parsable = false;
            }

            if primary_jkr_path.is_some() {
                results.push(SaveDirectoryInfo {
                    name: dir_name,
                    path: path.to_string_lossy().to_string(),
                    jkr_file_path: jkr_file_display_path,
                    parsable,
                    error_message: error_msg,
                });
            }
        }
    }
    log::info!("Found {} potential save directories.", results.len());
    Ok(results)
}

#[tauri::command]
pub async fn load_save_file(path: String) -> Result<Value, String> {
    log::info!("Loading save file: {}", path);
    let file_content = std::fs::read(&path).map_err(|e| {
        AppError::FileRead {
            path: PathBuf::from(path.clone()),
            source: e.to_string(),
        }
        .to_string()
    })?;
    // Use internal process_file
    process_file(&file_content).map_err(|e| {
        log::error!("Failed to process save file {}: {}", path, e);
        e.to_string()
    })
}

#[tauri::command]
pub async fn save_modified_file(path: String, data: Value) -> Result<(), String> {
    log::info!("Saving modified file: {}", path);
    // Use internal process_json
    let processed_data = process_json(data).map_err(|e| e.to_string())?;
    std::fs::write(&path, processed_data).map_err(|e| {
        let err = AppError::FileWrite {
            path: PathBuf::from(path.clone()),
            source: e.to_string(),
        };
        log::error!("Failed to write save file {}: {}", path, err);
        err.to_string()
    })?;
    log::info!("Successfully saved {}", path);
    Ok(())
}

#[tauri::command]
pub async fn get_balatro_save_path() -> Result<String, String> {
    // Use balamod function from bmm_lib
    balamod::get_balatro_save_directory()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}
