use std::cmp::Ordering;
use std::time::Duration;

use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::ModMeta;

pub const DEFAULT_BMI_SERVER_URL: &str = "http://localhost:8080";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: u32 = 3;
const PAGE_LIMIT: usize = 200;

#[derive(Clone, Copy, Debug)]
pub enum SortMode {
    NameAsc,
    NameDesc,
    UpdatedAsc,
    UpdatedDesc,
    DownloadsAsc,
    DownloadsDesc,
}

impl SortMode {
    pub fn from_optional(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("name_desc") => SortMode::NameDesc,
            Some("updated_asc") => SortMode::UpdatedAsc,
            Some("updated_desc") => SortMode::UpdatedDesc,
            Some("downloads_asc") => SortMode::DownloadsAsc,
            Some("downloads_desc") => SortMode::DownloadsDesc,
            _ => SortMode::NameAsc,
        }
    }

    fn as_param(&self) -> &'static str {
        match self {
            SortMode::NameAsc => "name_asc",
            SortMode::NameDesc => "name_desc",
            SortMode::UpdatedAsc => "updated_asc",
            SortMode::UpdatedDesc => "updated_desc",
            SortMode::DownloadsAsc => "downloads_asc",
            SortMode::DownloadsDesc => "downloads_desc",
        }
    }
}

