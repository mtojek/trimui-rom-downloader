use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::config::{Catalog, Source, SourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGame {
    pub key: String,
    pub file_size: u64,
}

#[derive(Debug)]
pub enum BackendError {
    ListFailed(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::ListFailed(msg) => write!(f, "List failed: {}", msg),
        }
    }
}

pub trait SourceBackend: Send + Sync {
    fn list_all_objects(
        &self,
        catalog: &Catalog,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError>;
}

fn send_log(log: &Sender<String>, msg: String) {
    let _ = log.send(msg);
}

pub fn create_backend(source: &Source) -> Result<Box<dyn SourceBackend>, BackendError> {
    match source.source_type {
        SourceType::S3 => Ok(Box::new(S3Backend::new(source)?)),
    }
}

struct S3Backend {
    endpoint: String,
    bucket_name: String,
    access_key: String,
    secret_key: String,
    resolve_redirect: bool,
}

impl S3Backend {
    fn new(source: &Source) -> Result<Self, BackendError> {
        let (endpoint, bucket_name) = match source.endpoint.rfind('/') {
            Some(pos) if pos > 8 => (
                source.endpoint[..pos].to_string(),
                source.endpoint[pos + 1..].to_string(),
            ),
            _ => {
                return Err(BackendError::ListFailed(format!(
                    "Invalid endpoint '{}', expected https://host/bucket",
                    source.endpoint
                )));
            }
        };

        Ok(S3Backend {
            endpoint,
            bucket_name,
            access_key: source.access_key.clone(),
            secret_key: source.secret_key.clone(),
            resolve_redirect: source.options.resolve_redirect,
        })
    }

    fn credentials(&self) -> Result<s3::creds::Credentials, BackendError> {
        s3::creds::Credentials::new(
            Some(&self.access_key),
            Some(&self.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| BackendError::ListFailed(e.to_string()))
    }

    fn bucket_with_endpoint(&self, endpoint: &str) -> Result<s3::Bucket, BackendError> {
        let region = s3::Region::Custom {
            region: "us-east-1".to_string(),
            endpoint: endpoint.to_string(),
        };
        let credentials = self.credentials()?;
        let bucket = s3::Bucket::new(&self.bucket_name, region, credentials)
            .map_err(|e| BackendError::ListFailed(e.to_string()))?
            .with_path_style();
        Ok(*bucket)
    }

    fn bucket(&self) -> Result<s3::Bucket, BackendError> {
        self.bucket_with_endpoint(&self.endpoint)
    }

    fn resolve_endpoint(&self, log: &Sender<String>) -> Result<String, BackendError> {
        let bucket = self.bucket()?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))?;

        // Use get_object to probe — returns raw ResponseData even on error/redirect
        let response = rt.block_on(async {
            bucket
                .get_object("/")
                .await
                .map_err(|e| BackendError::ListFailed(e.to_string()))
        })?;

        let status = response.status_code();
        send_log(log, format!("Redirect probe status: {}", status));

        if status == 307 || status == 301 || status == 302 {
            let body = String::from_utf8_lossy(response.bytes());

            if let Some(start) = body.find("<Endpoint>") {
                if let Some(end) = body.find("</Endpoint>") {
                    let host = &body[start + 10..end];
                    let resolved = format!("http://{}", host);
                    send_log(log, format!("Resolved endpoint: {}", resolved));
                    return Ok(resolved);
                }
            }
        }

        send_log(log, "No redirect needed".to_string());
        Ok(self.endpoint.clone())
    }
}

impl SourceBackend for S3Backend {
    fn list_all_objects(
        &self,
        catalog: &Catalog,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(BackendError::ListFailed("Cancelled".to_string()));
        }

        let bucket = if self.resolve_redirect {
            send_log(log, format!("Resolving redirect for {}", self.endpoint));
            let resolved = self.resolve_endpoint(log)?;
            if resolved != self.endpoint {
                send_log(log, format!("Using resolved endpoint: {}", resolved));
                self.bucket_with_endpoint(&resolved)?
            } else {
                self.bucket()?
            }
        } else {
            self.bucket()?
        };
        let prefix = format!("{}/", catalog.path);

        send_log(log, format!("Listing bucket='{}' prefix='{}'", self.bucket_name, prefix));

        if cancel.load(Ordering::Relaxed) {
            return Err(BackendError::ListFailed("Cancelled".to_string()));
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))?;

        let results = rt.block_on(async {
            bucket
                .list(prefix, None)
                .await
                .map_err(|e| BackendError::ListFailed(e.to_string()))
        })?;

        let mut games: Vec<RemoteGame> = Vec::new();
        for result in &results {
            for object in &result.contents {
                games.push(RemoteGame {
                    key: object.key.clone(),
                    file_size: object.size,
                });
            }
        }

        send_log(log, format!("Listed {} objects", games.len()));
        Ok(games)
    }

}
