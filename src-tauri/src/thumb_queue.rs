use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use reqwest::StatusCode;

use crate::lfs::{LfsError, resolve_lfs_pointer_bytes};
use tokio::sync::{Semaphore, mpsc};
use tokio::time::{Duration, sleep};

/// Background thumbnail fetch request
#[derive(Clone, Debug)]
struct ThumbReq {
    title: String,
    url: String,
    attempts: u32,
}

/// Manager that rate-limits and retries thumbnail downloads in the background.
/// It honors 429 Retry-After when present, and uses exponential backoff for 5xx/network errors.
#[derive(Clone)]
pub struct ThumbnailManager {
    tx: mpsc::Sender<ThumbReq>,
    // Prevent duplicate queueing per title within a session
    enqueued: Arc<Mutex<HashSet<String>>>,
}

impl ThumbnailManager {
    pub fn new() -> Self {
        // Bounded queue to avoid memory spikes on first run
        let (tx, mut rx) = mpsc::channel::<ThumbReq>(1024);
        let enqueued: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        // Limit concurrent downloads to avoid rate limits
        let semaphore = Arc::new(Semaphore::new(3));
        let client = reqwest::Client::builder()
            .user_agent("balatro-mod-manager/1.0")
            .build()
            .expect("reqwest client");

        // Spawn dispatcher task
        let enq_for_task = enqueued.clone();
        let tx_for_dispatch = tx.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(mut req) = rx.recv().await {
                // Skip if file already exists or has been de-duped
                if file_exists_for_title(&req.title) {
                    // Remove from enqueued set so future explicit requests are allowed
                    if let Ok(mut set) = enq_for_task.lock() {
                        set.remove(&req.title);
                    }
                    continue;
                }

                let permit = semaphore.clone().acquire_owned().await;
                let client = client.clone();
                let enq_set = enq_for_task.clone();
                let tx_inner = tx_for_dispatch.clone();
                tauri::async_runtime::spawn(async move {
                    let _permit = permit.ok();
                    match fetch_and_store(&client, &req.title, &req.url).await {
                        Ok(true) => {
                            if let Ok(mut set) = enq_set.lock() {
                                set.remove(&req.title);
                            }
                        }
                        Ok(false) => {
                            // Non-retryable (e.g., 404/unsupported), drop and clear
                            if let Ok(mut set) = enq_set.lock() {
                                set.remove(&req.title);
                            }
                        }
                        Err(Backoff::RetryAfter(delay)) => {
                            // schedule retry after delay
                            req.attempts = req.attempts.saturating_add(1);
                            let title = req.title.clone();
                            tauri::async_runtime::spawn(async move {
                                sleep(delay).await;
                                // Put back into queue, keep enqueued flag as-is
                                let _ = tx_inner.send(req).await;
                                // If send fails, allow future enqueue by clearing mark
                                if let Ok(mut set) = enq_set.lock() {
                                    set.remove(&title);
                                }
                            });
                        }
                    }
                });
            }
        });

        Self { tx, enqueued }
    }

    /// Enqueue a single thumbnail request if not already present and not already cached.
    pub fn enqueue(&self, title: String, url: String) {
        if file_exists_for_title(&title) {
            return;
        }
        if let Ok(mut set) = self.enqueued.lock()
            && !set.insert(title.clone())
        {
            return; // already queued
        }
        let _ = self.tx.try_send(ThumbReq {
            title,
            url,
            attempts: 0,
        });
    }

    /// Enqueue multiple thumbnail requests.
    pub fn enqueue_many(&self, items: impl IntoIterator<Item = (String, String)>) {
        for (title, url) in items {
            self.enqueue(title, url);
        }
    }
}

impl Default for ThumbnailManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that indicate we should retry later with a delay
enum Backoff {
    RetryAfter(Duration),
}

async fn fetch_and_store(
    client: &reqwest::Client,
    title: &str,
    url: &str,
) -> Result<bool, Backoff> {
    // Don't waste network if already cached
    if file_exists_for_title(title) {
        return Ok(false);
    }

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return Err(Backoff::RetryAfter(jitter(Duration::from_secs(3)))),
    };

    match resp.status() {
        StatusCode::OK => {
            let bytes = match resp.bytes().await {
                Ok(b) => b.to_vec(),
                Err(_) => return Ok(false),
            };
            let bytes = match resolve_lfs_pointer_bytes(client, bytes).await {
                Ok(resolved) => resolved,
                Err(LfsError::Retryable(_)) => {
                    return Err(Backoff::RetryAfter(jitter(Duration::from_secs(4))));
                }
                Err(_) => return Ok(false),
            };
            if write_thumbnail(title, &bytes).is_err() {
                // Disk error; drop silently
                return Ok(false);
            }
            Ok(true)
        }
        StatusCode::TOO_MANY_REQUESTS => {
            let delay =
                retry_after_delay(resp.headers()).unwrap_or_else(|| jitter(Duration::from_secs(5)));
            Err(Backoff::RetryAfter(delay))
        }
        s if s.is_server_error() => Err(Backoff::RetryAfter(jitter(Duration::from_secs(4)))),
        StatusCode::NOT_FOUND | StatusCode::GONE => Ok(false),
        _ => Ok(false),
    }
}

fn jitter(base: Duration) -> Duration {
    // Small jitter based on current time millis; avoids extra deps
    use std::time::{SystemTime, UNIX_EPOCH};
    let base_ms = base.as_millis() as u64;
    let wiggle_base = (base_ms / 3).max(1);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let wiggle = now_ms % wiggle_base;
    Duration::from_millis(base_ms + wiggle)
}

fn retry_after_delay(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    use reqwest::header::RETRY_AFTER;
    if let Some(val) = headers.get(RETRY_AFTER)
        && let Ok(s) = val.to_str()
    {
        // Either seconds or HTTP-date
        if let Ok(secs) = s.trim().parse::<u64>() {
            return Some(Duration::from_secs(secs));
        }
        if let Ok(target) = httpdate::parse_http_date(s) {
            // Convert to duration from now; guard against past
            if let Ok(diff) = target.duration_since(std::time::SystemTime::now()) {
                return Some(diff);
            }
        }
    }
    None
}

fn file_exists_for_title(title: &str) -> bool {
    let slug = safe_slug(title);
    if let Ok((thumbs, _)) = ensure_assets_dirs() {
        let p = thumbs.join(format!("{slug}.jpg"));
        return p.exists();
    }
    false
}

fn write_thumbnail(title: &str, bytes: &[u8]) -> Result<(), String> {
    let slug = safe_slug(title);
    let (thumbs, _) = ensure_assets_dirs()?;
    let path = thumbs.join(format!("{slug}.jpg"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())
}

// Duplicated minimal helpers to avoid broad refactors; keep in sync with repo.rs
fn safe_slug(input: &str) -> String {
    let mut s = input.trim().to_lowercase();
    s = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').to_string()
}

fn ensure_assets_dirs() -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let config_dir = dirs::config_dir().ok_or_else(|| "config dir not found".to_string())?;
    let base = config_dir.join("Balatro").join("mod_assets");
    let thumbs = base.join("thumbnails");
    let descs = base.join("descriptions");
    std::fs::create_dir_all(&thumbs).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&descs).map_err(|e| e.to_string())?;
    Ok((thumbs, descs))
}