#[derive(Clone)]
pub struct BmiClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BmiMod {
    #[serde(default, alias = "id", alias = "mod_id")]
    pub id: Option<String>,
    #[serde(default, alias = "dir_name", alias = "dirName", alias = "slug")]
    pub dir_name: Option<String>,
    #[serde(default, alias = "name", alias = "title")]
    pub name: Option<String>,
    #[serde(default, alias = "author", alias = "publisher")]
    pub author: Option<String>,
    #[serde(default, alias = "version")]
    pub version: Option<String>,
    #[serde(default, alias = "summary")]
    pub summary: Option<String>,
    #[serde(default, alias = "categories")]
    pub categories: Vec<String>,
    #[serde(default, alias = "repo", alias = "repository")]
    pub repo: Option<String>,
    #[serde(
        default,
        alias = "homepage",
        alias = "home_page",
        alias = "homepage_url"
    )]
    pub homepage: Option<String>,
    #[serde(default, alias = "requires_steamodded", alias = "requires-steamodded")]
    pub requires_steamodded: Option<bool>,
    #[serde(default, alias = "requires_talisman", alias = "requires-talisman")]
    pub requires_talisman: Option<bool>,
    #[serde(default, alias = "download_url", alias = "downloadURL")]
    pub download_url: Option<String>,
    #[serde(default, alias = "folder_name", alias = "folderName")]
    pub folder_name: Option<String>,
    #[serde(default)]
    pub meta: Option<ModMeta>,
    #[serde(
        default,
        alias = "description",
        alias = "body",
        alias = "content",
        alias = "markdown",
        alias = "readme",
        alias = "description_md",
        alias = "description_markdown"
    )]
    pub description: Option<String>,
    #[serde(default, alias = "description_html", alias = "descriptionHtml")]
    pub description_html: Option<String>,
    #[serde(
        default,
        alias = "thumbnail_url",
        alias = "thumbnailUrl",
        alias = "thumbnail"
    )]
    pub thumbnail_url: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_updated_at",
        alias = "updated_at",
        alias = "updatedAt",
        alias = "last_updated_at",
        alias = "lastUpdatedAt"
    )]
    pub updated_at: Option<String>,
    #[serde(default, alias = "deleted", alias = "is_deleted", alias = "removed")]
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ModsPage {
    #[serde(default, alias = "mods", alias = "items", alias = "data")]
    items: Vec<BmiMod>,
    #[serde(default, alias = "nextCursor", alias = "next_cursor")]
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCache {
    pub last_updated_at: Option<String>,
    pub items: Vec<crate::commands::repo::ArchiveModItem>,
}

impl BmiClient {
    pub fn new() -> Result<Self, String> {
        let base_url = bmi_server_url()?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent("balatro-mod-manager/1.0")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { base_url, client })
    }

    pub async fn check_health(&self) -> Result<(), String> {
        let url = self.base_url.join("healthz").map_err(|e| e.to_string())?;
        let resp = self
            .send_with_backoff(|| self.client.get(url.clone()))
            .await?;
        if resp.status().is_success() {
            return Ok(());
        }
        let status = resp.status();
        let body = resp.bytes().await.map_err(|e| e.to_string())?;
        Err(format!(
            "BMI healthz failed with status {}: {}",
            status,
            preview_bytes(&body)
        ))
    }

    pub fn thumbnail_url(&self, id: &str) -> Result<String, String> {
        let encoded = encode_path(id);
        let url = self
            .base_url
            .join(&format!("thumbnails/{encoded}.webp"))
            .map_err(|e| e.to_string())?;
        Ok(url.to_string())
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn get_bytes(&self, url: Url) -> Result<Vec<u8>, String> {
        let resp = self
            .send_with_backoff(|| self.client.get(url.clone()))
            .await?;
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| e.to_string())
    }

    pub async fn fetch_all_mods(
        &self,
        sort: SortMode,
    ) -> Result<(Vec<crate::commands::repo::ArchiveModItem>, Option<String>), String> {
        let mut cursor: Option<String> = None;
        let mut all = Vec::new();
        let mut latest_updated: Option<String> = None;
        loop {
            let page = self.fetch_mods_page(cursor.as_deref(), sort).await?;
            for item in page.items {
                let (archive, updated_at) = bmi_to_archive(item, self)?;
                if let Some(updated) = updated_at {
                    latest_updated = pick_latest_updated(latest_updated, Some(updated));
                }
                all.push(archive);
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok((all, latest_updated))
    }

    pub async fn fetch_changed_mods(
        &self,
        since: &str,
        sort: SortMode,
    ) -> Result<(Vec<BmiMod>, Option<String>), String> {
        let mut cursor: Option<String> = None;
        let mut all = Vec::new();
        let mut latest_updated: Option<String> = None;
        loop {
            let page = self
                .fetch_changed_page(since, cursor.as_deref(), sort)
                .await?;
            for item in page.items {
                let updated_at = updated_at_value(&item);
                if let Some(updated) = updated_at {
                    latest_updated = pick_latest_updated(latest_updated, Some(updated));
                }
                all.push(item);
            }
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok((all, latest_updated))
    }

    pub async fn fetch_mod(&self, id: &str) -> Result<BmiMod, String> {
        let encoded = encode_path(id);
        let url = self
            .base_url
            .join(&format!("mods/{encoded}"))
            .map_err(|e| e.to_string())?;
        let resp = self
            .send_with_backoff(|| self.client.get(url.clone()))
            .await?;
        decode_json(resp, "mod detail").await
    }

    pub async fn post_download(&self, id: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        struct DownloadResponse {
            #[serde(default, alias = "download_url", alias = "downloadUrl", alias = "url")]
            download_url: Option<String>,
        }

        let encoded = encode_path(id);
        let url = self
            .base_url
            .join(&format!("mods/{encoded}/download"))
            .map_err(|e| e.to_string())?;
        let resp = self
            .send_with_backoff(|| self.client.post(url.clone()))
            .await?;

        let status = resp.status();
        if status.as_u16() == 204 {
            let detail = self.fetch_mod(id).await?;
            return detail
                .download_url
                .ok_or_else(|| "BMI download URL missing in mod detail".to_string());
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.bytes().await.map_err(|e| e.to_string())?;
        if content_type.contains("application/json") {
            let parsed: DownloadResponse =
                serde_json::from_slice(&body).map_err(|e| e.to_string())?;
            if let Some(url) = parsed.download_url {
                return Ok(url);
            }
        }
        if status.is_success() {
            let detail = self.fetch_mod(id).await?;
            return detail
                .download_url
                .ok_or_else(|| "BMI download URL missing in mod detail".to_string());
        }
        Err(format!(
            "BMI download failed with status {}: {}",
            status,
            preview_bytes(&body)
        ))
    }

    async fn fetch_mods_page(
        &self,
        cursor: Option<&str>,
        sort: SortMode,
    ) -> Result<ModsPage, String> {
        let mut url = self.base_url.join("mods").map_err(|e| e.to_string())?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("limit", &PAGE_LIMIT.to_string());
            pairs.append_pair("sort", sort.as_param());
            if let Some(cursor) = cursor {
                pairs.append_pair("cursor", cursor);
            }
        }
        let resp = self
            .send_with_backoff(|| self.client.get(url.clone()))
            .await?;
        decode_json(resp, "mods page").await
    }

    async fn fetch_changed_page(
        &self,
        since: &str,
        cursor: Option<&str>,
        sort: SortMode,
    ) -> Result<ModsPage, String> {
        let mut url = self
            .base_url
            .join("mods/changed")
            .map_err(|e| e.to_string())?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("since", since);
            pairs.append_pair("limit", &PAGE_LIMIT.to_string());
            pairs.append_pair("sort", sort.as_param());
            if let Some(cursor) = cursor {
                pairs.append_pair("cursor", cursor);
            }
        }
        let resp = self
            .send_with_backoff(|| self.client.get(url.clone()))
            .await?;
        decode_json(resp, "mods changed page").await
    }

    async fn send_with_backoff<F>(&self, mut make_req: F) -> Result<reqwest::Response, String>
    where
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..=MAX_RETRIES {
            let resp = make_req().send().await.map_err(|e| e.to_string())?;
            if resp.status().as_u16() != 429 {
                return resp.error_for_status().map_err(|e| e.to_string());
            }
            if attempt == MAX_RETRIES {
                return Err("BMI rate limited after retries".to_string());
            }
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2);
        }
        Err("BMI request failed".to_string())
    }
}

