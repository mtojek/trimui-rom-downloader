use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const DATA_DIR_NAME: &str = ".rom-downloader";
const LIBRARY_FILE: &str = "mygames.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub source: String,
    pub platform: String,
    pub key: String,
    pub install_path: String,
    pub file_size: u64,
}

#[derive(Debug)]
pub enum LibraryError {
    IoError(String),
    ParseError(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::IoError(msg) => write!(f, "Library IO error: {}", msg),
            LibraryError::ParseError(msg) => write!(f, "Library parse error: {}", msg),
        }
    }
}

pub struct MyGames {
    path: PathBuf,
    entries: Vec<GameEntry>,
    installed: HashSet<(String, String, String)>,
}

fn make_key(source: &str, platform: &str, key: &str) -> (String, String, String) {
    (source.to_string(), platform.to_string(), key.to_string())
}

impl MyGames {
    pub fn new() -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let path = exe_dir.join(DATA_DIR_NAME).join(LIBRARY_FILE);

        let mut lib = MyGames {
            path,
            entries: Vec::new(),
            installed: HashSet::new(),
        };
        let _ = lib.load();
        lib
    }

    fn load(&mut self) -> Result<(), LibraryError> {
        if !self.path.exists() {
            return Ok(());
        }
        let contents =
            fs::read_to_string(&self.path).map_err(|e| LibraryError::IoError(e.to_string()))?;
        let entries: Vec<GameEntry> =
            serde_yaml::from_str(&contents).map_err(|e| LibraryError::ParseError(e.to_string()))?;

        self.installed.clear();
        for entry in &entries {
            self.installed
                .insert(make_key(&entry.source, &entry.platform, &entry.key));
        }
        self.entries = entries;
        Ok(())
    }

    fn save(&self) -> Result<(), LibraryError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| LibraryError::IoError(e.to_string()))?;
        }
        let yaml =
            serde_yaml::to_string(&self.entries).map_err(|e| LibraryError::IoError(e.to_string()))?;
        fs::write(&self.path, yaml).map_err(|e| LibraryError::IoError(e.to_string()))
    }

    pub fn is_installed(&self, source: &str, platform: &str, key: &str) -> bool {
        self.installed.contains(&make_key(source, platform, key))
    }

    pub fn list(&self) -> &[GameEntry] {
        &self.entries
    }

    pub fn add(&mut self, entry: GameEntry) -> Result<(), LibraryError> {
        let key = make_key(&entry.source, &entry.platform, &entry.key);
        if self.installed.contains(&key) {
            return Ok(());
        }
        self.installed.insert(key);
        self.entries.push(entry);
        self.save()
    }

    pub fn remove(&mut self, source: &str, platform: &str, key: &str) -> Result<(), LibraryError> {
        let lookup = make_key(source, platform, key);
        if !self.installed.remove(&lookup) {
            return Ok(());
        }

        // Find and remove entry, delete file from disk
        if let Some(pos) = self.entries.iter().position(|e| {
            e.source == source && e.platform == platform && e.key == key
        }) {
            let entry = self.entries.remove(pos);
            let path = PathBuf::from(&entry.install_path);
            if path.exists() {
                if path.is_dir() {
                    let _ = fs::remove_dir_all(&path);
                } else {
                    let _ = fs::remove_file(&path);
                }
            }
        }

        self.save()
    }
}
