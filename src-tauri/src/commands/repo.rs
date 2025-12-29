use std::path::PathBuf;

use crate::bmi::{self, BmiClient, SortMode, SyncCache};
use crate::lfs::{LfsError, resolve_lfs_pointer_bytes};
use crate::models::ModMeta;
use bmm_lib::errors::AppError;
use serde::{Deserialize, Serialize};

const BMI_CACHE_FILE: &str = "bmi_mods_cache";

#[tauri::command]
pub async fn list_repo_mods() -> Result<Vec<String>, String> {
    let items = fetch_repo_mods(None).await?;
    let mut out: Vec<String> = items.into_iter().map(|item| item.dir_name).collect();
    out.sort();
    Ok(out)
}

#[tauri::command]
pub async fn get_repo_file(path: &str) -> Result<String, String> {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 3 || parts[0] != "mods" {
        return Err("BMI repo file path unsupported".to_string());
    }
    let id = parts[1];
    let file = parts[2];
    let client = BmiClient::new()?;
    let detail = client.fetch_mod(id).await?;

    match file {
        "meta.json" => {
            let meta = detail
                .meta
                .ok_or_else(|| "BMI mod missing meta".to_string())?;
            serde_json::to_string(&meta).map_err(|e| e.to_string())
        }
        "description.md" => Ok(detail.description.unwrap_or_default()),
        _ => Err(format!("Unsupported BMI repo file: {file}")),
    }
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn get_repo_thumbnail_url(dirName: String) -> Result<Option<String>, String> {
    let client = BmiClient::new()?;
    let url = client.thumbnail_url(&dirName)?;
    Ok(Some(url))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveModItem {
    pub dir_name: String,
    pub meta: ModMeta,
    pub description: String,
    pub image_url: String,
}

// Fetch mod metadata and descriptions via the BMI HTTP service.
#[tauri::command]
pub async fn fetch_repo_mods(sort: Option<String>) -> Result<Vec<ArchiveModItem>, String> {
    let client = BmiClient::new()?;
    client.check_health().await?;
    let sort_mode = SortMode::from_optional(sort.clone());
    let (cache_dir, cache_file) = bmi_cache_paths(sort.as_deref())?;
    let cache = load_bmi_cache(&cache_file);

    let (mut items, mut latest_updated) = if let Some(existing) = cache.as_ref() {
        (existing.items.clone(), existing.last_updated_at.clone())
    } else {
        (Vec::new(), None)
    };
    if items.iter().any(|item| {
        let url = item.image_url.trim();
        url.is_empty() || !(url.starts_with("http://") || url.starts_with("https://"))
    }) {
        items.clear();
        latest_updated = None;
    }

    if items.is_empty() || latest_updated.is_none() {
        let (fresh, updated_at) = client.fetch_all_mods(sort_mode).await?;
        items = fresh;
        latest_updated = updated_at;
    } else if let Some(since) = latest_updated.clone() {
        let (changed, updated_at) = client.fetch_changed_mods(&since, sort_mode).await?;
        if !changed.is_empty() {
            items = bmi::apply_changed(&items, changed, &client)?;
        }
        latest_updated = bmi::pick_latest_updated(latest_updated, updated_at);
    }

    // Persist cache for future incremental sync
    let cache_state = SyncCache {
        last_updated_at: latest_updated,
        items: items.clone(),
    };
    let _ = std::fs::create_dir_all(&cache_dir);
    if let Ok(f) = std::fs::File::create(&cache_file) {
        let _ = serde_json::to_writer_pretty(f, &cache_state);
    }

    Ok(items)
}

fn bmi_cache_paths(sort: Option<&str>) -> Result<(PathBuf, PathBuf), String> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")).to_string())?;
    let cache_dir = config_dir.join("Balatro").join("mod_index_cache");
    let suffix = sort
        .unwrap_or("default")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    let cache_file = cache_dir.join(format!("{BMI_CACHE_FILE}_{suffix}.json"));
    Ok((cache_dir, cache_file))
}

fn load_bmi_cache(cache_file: &PathBuf) -> Option<SyncCache> {
    std::fs::File::open(cache_file)
        .ok()
        .and_then(|f| serde_json::from_reader::<_, SyncCache>(f).ok())
}

fn is_legal_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '!' | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '('
                | ')'
                | '+'
                | ','
                | '-'
                | '='
                | ';'
                | '@'
                | '['
                | ']'
                | '^'
                | '_'
                | '`'
                | '{'
                | '}'
                | '~'
        )
}