async fn decode_json<T: DeserializeOwned>(
    resp: reqwest::Response,
    context: &str,
) -> Result<T, String> {
    let status = resp.status();
    let body = resp.bytes().await.map_err(|e| e.to_string())?;
    serde_json::from_slice::<T>(&body).map_err(|_e| {
        format!(
            "BMI {} decode failed (status {}): {}",
            context,
            status,
            preview_bytes(&body)
        )
    })
}

fn preview_bytes(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    if trimmed.len() > 400 {
        format!("{}...", &trimmed[..400])
    } else {
        trimmed.to_string()
    }
}

fn encode_path(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

pub fn bmi_server_url() -> Result<Url, String> {
    let raw =
        std::env::var("BMI_SERVER_URL").unwrap_or_else(|_| DEFAULT_BMI_SERVER_URL.to_string());
    let trimmed = raw.trim_end_matches('/');
    let base = format!("{trimmed}/");
    Url::parse(&base).map_err(|e| format!("Invalid BMI_SERVER_URL: {e}"))
}

pub fn bmi_to_archive(
    item: BmiMod,
    client: &BmiClient,
) -> Result<(crate::commands::repo::ArchiveModItem, Option<String>), String> {
    let id = item
        .id
        .clone()
        .or(item.dir_name.clone())
        .ok_or_else(|| "BMI mod missing id".to_string())?;
    let mut meta = match item.meta.clone() {
        Some(meta) => meta,
        None => ModMeta {
            requires_steamodded: item.requires_steamodded.unwrap_or(false),
            requires_talisman: item.requires_talisman.unwrap_or(false),
            categories: if item.categories.is_empty() {
                vec!["Miscellaneous".to_string()]
            } else {
                item.categories.clone()
            },
            author: item.author.clone().unwrap_or_else(|| "Unknown".to_string()),
            repo: item
                .repo
                .clone()
                .or(item.homepage.clone())
                .unwrap_or_default(),
            title: item.name.clone().unwrap_or_else(|| id.clone()),
            download_url: item.download_url.clone(),
            folder_name: item.folder_name.clone().unwrap_or_default(),
            version: item.version.clone().unwrap_or_default(),
            automatic_version_check: false,
            last_updated: item
                .updated_at
                .as_ref()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_default(),
        },
    };
    meta.download_url = Some(format!("bmi://{id}"));
    let updated_at = updated_at_value(&item).or_else(|| {
        if meta.last_updated > 0 {
            Some(meta.last_updated.to_string())
        } else {
            None
        }
    });
    let description = match item.description_html.as_deref() {
        Some(text) if !text.trim().is_empty() => text.to_string(),
        _ => match item.description.as_deref() {
            Some(text) if !text.trim().is_empty() => text.to_string(),
            _ => item.summary.clone().unwrap_or_default(),
        },
    };
    let image_url = match item.thumbnail_url {
        Some(url) if !url.is_empty() => normalize_thumbnail_url(client, &url)?,
        _ => client.thumbnail_url(&id)?,
    };
    Ok((
        crate::commands::repo::ArchiveModItem {
            dir_name: id,
            meta,
            description,
            image_url,
        },
        updated_at,
    ))
}

pub fn apply_changed(
    existing: &[crate::commands::repo::ArchiveModItem],
    changed: Vec<BmiMod>,
    client: &BmiClient,
) -> Result<Vec<crate::commands::repo::ArchiveModItem>, String> {
    let mut out: Vec<crate::commands::repo::ArchiveModItem> = existing.to_vec();
    let mut index: std::collections::HashMap<String, usize> = out
        .iter()
        .enumerate()
        .map(|(idx, item)| (item.dir_name.clone(), idx))
        .collect();
    for item in changed {
        let id = item
            .id
            .clone()
            .or(item.dir_name.clone())
            .ok_or_else(|| "BMI mod missing id".to_string())?;
        if item.deleted {
            if let Some(idx) = index.get(&id).copied() {
                out.remove(idx);
                index = out
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| (item.dir_name.clone(), idx))
                    .collect();
            }
            continue;
        }
        let (archive, _) = bmi_to_archive(item, client)?;
        if let Some(idx) = index.get(&id).copied() {
            out[idx] = archive;
        } else {
            index.insert(id, out.len());
            out.push(archive);
        }
    }
    Ok(out)
}

