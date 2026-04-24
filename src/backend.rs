use serde::{Deserialize, Serialize};
use std::fmt;

use crate::config::{Catalog, Source, SourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGame {
    pub title: String,
    pub key: String,
    pub file_size: Option<u64>,
}

#[derive(Debug)]
pub enum BackendError {
    ListFailed(String),
    DownloadFailed(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::ListFailed(msg) => write!(f, "List failed: {}", msg),
            BackendError::DownloadFailed(msg) => write!(f, "Download failed: {}", msg),
        }
    }
}

pub trait SourceBackend: Send + Sync {
    fn list_objects(
        &self,
        catalog: &Catalog,
        letter: char,
    ) -> Result<Vec<RemoteGame>, BackendError>;

    fn download_object(
        &self,
        catalog: &Catalog,
        key: &str,
        dest: &str,
    ) -> Result<(), BackendError>;
}

pub fn create_backend(source: &Source) -> Result<Box<dyn SourceBackend>, BackendError> {
    match source.source_type {
        SourceType::S3 => Ok(Box::new(S3Backend::new(source))),
    }
}

struct S3Backend {
    endpoint: String,
    access_key: String,
    secret_key: String,
}

impl S3Backend {
    fn new(source: &Source) -> Self {
        S3Backend {
            endpoint: source.endpoint.clone(),
            access_key: source.access_key.clone(),
            secret_key: source.secret_key.clone(),
        }
    }

    fn bucket(&self, catalog: &Catalog) -> Result<s3::Bucket, BackendError> {
        let region = s3::Region::Custom {
            region: "us-east-1".to_string(),
            endpoint: self.endpoint.clone(),
        };
        let credentials = s3::creds::Credentials::new(
            Some(&self.access_key),
            Some(&self.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| BackendError::ListFailed(e.to_string()))?;

        let bucket = s3::Bucket::new(&catalog.path, region, credentials)
            .map_err(|e| BackendError::ListFailed(e.to_string()))?
            .with_path_style();

        Ok(*bucket)
    }
}

impl SourceBackend for S3Backend {
    fn list_objects(
        &self,
        catalog: &Catalog,
        letter: char,
    ) -> Result<Vec<RemoteGame>, BackendError> {
        let bucket = self.bucket(catalog)?;

        let prefix = if letter == '#' {
            String::new()
        } else {
            format!("{}", letter)
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))?;

        let results = rt.block_on(async {
            bucket
                .list(prefix.clone(), None)
                .await
                .map_err(|e| BackendError::ListFailed(e.to_string()))
        })?;

        let mut games: Vec<RemoteGame> = Vec::new();
        for result in &results {
            for object in &result.contents {
                let key = &object.key;
                let title = key
                    .rsplit('/')
                    .next()
                    .unwrap_or(key)
                    .to_string();

                if letter == '#' {
                    if let Some(first) = title.chars().next() {
                        if first.is_ascii_alphabetic() {
                            continue;
                        }
                    }
                }

                games.push(RemoteGame {
                    title,
                    key: key.clone(),
                    file_size: Some(object.size),
                });
            }
        }

        Ok(games)
    }

    fn download_object(
        &self,
        catalog: &Catalog,
        key: &str,
        dest: &str,
    ) -> Result<(), BackendError> {
        let bucket = self.bucket(catalog)?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        let response = rt.block_on(async {
            bucket
                .get_object(key)
                .await
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))
        })?;

        std::fs::write(dest, response.bytes())
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        Ok(())
    }
}