fn safe_slug(input: &str) -> String {
    let mut s = input.trim().to_lowercase();
    s = s
        .chars()
        .map(|c| if is_legal_char(c) { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').to_string()
}

fn ensure_assets_dirs() -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let config_dir = dirs::config_dir().ok_or_else(|| {
        AppError::DirNotFound(std::path::PathBuf::from("config directory")).to_string()
    })?;
    let base = config_dir.join("Balatro").join("mod_assets");
    let thumbs = base.join("thumbnails");
    let descs = base.join("descriptions");
    std::fs::create_dir_all(&thumbs).map_err(|e| {
        AppError::DirCreate {
            path: thumbs.clone(),
            source: e.to_string(),
        }
        .to_string()
    })?;
    std::fs::create_dir_all(&descs).map_err(|e| {
        AppError::DirCreate {
            path: descs.clone(),
            source: e.to_string(),
        }
        .to_string()
    })?;
    Ok((thumbs, descs))
}

#[tauri::command]
pub async fn get_cached_thumbnail_by_title(title: String) -> Result<Option<String>, String> {
    let (thumbs_dir, _) = ensure_assets_dirs()?;
    let slug = safe_slug(&title);
    let path = thumbs_dir.join(format!("{slug}.jpg"));
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(
        path.to_str()
            .ok_or_else(|| format!("Failed to convert thumbnail path: {}", path.display()))?
            .to_string(),
    ))
}