pub fn updated_at_value(item: &BmiMod) -> Option<String> {
    item.updated_at.clone().or_else(|| {
        item.meta
            .as_ref()
            .and_then(|meta| (meta.last_updated > 0).then(|| meta.last_updated.to_string()))
    })
}

fn deserialize_updated_at<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(num)) => Ok(Some(num.to_string())),
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(other) => Ok(Some(other.to_string())),
    }
}

fn normalize_thumbnail_url(client: &BmiClient, raw: &str) -> Result<String, String> {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        let mut url = Url::parse(raw).map_err(|e| e.to_string())?;
        if let Some(path) = normalize_thumbnail_path(url.path()) {
            url.set_path(&path);
        }
        return Ok(url.to_string());
    }
    let path = normalize_thumbnail_path(raw).unwrap_or_else(|| raw.to_string());
    client
        .base_url
        .join(path.trim_start_matches('/'))
        .map(|u| u.to_string())
        .map_err(|e| e.to_string())
}

fn normalize_thumbnail_path(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let prefix = "/thumbnails/";
    if !raw.starts_with(prefix) || !raw.ends_with(".webp") {
        return None;
    }
    let name = raw.trim_start_matches(prefix).trim_end_matches(".webp");
    let decoded = urlencoding::decode(name).ok().map(|s| s.into_owned());
    let encoded = urlencoding::encode(decoded.as_deref().unwrap_or(name)).into_owned();
    Some(format!("{prefix}{encoded}.webp"))
}

pub fn pick_latest_updated(current: Option<String>, candidate: Option<String>) -> Option<String> {
    match (current, candidate) {
        (None, None) => None,
        (Some(val), None) => Some(val),
        (None, Some(val)) => Some(val),
        (Some(a), Some(b)) => {
            if compare_updated_at(&a, &b) == Ordering::Less {
                Some(b)
            } else {
                Some(a)
            }
        }
    }
}

