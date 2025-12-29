use std::path::PathBuf;

use crate::lfs::{LfsError, resolve_lfs_pointer_bytes};
use crate::models::ModMeta;
use bmm_lib::errors::AppError;
use serde::{Deserialize, Serialize};

const REPO_MEDIA_MAIN: &str =
    "http://smallgit.dasguney.com:3000/skyline/balatro-mod-index/media/branch/main";
const REPO_ARCHIVE_URL: &str =
    "http://smallgit.dasguney.com:3000/skyline/balatro-mod-index/archive/main.tar.gz";

fn mod_path_parts(path: &std::path::Path) -> Option<(String, String)> {
    let comps: Vec<_> = path.components().collect();
    for i in 0..comps.len() {
        if comps[i].as_os_str() == std::ffi::OsStr::new("mods") {
            if comps.len() < i + 3 {
                return None;
            }
            let dir = match comps[i + 1] {
                std::path::Component::Normal(n) => n.to_string_lossy().to_string(),
                _ => return None,
            };
            let file = match comps[i + 2] {
                std::path::Component::Normal(n) => n.to_string_lossy().to_string(),
                _ => return None,
            };
            return Some((dir, file));
        }
    }
    None
}

#[tauri::command]
pub async fn list_repo_mods() -> Result<Vec<String>, String> {
    use flate2::read::GzDecoder;
    use std::collections::HashSet;
    use std::io::Read;
    use tar::Archive;

    let resp = reqwest::get(REPO_ARCHIVE_URL)
        .await
        .map_err(|e| format!("Repo archive error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Repo archive status: {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let mut dec = GzDecoder::new(bytes.as_ref());
    let mut tar_bytes = Vec::with_capacity(bytes.len());
    dec.read_to_end(&mut tar_bytes)
        .map_err(|e| format!("Archive decompress error: {}", e))?;
    let mut archive = Archive::new(std::io::Cursor::new(tar_bytes));

    let mut dirs: HashSet<String> = HashSet::new();
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = match entry.path() {
            Ok(p) => p.into_owned(),
            Err(_) => continue,
        };
        if let Some((dir, _file)) = mod_path_parts(&path) {
            dirs.insert(dir);
        }
    }
    let mut out: Vec<String> = dirs.into_iter().collect();
    out.sort();
    Ok(out)
}

#[tauri::command]
pub async fn get_repo_file(path: &str) -> Result<String, String> {
    use tokio::time::{Duration, sleep};

    // Encode path by segments so slashes remain
    let encoded: String = path
        .split('/')
        .map(urlencoding::encode)
        .map(|s| s.into_owned())
        .collect::<Vec<_>>()
        .join("/");

    let url = format!("{}/{}", REPO_MEDIA_MAIN, encoded);

    let client = reqwest::Client::new();
    let mut delay = Duration::from_millis(250);
    for attempt in 0..4 {
        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
            let bytes = resolve_lfs_pointer_bytes(&client, bytes)
                .await
                .map_err(|e| format!("{e}"))?;
            let text = String::from_utf8(bytes.to_vec())
                .map_err(|e| format!("Invalid UTF-8 from repo: {e}"))?;
            return Ok(text);
        }
        let code = resp.status().as_u16();
        // 404/410: not found — no point retrying this URL
        if code == 404 || code == 410 {
            break;
        }
        // 429/5xx: temporary, retry after delay
        if attempt < 3 {
            sleep(delay).await;
            delay = delay.saturating_mul(2);
        }
    }
    Err(format!("Failed to fetch {} after retries", path))
}

