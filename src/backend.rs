use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::config::{Bucket, Config, Source, SourceType};

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
    fn list_bucket(
        &self,
        bucket: &Bucket,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError>;

    fn download_object(
        &self,
        bucket_name: &str,
        key: &str,
        dest: &std::path::Path,
        offset: u64,
        total_bytes: u64,
        cancel: &Arc<AtomicBool>,
        progress: &Sender<DownloadProgress>,
    ) -> Result<(), BackendError>;

    fn head_object(&self, bucket_name: &str, key: &str) -> Result<u64, BackendError>;
}

pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
}

fn send_log(log: &Sender<String>, msg: String) {
    let _ = log.send(msg);
}

pub fn create_backend(source: &Source, config: &Config) -> Result<Box<dyn SourceBackend>, BackendError> {
    let creds = source.resolve_credentials(config).ok_or_else(|| {
        BackendError::ListFailed(format!("Credentials '{}' not found", source.credentials))
    })?;
    match source.source_type {
        SourceType::S3Archive => Ok(Box::new(IABackend::new(creds.access_key.clone(), creds.secret_key.clone()))),
    }
}

struct IABackend {
    access_key: String,
    secret_key: String,
}

impl IABackend {
    fn new(access_key: String, secret_key: String) -> Self {
        IABackend { access_key, secret_key }
    }

    fn make_runtime(&self) -> Result<tokio::runtime::Runtime, BackendError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))
    }

    fn make_download_url(&self, bucket_name: &str, key: &str) -> String {
        let encoded_key: String = key.split('/')
            .map(|seg| urlencoding::encode(seg))
            .collect::<Vec<_>>()
            .join("/");
        format!("https://archive.org/download/{}/{}", bucket_name, encoded_key)
    }

    fn follow_redirects(
        &self,
        rt: &tokio::runtime::Runtime,
        client: &reqwest::Client,
        method: &str,
        url: &str,
        auth: &str,
        range: Option<&str>,
    ) -> Result<reqwest::Response, BackendError> {
        rt.block_on(async {
            let mut current_url = url.to_string();
            for _hop in 0..10 {
                let mut req = match method {
                    "HEAD" => client.head(&current_url),
                    _ => client.get(&current_url),
                };
                req = req.header("Authorization", auth);
                if let Some(r) = range {
                    req = req.header("Range", r);
                }
                let resp = req.send().await
                    .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

                let status = resp.status().as_u16();
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
        })
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
    fn list_bucket(
        &self,
        bucket: &Bucket,
        log: &Sender<String>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<RemoteGame>, BackendError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(BackendError::ListFailed("Cancelled".to_string()));
        }

        let url = format!("https://archive.org/metadata/{}/files", bucket.name);
        let prefix = if bucket.path.is_empty() {
            String::new()
        } else {
            format!("{}/", bucket.path)
        };

        send_log(log, format!("Listing bucket='{}' prefix='{}'", bucket.name, prefix));

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
            .filter(|f| prefix.is_empty() || f.name.starts_with(&prefix))
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
        bucket_name: &str,
        key: &str,
        dest: &std::path::Path,
        offset: u64,
        total_bytes: u64,
        cancel: &Arc<AtomicBool>,
        progress: &Sender<DownloadProgress>,
    ) -> Result<(), BackendError> {
        eprintln!("[IA] download_object: bucket='{}' key='{}' dest='{}' offset={} total_bytes={}", bucket_name, key, dest.display(), offset, total_bytes);

        if total_bytes == 0 {
            return Err(BackendError::DownloadFailed(
                format!("File size is 0 for key '{}'", key)
            ));
        }

        if offset >= total_bytes {
            eprintln!("[IA] File already complete ({} >= {}), skipping", offset, total_bytes);
            let _ = progress.send(DownloadProgress { bytes_downloaded: total_bytes, total_bytes });
            return Ok(());
        }

        let url = self.make_download_url(bucket_name, key);
        eprintln!("[IA] Download URL: {}", url);

        let rt = self.make_runtime()?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BackendError::DownloadFailed(e.to_string()))?;

        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);
        let range_header = if offset > 0 {
            Some(format!("bytes={}-", offset))
        } else {
            None
        };

        let mut last_status = 0u16;
        let mut response = None;

        for attempt in 0..5 {
            if cancel.load(Ordering::Relaxed) {
                return Err(BackendError::DownloadFailed("Cancelled".to_string()));
            }
            if attempt > 0 {
                eprintln!("[IA] Retry {}/5 after 5s (last status={})", attempt + 1, last_status);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }

            let resp = self.follow_redirects(&rt, &client, "GET", &url, &auth, range_header.as_deref())?;
            last_status = resp.status().as_u16();
            eprintln!("[IA] Attempt {} final status: {}", attempt + 1, last_status);

            if last_status == 200 || last_status == 206 {
                response = Some(resp);
                break;
            }

            if last_status == 500 || last_status == 502 || last_status == 503 {
                continue;
            }

            return Err(BackendError::DownloadFailed(
                format!("Download returned status {} for '{}'", last_status, key)
            ));
        }

        let response = response.ok_or_else(|| {
            BackendError::DownloadFailed(
                format!("Download failed after 5 retries (last status={}) for '{}'", last_status, key)
            )
        })?;

        let actual_offset = if offset > 0 && last_status == 200 {
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

    fn head_object(&self, bucket_name: &str, key: &str) -> Result<u64, BackendError> {
        let url = self.make_download_url(bucket_name, key);
        eprintln!("[IA] HEAD {}", url);

        let rt = self.make_runtime()?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| BackendError::ListFailed(e.to_string()))?;
        let auth = format!("LOW {}:{}", self.access_key, self.secret_key);

        let resp = self.follow_redirects(&rt, &client, "HEAD", &url, &auth, None)?;
        let status = resp.status().as_u16();

        if status == 200 {
            let len = resp.headers().get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            eprintln!("[IA] HEAD result: {} bytes", len);
            Ok(len)
        } else {
            Err(BackendError::ListFailed(format!("HEAD returned status {}", status)))
        }
    }
}