fn compare_updated_at(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| s.parse::<i64>().ok();
    match (parse(a), parse(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModMeta;

    fn sample_meta(title: &str, last_updated: u64) -> ModMeta {
        ModMeta {
            requires_steamodded: false,
            requires_talisman: false,
            categories: vec!["Content".to_string()],
            author: "Test".to_string(),
            repo: "https://example.com/repo".to_string(),
            title: title.to_string(),
            download_url: Some("bmi://mod-1".to_string()),
            folder_name: "mod1".to_string(),
            version: "1.0.0".to_string(),
            automatic_version_check: false,
            last_updated,
        }
    }

    #[test]
    fn apply_changed_updates_and_deletes() {
        let client = BmiClient::new().expect("client");
        let existing = vec![
            crate::commands::repo::ArchiveModItem {
                dir_name: "mod-1".to_string(),
                meta: sample_meta("Alpha", 10),
                description: "Old".to_string(),
                image_url: "https://example.com/thumb.webp".to_string(),
            },
            crate::commands::repo::ArchiveModItem {
                dir_name: "mod-2".to_string(),
                meta: sample_meta("Beta", 20),
                description: "Keep".to_string(),
                image_url: "https://example.com/thumb2.webp".to_string(),
            },
        ];
        let changed = vec![
            BmiMod {
                id: Some("mod-1".to_string()),
                dir_name: None,
                name: None,
                author: None,
                version: None,
                summary: None,
                categories: Vec::new(),
                repo: None,
                homepage: None,
                requires_steamodded: None,
                requires_talisman: None,
                download_url: None,
                folder_name: None,
                meta: Some(sample_meta("Alpha Updated", 30)),
                description: Some("New".to_string()),
                description_html: None,
                thumbnail_url: Some("https://example.com/new.webp".to_string()),
                updated_at: Some("30".to_string()),
                deleted: false,
            },
            BmiMod {
                id: Some("mod-2".to_string()),
                dir_name: None,
                name: None,
                author: None,
                version: None,
                summary: None,
                categories: Vec::new(),
                repo: None,
                homepage: None,
                requires_steamodded: None,
                requires_talisman: None,
                download_url: None,
                folder_name: None,
                meta: None,
                description: None,
                description_html: None,
                thumbnail_url: None,
                updated_at: None,
                deleted: true,
            },
        ];

        let merged = apply_changed(&existing, changed, &client).expect("merge");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].dir_name, "mod-1");
        assert_eq!(merged[0].meta.title, "Alpha Updated");
        assert_eq!(merged[0].description, "New");
    }

    #[test]
    fn paging_stops_when_cursor_is_none() {
        let client = BmiClient::new().expect("client");
        let pages = vec![
            ModsPage {
                items: vec![BmiMod {
                    id: Some("mod-1".to_string()),
                    dir_name: None,
                    name: None,
                    author: None,
                    version: None,
                    summary: None,
                    categories: Vec::new(),
                    repo: None,
                    homepage: None,
                    requires_steamodded: None,
                    requires_talisman: None,
                    download_url: None,
                    folder_name: None,
                    meta: Some(sample_meta("Alpha", 5)),
                    description: Some("First".to_string()),
                    description_html: None,
                    thumbnail_url: None,
                    updated_at: Some("5".to_string()),
                    deleted: false,
                }],
                next_cursor: Some("next".to_string()),
            },
            ModsPage {
                items: vec![BmiMod {
                    id: Some("mod-2".to_string()),
                    dir_name: None,
                    name: None,
                    author: None,
                    version: None,
                    summary: None,
                    categories: Vec::new(),
                    repo: None,
                    homepage: None,
                    requires_steamodded: None,
                    requires_talisman: None,
                    download_url: None,
                    folder_name: None,
                    meta: Some(sample_meta("Beta", 8)),
                    description: Some("Second".to_string()),
                    description_html: None,
                    thumbnail_url: None,
                    updated_at: Some("8".to_string()),
                    deleted: false,
                }],
                next_cursor: None,
            },
            ModsPage {
                items: vec![BmiMod {
                    id: Some("mod-3".to_string()),
                    dir_name: None,
                    name: None,
                    author: None,
                    version: None,
                    summary: None,
                    categories: Vec::new(),
                    repo: None,
                    homepage: None,
                    requires_steamodded: None,
                    requires_talisman: None,
                    download_url: None,
                    folder_name: None,
                    meta: Some(sample_meta("Gamma", 12)),
                    description: Some("Third".to_string()),
                    description_html: None,
                    thumbnail_url: None,
                    updated_at: Some("12".to_string()),
                    deleted: false,
                }],
                next_cursor: Some("ignored".to_string()),
            },
        ];

        let mut collected = Vec::new();
        let mut latest = None;
        for page in pages {
            for item in page.items {
                let (archive, updated_at) = bmi_to_archive(item, &client).expect("archive");
                if let Some(updated) = updated_at {
                    latest = pick_latest_updated(latest, Some(updated));
                }
                collected.push(archive);
            }
            if page.next_cursor.is_none() {
                break;
            }
        }

        assert_eq!(collected.len(), 2);
        assert_eq!(latest, Some("8".to_string()));
    }

    #[test]
    fn pick_latest_updated_prefers_larger_numeric() {
        let updated = pick_latest_updated(Some("10".to_string()), Some("20".to_string()));
        assert_eq!(updated, Some("20".to_string()));
    }
}
