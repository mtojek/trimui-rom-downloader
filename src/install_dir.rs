use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_BASE_DIR: &str = "/mnt/SDCARD/Roms";
const BASE_DIR_ENV: &str = "TRD_ROM_BASE_DIR";

pub struct InstallDirResolver {
    base_dir: PathBuf,
    cache: HashMap<String, PathBuf>,
}

impl InstallDirResolver {
    pub fn new() -> Self {
        let base_dir = std::env::var(BASE_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_BASE_DIR));
        let mut resolver = InstallDirResolver {
            base_dir,
            cache: HashMap::new(),
        };
        resolver.scan();
        resolver
    }

    fn scan(&mut self) {
        self.cache.clear();
        let entries = match fs::read_dir(&self.base_dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            if let Some(code) = extract_platform_code(&name) {
                self.cache.insert(code, entry.path());
            }
        }
    }

    pub fn resolve(&self, platform: &str) -> Option<&Path> {
        self.cache.get(platform).map(|p| p.as_path())
    }

    pub fn game_dir(&self, platform: &str, game_name: &str) -> Option<PathBuf> {
        self.resolve(platform).map(|p| p.join(game_name))
    }
}

fn extract_platform_code(dir_name: &str) -> Option<String> {
    let open = dir_name.rfind('(')?;
    let close = dir_name.rfind(')')?;
    if close > open + 1 {
        Some(dir_name[open + 1..close].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_roms(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("install_dir_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn resolver_with_base(base: &Path) -> InstallDirResolver {
        let mut resolver = InstallDirResolver {
            base_dir: base.to_path_buf(),
            cache: HashMap::new(),
        };
        resolver.scan();
        resolver
    }

    #[test]
    fn finds_platform_by_code() {
        let base = temp_roms("find");
        fs::create_dir_all(base.join("Sony PlayStation (PS)")).unwrap();
        fs::create_dir_all(base.join("Game Boy Advance (GBA)")).unwrap();
        fs::create_dir_all(base.join("Nintendo Entertainment System (FC)")).unwrap();

        let r = resolver_with_base(&base);
        assert_eq!(r.resolve("PS").unwrap(), base.join("Sony PlayStation (PS)"));
        assert_eq!(r.resolve("GBA").unwrap(), base.join("Game Boy Advance (GBA)"));
        assert_eq!(r.resolve("FC").unwrap(), base.join("Nintendo Entertainment System (FC)"));
        assert!(r.resolve("SNES").is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn game_dir_appends_name() {
        let base = temp_roms("game_dir");
        fs::create_dir_all(base.join("Sony PlayStation (PS)")).unwrap();

        let r = resolver_with_base(&base);
        assert_eq!(
            r.game_dir("PS", "Crash Bandicoot").unwrap(),
            base.join("Sony PlayStation (PS)/Crash Bandicoot"),
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn empty_base_returns_none() {
        let base = temp_roms("empty");
        let r = resolver_with_base(&base);
        assert!(r.resolve("PS").is_none());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn ignores_files_and_dirs_without_parens() {
        let base = temp_roms("ignore");
        fs::create_dir_all(base.join("random_folder")).unwrap();
        fs::write(base.join("some_file.txt"), "test").unwrap();
        fs::create_dir_all(base.join(".DS_Store_dir")).unwrap();

        let r = resolver_with_base(&base);
        assert!(r.cache.is_empty());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn extract_platform_code_works() {
        assert_eq!(extract_platform_code("Sony PlayStation (PS)"), Some("PS".to_string()));
        assert_eq!(extract_platform_code("Game Boy Advance (GBA)"), Some("GBA".to_string()));
        assert_eq!(extract_platform_code("no parens"), None);
        assert_eq!(extract_platform_code("empty ()"), None);
    }
}
