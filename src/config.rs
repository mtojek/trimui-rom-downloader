use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub sources: Vec<Source>,
    #[serde(default)]
    pub credentials: HashMap<String, Credentials>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub credentials: String,
    pub platform: String,
    pub buckets: Vec<Bucket>,
    #[serde(default)]
    pub extract: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bucket {
    pub name: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    S3Archive,
}

#[derive(Debug)]
pub enum ConfigError {
    NotFound(String),
    ParseError(String),
    ValidationError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(path) => write!(f, "Config file not found: {}", path),
            ConfigError::ParseError(msg) => write!(f, "Failed to parse config: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Config validation error: {}", msg),
        }
    }
}

impl Source {
    pub fn resolve_credentials<'a>(&self, config: &'a Config) -> Option<&'a Credentials> {
        config.credentials.get(&self.credentials)
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        if !Path::new(path).exists() {
            return Err(ConfigError::NotFound(path.to_string()));
        }

        let contents =
            std::fs::read_to_string(path).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        let config: Config =
            serde_yaml::from_str(&contents).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config.validate()?;
        Ok(config.normalized())
    }

    fn normalized(mut self) -> Self {
        for source in &mut self.sources {
            for bucket in &mut source.buckets {
                bucket.path = normalize_path(&bucket.path);
            }
        }
        self
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.sources.is_empty() {
            return Err(ConfigError::ValidationError(
                "At least one source is required".to_string(),
            ));
        }

        for source in &self.sources {
            if source.name.is_empty() {
                return Err(ConfigError::ValidationError(
                    "Source name cannot be empty".to_string(),
                ));
            }
            if source.credentials.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': credentials cannot be empty",
                    source.name
                )));
            }
            if !self.credentials.contains_key(&source.credentials) {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': credentials '{}' not found",
                    source.name, source.credentials
                )));
            }
            if source.platform.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': platform cannot be empty",
                    source.name
                )));
            }
            if source.buckets.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': at least one bucket is required",
                    source.name
                )));
            }
            for bucket in &source.buckets {
                if bucket.name.is_empty() {
                    return Err(ConfigError::ValidationError(format!(
                        "Source '{}': bucket name cannot be empty",
                        source.name
                    )));
                }
            }
        }

        Ok(())
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    trimmed
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