#[allow(non_snake_case)]
#[tauri::command]
pub async fn get_repo_thumbnail_url(dirName: String) -> Result<Option<String>, String> {
    // Try unencoded then encoded
    let enc = urlencoding::encode(&dirName);
    let candidates = [
        format!("{}/mods/{}/thumbnail.jpg", REPO_MEDIA_MAIN, dirName),
        format!("{}/mods/{}/thumbnail.jpg", REPO_MEDIA_MAIN, enc),
    ];

    use reqwest::header::RANGE;
    let client = reqwest::Client::new();
    for url in candidates {
        let resp = client
            .get(&url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            return Ok(Some(url));
        }
    }
    Ok(None)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ArchiveModItem {
    pub dir_name: String,
    pub meta: ModMeta,
    pub description: String,
    pub image_url: String,
}

// Fetch all mod metadata and descriptions via a single archive request.
// LFS pointers are resolved via the Git LFS batch API when needed.
#[tauri::command]
pub async fn fetch_repo_mods() -> Result<Vec<ArchiveModItem>, String> {
    use flate2::read::GzDecoder;
    use std::time::Instant;
    use tar::Archive;

    // Cache location in config dir (created lazily when writing)
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::DirNotFound(PathBuf::from("config directory")).to_string())?;
    let cache_dir = config_dir.join("Balatro").join("mod_index_cache");
    let cache_file = cache_dir.join("mod_index_archive.json");

    #[derive(Serialize, Deserialize)]
    struct ArchiveCache {
        etag: Option<String>,
        branch: String,
        items: Vec<ArchiveModItem>,
    }

    // Try to load existing cache to get ETag and for offline fallback
    let existing_cache: Option<ArchiveCache> = std::fs::File::open(&cache_file)
        .ok()
        .and_then(|f| serde_json::from_reader::<_, ArchiveCache>(f).ok());

    let url = REPO_ARCHIVE_URL;

    // Download archive stream
    use reqwest::header::IF_NONE_MATCH;
    let client = reqwest::Client::new();
    let mut received_etag: Option<String> = None;
    let mut req = client.get(url);
    if let Some(c) = &existing_cache
        && let Some(et) = &c.etag
    {
        req = req.header(IF_NONE_MATCH, et);
    }
    let mut resp = req
        .send()
        .await
        .map_err(|e| format!("Repo archive request error: {e}"))?;
    if resp.status().as_u16() == 304 {
        if let Some(c) = existing_cache {
            if !c.items.is_empty() {
                log::info!(
                    "Repo archive 304 Not Modified, using cached items: {}",
                    c.items.len()
                );
                return Ok(c.items);
            }
            log::info!("Repo archive 304 with empty cache; refetching archive");
        }
        resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Repo archive retry error: {e}"))?;
    }
    if !resp.status().is_success() {
        return Err(format!("Repo archive status: {}", resp.status()));
    }
    if let Some(et) = resp.headers().get(reqwest::header::ETAG)
        && let Ok(s) = et.to_str()
    {
        received_etag = Some(s.to_string());
    }

    // Stream download to a temp file to avoid holding the full archive in memory.
    let tmp_path = cache_dir.join("mods_archive.tmp");
    // Ensure directory exists before writing temp archive
    let _ = std::fs::create_dir_all(&cache_dir);
    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&tmp_path)
            .await
            .map_err(|e| format!("Failed to create temp archive file: {e}"))?;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = tokio_stream::StreamExt::next(&mut stream).await {
            let bytes = chunk.map_err(|e| format!("Download error: {e}"))?;
            file.write_all(&bytes)
                .await
                .map_err(|e| format!("Failed writing archive: {e}"))?;
        }
        file.sync_all()
            .await
            .map_err(|e| format!("Failed to flush archive: {e}"))?;
    }

    // Decompress and iterate entries from disk
    let parse_start = Instant::now();
    let file =
        std::fs::File::open(&tmp_path).map_err(|e| format!("Failed to reopen archive: {e}"))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);

    let mut dir_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in archive.entries().map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = match entry.path() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let (dir_name, filename) = match mod_path_parts(&path) {
            Some(parts) => parts,
            None => continue,
        };
        if filename == "meta.json" || filename == "description.md" {
            dir_set.insert(dir_name);
        }
    }

    // Build result by fetching meta/description via raw URLs with LFS pointer resolution.
    use futures::{StreamExt, stream};
    let client = reqwest::Client::new();
    let concurrency = 12usize;
    let results = stream::iter(dir_set.into_iter())
        .map(|dir| {
            let client = client.clone();
            async move {
                let dir_name = dir.clone();
                let dir_enc = urlencoding::encode(&dir_name).into_owned();
                let meta_url = format!("{}/mods/{}/meta.json", REPO_MEDIA_MAIN, dir_enc);
                let desc_url = format!("{}/mods/{}/description.md", REPO_MEDIA_MAIN, dir_enc);

                let meta = match client.get(&meta_url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let bytes = resp.bytes().await.ok()?.to_vec();
                        let bytes = resolve_lfs_pointer_bytes(&client, bytes).await.ok()?;
                        let text = String::from_utf8(bytes).ok()?;
                        serde_json::from_str::<ModMeta>(&text).ok()?
                    }
                    _ => return None,
                };

                let description = match client.get(&desc_url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let bytes = resp.bytes().await.ok()?.to_vec();
                        let bytes = resolve_lfs_pointer_bytes(&client, bytes).await.ok()?;
                        String::from_utf8(bytes).unwrap_or_default()
                    }
                    _ => String::new(),
                };

                Some(ArchiveModItem {
                    dir_name,
                    meta,
                    description,
                    image_url: format!("{}/mods/{}/thumbnail.jpg", REPO_MEDIA_MAIN, dir_enc),
                })
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    let mut out: Vec<ArchiveModItem> = results.into_iter().flatten().collect();

    // Sort by title asc for stability
    out.sort_by(|a, b| {
        a.meta
            .title
            .to_lowercase()
            .cmp(&b.meta.title.to_lowercase())
    });

    log::info!(
        "Parsed {} mods from archive in {} ms",
        out.len(),
        parse_start.elapsed().as_millis()
    );

    // Save cache with ETag for future 304 validations
    let cache = ArchiveCache {
        etag: received_etag,
        branch: "main".to_string(),
        items: out.clone(),
    };
    // Ensure directory exists before writing
    let _ = std::fs::create_dir_all(&cache_dir);
    if let Ok(f) = std::fs::File::create(&cache_file) {
        let _ = serde_json::to_writer_pretty(f, &cache);
    }

    // Clean up temp archive
    let _ = std::fs::remove_file(&tmp_path);

    Ok(out)
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
    let client = reqwest::Client::new();
    let enc = urlencoding::encode(&dir_name);
    let url = format!("{}/mods/{}/thumbnail.jpg", REPO_MEDIA_MAIN, enc);
    if let Ok(resp) = client.get(&url).send().await {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                let bytes = bytes.to_vec();
                let bytes = match resolve_lfs_pointer_bytes(&client, bytes).await {
                    Ok(resolved) => resolved,
                    Err(LfsError::Retryable(_)) => {
                        state.thumbs.enqueue(title.clone(), url.to_string());
                        return Ok(None);
                    }
                    Err(_) => return Ok(None),
                };
                // Persist and return
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
        } else if resp.status().as_u16() == 429 {
            // Handle rate limiting in the background; keep UI unblocked
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
        return std::fs::read_to_string(&path).map_err(|e| {
            AppError::FileRead {
                path,
                source: e.to_string(),
            }
            .to_string()
        });
    }

    // Fetch from repo raw
    let client = reqwest::Client::new();
    let enc = urlencoding::encode(&dir_name);
    let url = format!("{}/mods/{}/description.md", REPO_MEDIA_MAIN, enc);
    if let Ok(resp) = client.get(&url).send().await
        && resp.status().is_success()
        && let Ok(bytes) = resp.bytes().await
    {
        let bytes = resolve_lfs_pointer_bytes(&client, bytes.to_vec())
            .await
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {e}"))?;
        // Cache for future sessions regardless of install state
        if let Err(e) = std::fs::write(&path, &text) {
            log::warn!("Failed to cache description for {}: {}", title, e);
        }
        return Ok(text);
    }
    Err(format!("Description not found for {}", dir_name))
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

    let client = reqwest::Client::builder()
        .user_agent("balatro-mod-manager/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let concurrency = 8usize;
    let saved = stream::iter(pending.into_iter())
        .map(|m| {
            let client = client.clone();
            let thumbs_dir = thumbs_dir.clone();
            async move {
                let url = format!(
                    "{}/mods/{}/thumbnail.jpg",
                    REPO_MEDIA_MAIN,
                    urlencoding::encode(&m.dir_name)
                );
                if let Ok(resp) = client.get(&url).send().await
                    && resp.status().is_success()
                    && let Ok(bytes) = resp.bytes().await
                    && let Ok(bytes) = resolve_lfs_pointer_bytes(&client, bytes.to_vec()).await
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
