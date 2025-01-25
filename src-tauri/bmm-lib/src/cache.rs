use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use serde_repr::*;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_DURATION: u64 = 15 * 60; // 15 minutes in seconds

#[derive(Serialize, Deserialize, Debug)]
struct CacheHeader {
    version: u32,
    timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ModCache {
    header: CacheHeader,
    mods: Vec<Mod>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Mod {
    pub title: String,
    pub description: String,
    pub image: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
    #[serde(rename = "categories")]
    pub categories: Vec<Category>,
    #[serde(rename = "colors")]
    pub colors: ColorPair,
    pub installed: bool,
    #[serde(rename = "requires_steamodded")]
    pub requires_steamodded: bool,
    #[serde(rename = "requires_talisman")]
    pub requires_talisman: bool,
    pub publisher: String,
    pub repo: String,
    #[serde(rename = "downloadURL")]
    pub download_url: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ColorPair {
    pub color1: String,
    pub color2: String,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum Category {
    Content = 0,
    Joker = 1,
    QualityOfLife = 2,
    Technical = 3,
    Miscellaneous = 4,
    ResourcePacks = 5,
    API = 6,
}

impl From<i32> for Category {
    fn from(value: i32) -> Self {
        match value {
            0 => Category::Content,
            1 => Category::Joker,
            2 => Category::QualityOfLife,
            3 => Category::Technical,
            4 => Category::Miscellaneous,
            5 => Category::ResourcePacks,
            6 => Category::API,
            _ => panic!("Invalid category index: {}", value),
        }
    }
}

// Add to your existing cache module
pub fn save_versions_cache(mod_type: &str, versions: &[String]) -> Result<()> {
    let mut path = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("balatro-mod-manager");

    std::fs::create_dir_all(&path)?;
    path.push(format!("versions-{}.cache.bin.gz", mod_type));

    let mut encoder = GzEncoder::new(File::create(&path)?, Compression::default());
    let cache = VersionCache {
        header: CacheHeader {
            version: 1,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        },
        versions: versions.to_vec(),
    };

    bincode::serialize_into(&mut encoder, &cache)?;
    Ok(())
}

pub fn load_versions_cache(mod_type: &str) -> Result<Option<Vec<String>>> {
    let path = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("balatro-mod-manager")
        .join(format!("versions-{}.cache.bin.gz", mod_type));

    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let decoder = GzDecoder::new(buffer.as_slice());

    let cache: VersionCache = match bincode::deserialize_from(decoder) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    // Validate cache
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if current_time - cache.header.timestamp > CACHE_DURATION {
        return Ok(None);
    }

    Ok(Some(cache.versions))
}

// Add these structs to cache.rs
#[derive(Serialize, Deserialize)]
struct VersionCache {
    header: CacheHeader,
    versions: Vec<String>,
}

pub fn get_cache_path() -> Result<PathBuf> {
    let mut path = dirs::cache_dir()
        .context("Failed to get cache directory")?
        .join("balatro-mod-manager");

    std::fs::create_dir_all(&path)?;
    path.push("mods.cache.bin.gz");
    Ok(path)
}

pub fn save_cache(mods: &[Mod]) -> Result<()> {
    let path = get_cache_path()?;
    let mut encoder = GzEncoder::new(File::create(&path)?, Compression::default());

    let cache = ModCache {
        header: CacheHeader {
            version: 1,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        },
        mods: mods.to_vec(),
    };

    bincode::serialize_into(&mut encoder, &cache)?;
    Ok(())
}

pub fn load_cache() -> Result<Option<(Vec<Mod>, u64)>> {
    let path = get_cache_path()?;
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let decoder = GzDecoder::new(buffer.as_slice());
    let cache: ModCache = match bincode::deserialize_from(decoder) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    // Validate cache version
    if cache.header.version != 1 {
        return Ok(None);
    }

    // Check expiration
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if current_time - cache.header.timestamp > CACHE_DURATION {
        return Ok(None);
    }

    Ok(Some((cache.mods, cache.header.timestamp)))
}
