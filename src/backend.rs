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
    fn list_all_objects(
        &self,
        catalog: &Catalog,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError>;

    fn download_object(
        &self,
        key: &str,
        dest: &std::path::Path,
        offset: u64,
        total_bytes: u64,
        cancel: &Arc<AtomicBool>,
        progress: &Sender<DownloadProgress>,
    ) -> Result<(), BackendError>;
}

pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
}

fn send_log(log: &Sender<String>, msg: String) {
    let _ = log.send(msg);
}

pub fn create_backend(source: &Source) -> Result<Box<dyn SourceBackend>, BackendError> {
    match source.source_type {
        SourceType::S3Archive => Ok(Box::new(IABackend::new(source))),
    }
}

struct IABackend {
    item: String,
    access_key: String,
    secret_key: String,
}

impl IABackend {
    fn new(source: &Source) -> Self {
        IABackend {
            item: source.endpoint.clone(),
            access_key: source.access_key.clone(),
            secret_key: source.secret_key.clone(),
        }
    }

    fn make_runtime(&self) -> Result<tokio::runtime::Runtime, BackendError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))
    }

}

#[derive(Deserialize)]
struct IAMetadataResponse {
    result: Vec<IAFileEntry>,
}

#[derive(Deserialize)]
struct IAFileEntry {
    name: String,
    size: Option<String>,
}

impl SourceBackend for IABackend {
    fn list_all_objects(
        &self,
        catalog: &Catalog,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(BackendError::ListFailed("Cancelled".to_string()));
        }

        let url = format!("https://archive.org/metadata/{}/files", self.item);
        let prefix = format!("{}/", catalog.path);

        send_log(log, format!("Listing item='{}' prefix='{}'", self.item, prefix));

        let rt = self.make_runtime()?;
        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);

        let response = rt.block_on(async {
            let client = reqwest::Client::new();
            client.get(&url)
                .header("Authorization", &auth)
                .send()
                .await
                .map_err(|e| BackendError::ListFailed(e.to_string()))
        })?;

        let status = response.status().as_u16();
        if status != 200 {
            return Err(BackendError::ListFailed(
                format!("Metadata API returned status {}", status)
            ));
        }

        if cancel.load(Ordering::Relaxed) {
            return Err(BackendError::ListFailed("Cancelled".to_string()));
        }

        let metadata: IAMetadataResponse = rt.block_on(async {
            response.json().await
                .map_err(|e| BackendError::ListFailed(e.to_string()))
        })?;

        let games: Vec<RemoteGame> = metadata.result.iter()
            .filter(|f| f.name.starts_with(&prefix))
            .filter_map(|f| {
                let size = f.size.as_ref()?.parse::<u64>().ok()?;
                Some(RemoteGame {
                    key: f.name.clone(),
                    file_size: size,
                })
            })
            .collect();

        send_log(log, format!("Listed {} objects", games.len()));
        Ok(games)
    }

    fn download_object(
        &self,
        key: &str,
        dest: &std::path::Path,
        offset: u64,
        total_bytes: u64,
        cancel: &Arc<AtomicBool>,
        progress: &Sender<DownloadProgress>,
    ) -> Result<(), BackendError> {
        eprintln!("[IA] download_object: key='{}' dest='{}' total_bytes={}", key, dest.display(), total_bytes);

        if total_bytes == 0 {
            return Err(BackendError::DownloadFailed(
                format!("File size is 0 for key '{}'", key)
            ));
        }

        if offset >= total_bytes {
            eprintln!("[IA] Already complete (offset {} >= total {})", offset, total_bytes);
            let _ = progress.send(DownloadProgress { bytes_downloaded: total_bytes, total_bytes });
            return Ok(());
        }

        // Percent-encode each path segment (preserve '/' separators)
        let encoded_key: String = key.split('/')
            .map(|seg| urlencoding::encode(seg))
            .collect::<Vec<_>>()
            .join("/");
        let url = format!("https://archive.org/download/{}/{}", self.item, encoded_key);
        eprintln!("[IA] Download URL: {}", url);

        let rt = self.make_runtime()?;

        // curl --location-trusted -H "Authorization: LOW key:secret" -H "Range: bytes=N-"
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);

        let mut current_url = url.clone();
        let response = rt.block_on(async {
            for hop in 0..10 {
                let mut req = client.get(&current_url)
                    .header("Authorization", &auth);
                if offset > 0 {
                    req = req.header("Range", format!("bytes={}-", offset));
                }

                let resp = req.send().await
                    .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

                let status = resp.status().as_u16();
                eprintln!("[IA] Hop {} status={} url={}", hop, status, current_url);

                if status == 301 || status == 302 || status == 307 || status == 308 {
                    if let Some(loc) = resp.headers().get("location") {
                        current_url = loc.to_str()
                            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?
                            .to_string();
                        continue;
                    }
                    return Err(BackendError::DownloadFailed(
                        format!("Redirect {} without Location header", status)
                    ));
                }

                return Ok(resp);
            }
            Err(BackendError::DownloadFailed("Too many redirects".to_string()))
        })?;

        let status = response.status().as_u16();
        eprintln!("[IA] Final status: {}", status);

        if status != 200 && status != 206 {
            return Err(BackendError::DownloadFailed(
                format!("Download returned status {} for '{}'", status, key)
            ));
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;
        }

        let mut downloaded = offset;
        let mut file = if offset > 0 {
            eprintln!("[IA] Resuming from byte {}", offset);
            std::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(dest)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?
        } else {
            std::fs::File::create(dest)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?
        };

        use std::io::Write;
        use tokio_stream::StreamExt;

        eprintln!("[IA] Streaming body ({} bytes remaining)", total_bytes - offset);
        let mut stream = response.bytes_stream();

        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(BackendError::DownloadFailed("Cancelled".to_string()));
            }
            let chunk = rt.block_on(async { stream.next().await });
            match chunk {
                Some(Ok(bytes)) => {
                    file.write_all(&bytes)
                        .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;
                    downloaded += bytes.len() as u64;
                    let _ = progress.send(DownloadProgress { bytes_downloaded: downloaded, total_bytes });
                }
                Some(Err(e)) => return Err(BackendError::DownloadFailed(e.to_string())),
                None => break,
            }
        }

        file.sync_all()
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        eprintln!("[IA] Download complete: {} bytes", downloaded);
        Ok(())
    }
}
