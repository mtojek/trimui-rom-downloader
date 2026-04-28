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

    fn head_object(&self, key: &str) -> Result<u64, BackendError>;
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
        eprintln!("[IA] download_object: key='{}' dest='{}' offset={} total_bytes={}", key, dest.display(), offset, total_bytes);

        if total_bytes == 0 {
            return Err(BackendError::DownloadFailed(
                format!("File size is 0 for key '{}'", key)
            ));
        }

        // Already complete
        if offset >= total_bytes {
            eprintln!("[IA] File already complete ({} >= {}), skipping", offset, total_bytes);
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

        // curl --location-trusted -H "Authorization: LOW key:secret"
        // Retry on 500/502/503 up to 5 times with 5s delay
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);
        let use_range = offset > 0;

        let mut last_status = 0u16;
        let mut response = None;

        for attempt in 0..5 {
            if cancel.load(Ordering::Relaxed) {
                return Err(BackendError::DownloadFailed("Cancelled".to_string()));
            }
            if attempt > 0 {
                eprintln!("[IA] Retry {}/5 after {}s (last status={})", attempt + 1, 5, last_status);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }

            let mut current_url = url.clone();
            let resp = rt.block_on(async {
                for hop in 0..10 {
                    let mut req = client.get(&current_url)
                        .header("Authorization", &auth);
                    if use_range {
                        req = req.header("Range", format!("bytes={}-", offset));
                    }
                    let resp = req.send()
                        .await
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

            last_status = resp.status().as_u16();
            eprintln!("[IA] Attempt {} final status: {}", attempt + 1, last_status);

            if last_status == 200 || last_status == 206 {
                response = Some(resp);
                break;
            }

            // Retry on server errors
            if last_status == 500 || last_status == 502 || last_status == 503 {
                continue;
            }

            // Non-retryable error
            return Err(BackendError::DownloadFailed(
                format!("Download returned status {} for '{}'", last_status, key)
            ));
        }

        let response = response.ok_or_else(|| {
            BackendError::DownloadFailed(
                format!("Download failed after 5 retries (last status={}) for '{}'", last_status, key)
            )
        })?;

        // If we requested Range but got 200 (not 206), server doesn't support resume — start from scratch
        let actual_offset = if use_range && last_status == 200 {
            eprintln!("[IA] Server returned 200 instead of 206, restarting from scratch");
            0u64
        } else if last_status == 206 {
            eprintln!("[IA] Resuming from offset {}", offset);
            offset
        } else {
            0u64
        };

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;
        }

        // Open file: append if resuming, create if starting fresh
        let mut file = if actual_offset > 0 {
            use std::fs::OpenOptions;
            OpenOptions::new().append(true).open(dest)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?
        } else {
            std::fs::File::create(dest)
                .map_err(|e| BackendError::DownloadFailed(e.to_string()))?
        };

        let mut downloaded: u64 = actual_offset;

        use std::io::Write;
        use tokio_stream::StreamExt;

        eprintln!("[IA] Streaming body ({} bytes remaining)", total_bytes - actual_offset);
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

        eprintln!("[IA] Download complete: {} bytes total", downloaded);
        Ok(())
    }

    fn head_object(&self, key: &str) -> Result<u64, BackendError> {
        let encoded_key: String = key.split('/')
            .map(|seg| urlencoding::encode(seg))
            .collect::<Vec<_>>()
            .join("/");
        let url = format!("https://archive.org/download/{}/{}", self.item, encoded_key);
        eprintln!("[IA] HEAD {}", url);

        let rt = self.make_runtime()?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))?;
        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);

        rt.block_on(async {
            let mut current_url = url;
            for _hop in 0..10 {
                let resp = client.head(&current_url)
                    .header("Authorization", &auth)
                    .send()
                    .await
                    .map_err(|e| BackendError::ListFailed(e.to_string()))?;

                let status = resp.status().as_u16();
                if status == 301 || status == 302 || status == 307 || status == 308 {
                    if let Some(loc) = resp.headers().get("location") {
                        current_url = loc.to_str()
                            .map_err(|e| BackendError::ListFailed(e.to_string()))?
                            .to_string();
                        continue;
                    }
                    return Err(BackendError::ListFailed("Redirect without Location".to_string()));
                }

                if status == 200 {
                    let len = resp.headers().get("content-length")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(0);
                    eprintln!("[IA] HEAD result: {} bytes", len);
                    return Ok(len);
                }

                return Err(BackendError::ListFailed(format!("HEAD returned status {}", status)));
            }
            Err(BackendError::ListFailed("Too many redirects".to_string()))
        })
    }
}