/// Return a map of title -> cached thumbnail path for the given titles (if present).
#[tauri::command]
pub async fn get_cached_thumbnails_map(
    titles: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let (thumbs_dir, _) = ensure_assets_dirs()?;
    let mut out = std::collections::HashMap::new();
    for title in titles {
        let slug = safe_slug(&title);
        let path = thumbs_dir.join(format!("{slug}.jpg"));
        if path.exists()
            && let Some(s) = path.to_str()
        {
            out.insert(title, s.to_string());
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn cache_thumbnail_from_url(
    title: String,
    url: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<bool, String> {
    // If present, no-op quickly
    let (thumbs_dir, _) = ensure_assets_dirs()?;
    let slug = safe_slug(&title);
    let path = thumbs_dir.join(format!("{slug}.jpg"));
    if path.exists() {
        return Ok(false);
    }

    // Enqueue background fetch with 429-aware backoff; return immediately
    log::info!("Thumbnail enqueue: title='{}' url='{}'", title, url);
    state.thumbs.enqueue(title, url);
    Ok(false)
}

#[tauri::command]
pub async fn get_cached_installed_thumbnail(
    title: String,
    dir_name: String,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<Option<String>, String> {
    let installed = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_installed_mods()
            .map_err(|e| e.to_string())?
            .into_iter()
            .any(|m| m.name.eq_ignore_ascii_case(&title))
    };
    if !installed {
        return Ok(None);
    }

    let (thumbs_dir, _) = ensure_assets_dirs()?;
    let slug = safe_slug(&title);
    let path = thumbs_dir.join(format!("{slug}.jpg"));
    if path.exists() {
        return Ok(Some(
            path.to_str()
                .ok_or_else(|| "Failed to convert thumbnail path".to_string())?
                .to_string(),
        ));
    }

    // Not cached yet: try to download from repo raw and store.
    let client = BmiClient::new()?;
    let url = client.thumbnail_url(&dir_name)?;
    let parsed = reqwest::Url::parse(&url).map_err(|e| e.to_string())?;
    match client.get_bytes(parsed).await {
        Ok(bytes) => {
            let bytes = match resolve_lfs_pointer_bytes(client.http_client(), bytes).await {
                Ok(resolved) => resolved,
                Err(LfsError::Retryable(_)) => {
                    state.thumbs.enqueue(title.clone(), url.to_string());
                    return Ok(None);
                }
                Err(_) => return Ok(None),
            };
            std::fs::write(&path, &bytes).map_err(|e| {
                AppError::FileWrite {
                    path: path.clone(),
                    source: e.to_string(),
                }
                .to_string()
            })?;
            return Ok(Some(
                path.to_str()
                    .ok_or_else(|| "Failed to convert thumbnail path".to_string())?
                    .to_string(),
            ));
        }
        Err(_) => {
            // Defer retry to the background queue for rate limits or transient failures.
            state.thumbs.enqueue(title.clone(), url.to_string());
        }
    }
    Ok(None)
}

#[tauri::command]
pub async fn get_description_cached_or_remote(
    title: String,
    dir_name: String,
    _state: tauri::State<'_, crate::state::AppState>,
) -> Result<String, String> {
    let (_, descs_dir) = ensure_assets_dirs()?;
    let slug = safe_slug(&title);
    let path = descs_dir.join(format!("{slug}.md"));

    // Always prefer cached copy if present
    if path.exists() {
        let cached = std::fs::read_to_string(&path).map_err(|e| {
            AppError::FileRead {
                path: path.clone(),
                source: e.to_string(),
            }
            .to_string()
        })?;
        if !cached.trim().is_empty() {
            return Ok(cached);
        }
    }

    let client = BmiClient::new()?;
    log::info!("Description fetch: title='{}' id='{}'", title, dir_name);
    let detail = client.fetch_mod(&dir_name).await?;
    let text = match detail.description_html.as_deref() {
        Some(desc) if !desc.trim().is_empty() => desc.to_string(),
        _ => match detail.description.as_deref() {
            Some(desc) if !desc.trim().is_empty() => desc.to_string(),
            _ => detail.summary.clone().unwrap_or_default(),
        },
    };
    if let Err(e) = std::fs::write(&path, &text) {
        log::warn!("Failed to cache description for {}: {}", title, e);
    }
    log::info!("Description loaded: title='{}' len={}", title, text.len());
    if text.trim().is_empty() {
        log::warn!("Description empty after fetch: title='{}'", title);
    }
    Ok(text)
}

#[tauri::command]
pub async fn get_cached_description_by_title(title: String) -> Result<Option<String>, String> {
    let (_, descs_dir) = ensure_assets_dirs()?;
    let slug = safe_slug(&title);
    let path = descs_dir.join(format!("{slug}.md"));
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| {
        AppError::FileRead {
            path: path.clone(),
            source: e.to_string(),
        }
        .to_string()
    })?;
    Ok(Some(text))
}

// ============ Batch Thumbnails (Raw URLs) ============

#[derive(Debug, Clone, Deserialize)]
pub struct ModThumbInput {
    pub title: String,
    pub dir_name: String,
}

#[tauri::command]
pub async fn batch_fetch_thumbnails_repo(inputs: Vec<ModThumbInput>) -> Result<u32, String> {
    use futures::{StreamExt, stream};

    // Ensure output directory exists early
    let (thumbs_dir, _) = ensure_assets_dirs()?;

    // Filter out inputs already cached
    let pending: Vec<ModThumbInput> = inputs
        .into_iter()
        .filter(|m| {
            let slug = safe_slug(&m.title);
            !thumbs_dir.join(format!("{slug}.jpg")).exists()
        })
        .collect();
    if pending.is_empty() {
        return Ok(0);
    }

    let client = BmiClient::new()?;

    let concurrency = 8usize;
    let saved = stream::iter(pending.into_iter())
        .map(|m| {
            let client = client.clone();
            let thumbs_dir = thumbs_dir.clone();
            async move {
                let url = match client.thumbnail_url(&m.dir_name) {
                    Ok(url) => url,
                    Err(_) => return 0u32,
                };
                let parsed = match reqwest::Url::parse(&url) {
                    Ok(url) => url,
                    Err(_) => return 0u32,
                };
                if let Ok(bytes) = client.get_bytes(parsed).await
                    && let Ok(bytes) = resolve_lfs_pointer_bytes(client.http_client(), bytes).await
                {
                    let slug = safe_slug(&m.title);
                    let path = thumbs_dir.join(format!("{slug}.jpg"));
                    if std::fs::write(&path, &bytes).is_ok() {
                        return 1u32;
                    }
                }
                0u32
            }
        })
        .buffer_unordered(concurrency)
        .fold(0u32, |acc, n| async move { acc + n })
        .await;

    Ok(saved)
}
