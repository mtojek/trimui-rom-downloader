use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const DATA_DIR_NAME: &str = ".rom-downloader";
const LIBRARY_FILE: &str = "mygames.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub key: String,
    pub source: String,
    pub platform: String,
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
        Self::from_path(exe_dir.join(DATA_DIR_NAME).join(LIBRARY_FILE))
    }

    fn from_path(path: PathBuf) -> Self {
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

        self.entries.retain(|e| {
            !(e.source == source && e.platform == platform && e.key == key)
        });

        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_library(name: &str) -> (MyGames, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mygames_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mygames.yaml");
        (MyGames::from_path(path), dir)
    }

    fn sample_entry(key: &str) -> GameEntry {
        GameEntry {
            key: key.to_string(),
            source: "Internet Archive".to_string(),
            platform: PS.to_string(),
        }
    }

    const PS: &str = "PS";

    #[test]
    fn add_and_is_installed() {
        let (mut lib, dir) = temp_library("add");
        assert!(!lib.is_installed("Internet Archive", PS, "Crash Bandicoot"));

        lib.add(sample_entry("Crash Bandicoot")).unwrap();
        assert!(lib.is_installed("Internet Archive", PS, "Crash Bandicoot"));
        assert_eq!(lib.list().len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_is_idempotent() {
        let (mut lib, dir) = temp_library("idempotent");
        lib.add(sample_entry("Spyro the Dragon")).unwrap();
        lib.add(sample_entry("Spyro the Dragon")).unwrap();
        assert_eq!(lib.list().len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_entry() {
        let (mut lib, dir) = temp_library("remove");
        lib.add(sample_entry("Crash Bandicoot")).unwrap();
        lib.add(sample_entry("Spyro the Dragon")).unwrap();
        assert_eq!(lib.list().len(), 2);

        lib.remove("Internet Archive", PS, "Crash Bandicoot").unwrap();
        assert!(!lib.is_installed("Internet Archive", PS, "Crash Bandicoot"));
        assert!(lib.is_installed("Internet Archive", PS, "Spyro the Dragon"));
        assert_eq!(lib.list().len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_nonexistent_is_ok() {
        let (mut lib, dir) = temp_library("remove_none");
        lib.remove("Internet Archive", PS, "nope").unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn persists_to_disk_and_reloads() {
        let (mut lib, dir) = temp_library("persist");
        lib.add(sample_entry("Crash Bandicoot")).unwrap();
        lib.add(sample_entry("Spyro the Dragon")).unwrap();

        // Reload from disk
        let lib2 = MyGames::from_path(dir.join("mygames.yaml"));
        assert_eq!(lib2.list().len(), 2);
        assert!(lib2.is_installed("Internet Archive", PS, "Crash Bandicoot"));
        assert!(lib2.is_installed("Internet Archive", PS, "Spyro the Dragon"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn yaml_matches_golden_file() {
        let (mut lib, dir) = temp_library("golden");
        lib.add(GameEntry {
            key: "Crash Bandicoot".to_string(),
            source: "Internet Archive".to_string(),
            platform: PS.to_string(),
        }).unwrap();
        lib.add(GameEntry {
            key: "Spyro the Dragon".to_string(),
            source: "Internet Archive".to_string(),
            platform: PS.to_string(),
        }).unwrap();

        let actual = fs::read_to_string(dir.join("mygames.yaml")).unwrap();
        let golden = include_str!("../testdata/mygames_example.yaml");
        assert_eq!(actual, golden, "YAML output does not match golden file testdata/mygames_example.yaml");

        let _ = fs::remove_dir_all(&dir);
    }
}
