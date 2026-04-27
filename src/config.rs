use serde::Deserialize;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub catalogs: Vec<Catalog>,
    #[serde(default)]
    pub options: SourceOptions,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SourceOptions {
    #[serde(default)]
    pub resolve_redirect: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    S3,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    pub path: String,
    pub platform: String,
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
        Ok(config)
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
            if source.endpoint.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': endpoint cannot be empty",
                    source.name
                )));
            }
            if source.access_key.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': access_key cannot be empty",
                    source.name
                )));
            }
            if source.secret_key.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': secret_key cannot be empty",
                    source.name
                )));
            }
            if source.catalogs.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Source '{}': at least one catalog is required",
                    source.name
                )));
            }
            for catalog in &source.catalogs {
                if catalog.path.is_empty() {
                    return Err(ConfigError::ValidationError(format!(
                        "Source '{}': catalog path cannot be empty",
                        source.name
                    )));
                }
                if catalog.platform.is_empty() {
                    return Err(ConfigError::ValidationError(format!(
                        "Source '{}': catalog platform cannot be empty",
                        source.name
                    )));
                }
            }
        }

        Ok(())
    }

}
