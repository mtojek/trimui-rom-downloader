use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::backend::RemoteGame;
use crate::config::Catalog;

const STALE_THRESHOLD: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
const CACHE_DIR_NAME: &str = ".rom-downloader";

#[derive(Debug)]
pub enum CacheError {
    IoError(String),
    ParseError(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::IoError(msg) => write!(f, "Cache IO error: {}", msg),
            CacheError::ParseError(msg) => write!(f, "Cache parse error: {}", msg),
        }
    }
}

pub struct CatalogCache {
    cache_dir: PathBuf,
}

impl CatalogCache {
    pub fn new() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        CatalogCache {
            cache_dir: exe_dir.join(CACHE_DIR_NAME),
        }
    }

    fn cache_path(&self, source_name: &str, catalog: &Catalog) -> PathBuf {
        let dir_name = source_name.replace('/', "_").replace(' ', "_");
        let filename = format!("{}.yaml", catalog.path.replace('/', "_"));
        self.cache_dir.join("cache").join("sources").join(dir_name).join(filename)
    }

    pub fn is_stale(&self, source_name: &str, catalog: &Catalog) -> bool {
        let path = self.cache_path(source_name, catalog);
        if !path.exists() {
            return true;
        }

        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        SystemTime::now()
            .duration_since(modified)
            .unwrap_or(STALE_THRESHOLD)
            >= STALE_THRESHOLD
    }

    pub fn load(&self, source_name: &str, catalog: &Catalog) -> Result<Vec<RemoteGame>, CacheError> {
        let path = self.cache_path(source_name, catalog);
        let contents =
            fs::read_to_string(&path).map_err(|e| CacheError::IoError(e.to_string()))?;
        serde_yaml::from_str(&contents).map_err(|e| CacheError::ParseError(e.to_string()))
    }

    pub fn save(&self, source_name: &str, catalog: &Catalog, games: &[RemoteGame]) -> Result<(), CacheError> {
        let path = self.cache_path(source_name, catalog);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| CacheError::IoError(e.to_string()))?;
        }
        let yaml = serde_yaml::to_string(games).map_err(|e| CacheError::IoError(e.to_string()))?;
        fs::write(&path, yaml).map_err(|e| CacheError::IoError(e.to_string()))
    }

    pub fn invalidate(&self, source_name: &str, catalog: &Catalog) -> Result<(), CacheError> {
        let path = self.cache_path(source_name, catalog);
        if path.exists() {
            fs::remove_file(&path).map_err(|e| CacheError::IoError(e.to_string()))?;
        }
        Ok(())
    }
}
