use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::backend::{self, BackendError, DownloadProgress};
use crate::config::{Config, Source};
use crate::install_dir::InstallDirResolver;

const DATA_DIR_NAME: &str = ".rom-downloader";
const QUEUE_FILE: &str = "downloads.yaml";
const MAX_ACTIVE: usize = 2;

pub type DownloadId = u64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadState {
    Queued,
    Active,
    Paused,
    Completed,
    Failed,
}

impl std::fmt::Display for DownloadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadState::Queued => write!(f, "Queued"),
            DownloadState::Active => write!(f, "Active"),
            DownloadState::Paused => write!(f, "Paused"),
            DownloadState::Completed => write!(f, "Completed"),
            DownloadState::Failed => write!(f, "Failed"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadEntry {
    pub id: DownloadId,
    pub state: DownloadState,
    pub source_name: String,
    pub platform: String,
    pub key: String,
    pub bucket_name: String,
    pub file_name: String,
    pub game_key: String,
    pub dest_path: PathBuf,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Completed { id: DownloadId },
    Failed { id: DownloadId, error: String },
}

pub enum DownloadCommand {
    Enqueue {
        source: Source,
        platform: String,
        key: String,
        bucket_name: String,
        file_name: String,
        game_key: String,
        dest_path: PathBuf,
        total_bytes: u64,
    },
    Pause(DownloadId),
    Resume(DownloadId),
    Cancel(DownloadId),
}

struct ActiveDownload {
    id: DownloadId,
    cancel: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<Result<(), BackendError>>,
}

pub struct DownloadManager {
    queue: Arc<Mutex<DownloadQueue>>,
    cmd_tx: Sender<DownloadCommand>,
    event_rx: Receiver<DownloadEvent>,
    _worker: std::thread::JoinHandle<()>,
}

struct DownloadQueue {
    entries: VecDeque<DownloadEntry>,
    next_id: DownloadId,
}

impl DownloadQueue {
    fn new() -> Self {
        DownloadQueue {
            entries: VecDeque::new(),
            next_id: 1,
        }
    }

    fn find(&self, id: DownloadId) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    fn find_mut(&mut self, id: DownloadId) -> Option<&mut DownloadEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    fn next_queued_id(&self) -> Option<DownloadId> {
        self.entries
            .iter()
            .find(|e| e.state == DownloadState::Queued)
            .map(|e| e.id)
    }

    fn snapshot(&self) -> Vec<DownloadEntry> {
        self.entries.iter().cloned().collect()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedState {
    Downloading,
    Paused,
    Failed,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEntry {
    source_name: String,
    platform: String,
    key: String,
    #[serde(default)]
    bucket_name: String,
    state: PersistedState,
}

fn derive_file_name(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_string()
}

fn derive_game_key(file_name: &str) -> String {
    match file_name.rsplit_once('.') {
        Some((name, _)) => name.to_string(),
        None => file_name.to_string(),
    }
}

fn derive_dest_path(
    install_resolver: &InstallDirResolver,
    platform: &str,
    game_key: &str,
    file_name: &str,
) -> PathBuf {
    let game_dir = install_resolver
        .game_dir(platform, game_key)
        .unwrap_or_else(|| {
            PathBuf::from("/mnt/SDCARD/Roms")
                .join(platform)
                .join(game_key)
        });
    game_dir.join(file_name)
}

/// Find the bucket name for a given key by matching bucket paths in the source.
fn find_bucket_name(source: &Source, key: &str) -> String {
    for bucket in &source.buckets {
        if bucket.path.is_empty() {
            return bucket.name.clone();
        }
        let prefix = format!("{}/", bucket.path);
        if key.starts_with(&prefix) {
            return bucket.name.clone();
        }
    }
    // Fallback: first bucket
    source.buckets.first().map(|b| b.name.clone()).unwrap_or_default()
}

impl DownloadManager {
    pub fn new(config: Config, install_resolver: &InstallDirResolver) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<DownloadCommand>();
        let (event_tx, event_rx) = std::sync::mpsc::channel::<DownloadEvent>();

        let queue = Arc::new(Mutex::new(DownloadQueue::new()));

        // Load persisted queue
        {
            let mut q = queue.lock().unwrap();
            let path = queue_path();
            match load_queue() {
                Ok(persisted) if !persisted.is_empty() => {
                    eprintln!("[DL] Loaded {} entries from {}", persisted.len(), path.display());
                    for pe in persisted {
                        let (state, state_label) = match pe.state {
                            PersistedState::Downloading => (DownloadState::Queued, "Queued (was downloading)"),
                            PersistedState::Paused => (DownloadState::Paused, "Paused"),
                            PersistedState::Failed => (DownloadState::Failed, "Failed"),
                            PersistedState::Done => (DownloadState::Completed, "Done"),
                        };
                        let file_name = derive_file_name(&pe.key);
                        let game_key = derive_game_key(&file_name);
                        let dest_path = derive_dest_path(install_resolver, &pe.platform, &game_key, &file_name);
                        let downloaded_bytes = if dest_path.exists() {
                            std::fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0)
                        } else {
                            0
                        };
                        let id = q.next_id;
                        q.next_id += 1;
                        eprintln!("[DL] #{} '{}': restored as {} ({}B on disk)", id, file_name, state_label, downloaded_bytes);
                        q.entries.push_back(DownloadEntry {
                            id,
                            state,
                            source_name: pe.source_name,
                            platform: pe.platform,
                            key: pe.key,
                            bucket_name: pe.bucket_name,
                            file_name,
                            game_key,
                            dest_path,
                            total_bytes: 0,
                            downloaded_bytes,
                            speed: 0.0,
                            error: None,
                        });
                    }
                }
                Ok(_) => {
                    eprintln!("[DL] No pending downloads in queue");
                }
                Err(e) => {
                    eprintln!("[DL] Failed to load queue: {}", e);
                }
            }
        }

        let queue_clone = queue.clone();
        let worker = std::thread::spawn(move || {
            eprintln!("[DL] Worker thread started (max {} concurrent)", MAX_ACTIVE);
            worker_loop(queue_clone, cmd_rx, event_tx, config);
            eprintln!("[DL] Worker thread stopped");
        });

        DownloadManager {
            queue,
            cmd_tx,
            event_rx,
            _worker: worker,
        }
    }

    pub fn send_command(&self, cmd: DownloadCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn poll_events(&self) -> Vec<DownloadEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn statuses(&self) -> Vec<DownloadEntry> {
        self.queue.lock().unwrap().snapshot()
    }

    pub fn is_queued_or_active(&self, source_name: &str, platform: &str, game_key: &str) -> bool {
        let q = self.queue.lock().unwrap();
        q.entries.iter().any(|e| {
            e.source_name == source_name
                && e.platform == platform
                && e.game_key == game_key
                && matches!(
                    e.state,
                    DownloadState::Queued | DownloadState::Active | DownloadState::Paused
                )
        })
    }

    pub fn is_failed(&self, source_name: &str, platform: &str, game_key: &str) -> bool {
        let q = self.queue.lock().unwrap();
        q.entries.iter().any(|e| {
            e.source_name == source_name
                && e.platform == platform
                && e.game_key == game_key
                && e.state == DownloadState::Failed
        })
    }
}

fn worker_loop(
    queue: Arc<Mutex<DownloadQueue>>,
    cmd_rx: Receiver<DownloadCommand>,
    event_tx: Sender<DownloadEvent>,
    config: Config,
) {
    let mut active_downloads: Vec<ActiveDownload> = Vec::new();

    // Resolve total_bytes via HEAD for loaded entries
    {
        let entries_to_resolve: Vec<(DownloadId, String, String, String)> = {
            let q = queue.lock().unwrap();
            q.entries.iter()
                .filter(|e| e.total_bytes == 0 && e.state != DownloadState::Completed)
                .map(|e| (e.id, e.source_name.clone(), e.key.clone(), e.bucket_name.clone()))
                .collect()
        };
        for (id, source_name, key, bucket_name) in entries_to_resolve {
            let source = config.sources.iter().find(|s| s.name == source_name);
            if let Some(source) = source {
                match backend::create_backend(source, &config) {
                    Ok(be) => {
                        let bucket_name = if bucket_name.is_empty() {
                            find_bucket_name(source, &key)
                        } else {
                            bucket_name.clone()
                        };
                        match be.head_object(&bucket_name, &key) {
                            Ok(size) => {
                                let mut q = queue.lock().unwrap();
                                if let Some(entry) = q.find_mut(id) {
                                    entry.total_bytes = size;
                                    eprintln!("[DL] #{} '{}': HEAD resolved total_bytes={}", id, entry.file_name, size);
                                }
                            }
                            Err(e) => eprintln!("[DL] #{}: HEAD failed for '{}': {}", id, key, e),
                        }
                    }
                    Err(e) => eprintln!("[DL] #{}: Failed to create backend: {}", id, e),
                }
            } else {
                eprintln!("[DL] #{}: Source '{}' not found in config, skipping HEAD", id, source_name);
            }
        }
    }

    loop {
        // Process commands (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                DownloadCommand::Enqueue {
                    source,
                    platform,
                    key,
                    bucket_name,
                    file_name,
                    game_key,
                    dest_path,
                    total_bytes,
                } => {
                    let mut q = queue.lock().unwrap();
                    let duplicate = q.entries.iter().any(|e| {
                        e.source_name == source.name
                            && e.platform == platform
                            && e.game_key == game_key
                            && matches!(
                                e.state,
                                DownloadState::Queued
                                    | DownloadState::Active
                                    | DownloadState::Paused
                            )
                    });
                    if duplicate {
                        eprintln!("[DL] Skipping duplicate: '{}' already in queue", file_name);
                        continue;
                    }
                    let id = q.next_id;
                    q.next_id += 1;
                    eprintln!(
                        "[DL] #{} Enqueued: '{}' ({}) source='{}' bucket='{}' platform='{}' dest='{}'",
                        id, file_name, format_bytes(total_bytes), source.name, bucket_name, platform, dest_path.display()
                    );
                    q.entries.push_back(DownloadEntry {
                        id,
                        state: DownloadState::Queued,
                        source_name: source.name.clone(),
                        platform,
                        key,
                        bucket_name,
                        file_name,
                        game_key,
                        dest_path,
                        total_bytes,
                        downloaded_bytes: 0,
                        speed: 0.0,
                        error: None,
                    });
                    save_queue_locked(&q);
                }
                DownloadCommand::Pause(id) => {
                    eprintln!("[DL] #{} Pause requested", id);
                    if let Some(active) = active_downloads.iter().find(|a| a.id == id) {
                        active.cancel.store(true, Ordering::Relaxed);
                    }
                    let mut q = queue.lock().unwrap();
                    if let Some(entry) = q.find_mut(id) {
                        if entry.state == DownloadState::Active {
                            entry.state = DownloadState::Paused;
                            eprintln!("[DL] #{} '{}': Paused at {}", id, entry.file_name, format_bytes(entry.downloaded_bytes));
                            save_queue_locked(&q);
                        }
                    }
                }
                DownloadCommand::Resume(id) => {
                    eprintln!("[DL] #{} Resume requested", id);
                    let mut q = queue.lock().unwrap();
                    if let Some(entry) = q.find_mut(id) {
                        if entry.state == DownloadState::Paused || entry.state == DownloadState::Failed {
                            eprintln!("[DL] #{} '{}': {} -> re-queued", id, entry.file_name, entry.state);
                            entry.state = DownloadState::Queued;
                            entry.error = None;
                            save_queue_locked(&q);
                        }
                    }
                }
                DownloadCommand::Cancel(id) => {
                    eprintln!("[DL] #{} Cancel requested", id);
                    if let Some(active) = active_downloads.iter().find(|a| a.id == id) {
                        active.cancel.store(true, Ordering::Relaxed);
                    }
                    let mut q = queue.lock().unwrap();
                    if let Some(pos) = q.find(id) {
                        let entry = &q.entries[pos];
                        let path = entry.dest_path.clone();
                        let file_name = entry.file_name.clone();
                        q.entries.remove(pos);
                        save_queue_locked(&q);
                        if path.exists() {
                            eprintln!("[DL] #{} '{}': Deleting partial file {}", id, file_name, path.display());
                            let _ = std::fs::remove_file(&path);
                        }
                        if let Some(parent) = path.parent() {
                            if parent.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false) {
                                eprintln!("[DL] #{} '{}': Removing empty directory {}", id, file_name, parent.display());
                                let _ = std::fs::remove_dir(parent);
                            }
                        }
                        eprintln!("[DL] #{} '{}': Cancelled and removed from queue", id, file_name);
                    }
                }
            }
        }

        // Reap finished downloads
        let mut i = 0;
        while i < active_downloads.len() {
            if active_downloads[i].handle.is_finished() {
                let ad = active_downloads.remove(i);
                let mut q = queue.lock().unwrap();
                match ad.handle.join() {
                    Ok(Ok(())) => {
                        if let Some(entry) = q.find_mut(ad.id) {
                            if entry.state == DownloadState::Active {
                                entry.state = DownloadState::Completed;
                                entry.downloaded_bytes = entry.total_bytes;
                                eprintln!(
                                    "[DL] #{} '{}': Completed! {} downloaded to {}",
                                    ad.id, entry.file_name, format_bytes(entry.total_bytes), entry.dest_path.display()
                                );
                                save_queue_locked(&q);
                                let _ = event_tx.send(DownloadEvent::Completed { id: ad.id });
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        if let Some(entry) = q.find_mut(ad.id) {
                            if entry.state != DownloadState::Paused {
                                let msg = e.to_string();
                                if !msg.contains("Cancelled") {
                                    entry.state = DownloadState::Failed;
                                    entry.error = Some(msg.clone());
                                    eprintln!("[DL] #{} '{}': Failed — {}", ad.id, entry.file_name, msg);
                                    save_queue_locked(&q);
                                    let _ = event_tx.send(DownloadEvent::Failed {
                                        id: ad.id,
                                        error: msg,
                                    });
                                } else {
                                    eprintln!("[DL] #{} '{}': Download thread cancelled (pause/cancel)", ad.id, entry.file_name);
                                }
                            } else {
                                eprintln!("[DL] #{} '{}': Download thread stopped (paused)", ad.id, entry.file_name);
                            }
                        }
                    }
                    Err(_) => {
                        if let Some(entry) = q.find_mut(ad.id) {
                            entry.state = DownloadState::Failed;
                            entry.error = Some("Thread panicked".to_string());
                            eprintln!("[DL] #{} '{}': Download thread PANICKED!", ad.id, entry.file_name);
                            save_queue_locked(&q);
                            let _ = event_tx.send(DownloadEvent::Failed {
                                id: ad.id,
                                error: "Thread panicked".to_string(),
                            });
                        }
                    }
                }
            } else {
                i += 1;
            }
        }

        // Start new downloads if slots available
        {
            let mut q = queue.lock().unwrap();
            while active_downloads.len() < MAX_ACTIVE {
                let next_id = match q.next_queued_id() {
                    Some(id) => id,
                    None => break,
                };

                let entry = q.find_mut(next_id).unwrap();
                entry.state = DownloadState::Active;

                let source_name = entry.source_name.clone();
                let key = entry.key.clone();
                let bucket_name = entry.bucket_name.clone();
                let file_name = entry.file_name.clone();
                let dest = entry.dest_path.clone();
                let total_bytes = entry.total_bytes;
                let id = entry.id;

                // Resume: use existing file size as offset
                let offset: u64 = if dest.exists() {
                    match std::fs::metadata(&dest) {
                        Ok(meta) => {
                            let size = meta.len();
                            eprintln!("[DL] #{} '{}': Resuming from {} / {}", id, file_name, format_bytes(size), format_bytes(total_bytes));
                            entry.downloaded_bytes = size;
                            size
                        }
                        Err(_) => {
                            entry.downloaded_bytes = 0;
                            0
                        }
                    }
                } else {
                    entry.downloaded_bytes = 0;
                    0
                };

                eprintln!(
                    "[DL] #{} '{}': Starting download ({}, offset={}) to {}",
                    id, file_name, format_bytes(total_bytes), format_bytes(offset), dest.display()
                );

                let source = config
                    .sources
                    .iter()
                    .find(|s| s.name == source_name)
                    .cloned();

                let config_clone = config.clone();

                save_queue_locked(&q);

                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_clone = cancel.clone();
                let queue_clone = queue.clone();

                let handle = std::thread::spawn(move || {
                    download_worker(
                        id,
                        source,
                        config_clone,
                        &bucket_name,
                        &key,
                        &file_name,
                        &dest,
                        offset,
                        total_bytes,
                        cancel_clone,
                        queue_clone,
                    )
                });

                active_downloads.push(ActiveDownload {
                    id,
                    cancel,
                    handle,
                });
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn download_worker(
    id: DownloadId,
    source: Option<Source>,
    config: Config,
    bucket_name: &str,
    key: &str,
    file_name: &str,
    dest: &std::path::Path,
    offset: u64,
    total_bytes: u64,
    cancel: Arc<AtomicBool>,
    queue: Arc<Mutex<DownloadQueue>>,
) -> Result<(), BackendError> {
    let source = source.ok_or_else(|| {
        BackendError::DownloadFailed("Source not found in config".to_string())
    })?;

    eprintln!("[DL] #{} '{}': Creating backend for source '{}', bucket '{}'", id, file_name, source.name, bucket_name);
    let be = backend::create_backend(&source, &config)?;

    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<DownloadProgress>();

    let file_name_owned = file_name.to_string();
    let queue2 = queue.clone();
    let progress_thread = std::thread::spawn(move || {
        let mut last_report = Instant::now();
        let mut last_bytes = offset;
        let mut last_log = Instant::now();
        let mut current_speed: f64 = 0.0;

        while let Ok(p) = progress_rx.recv() {
            let now = Instant::now();
            let elapsed = now.duration_since(last_report).as_secs_f64();

            if elapsed >= 0.5 {
                let delta = p.bytes_downloaded.saturating_sub(last_bytes) as f64;
                current_speed = delta / elapsed;
                last_report = now;
                last_bytes = p.bytes_downloaded;
            }

            if let Ok(mut q) = queue2.lock() {
                if let Some(entry) = q.find_mut(id) {
                    entry.downloaded_bytes = p.bytes_downloaded;
                    entry.speed = current_speed;
                }
            }

            if now.duration_since(last_log).as_secs() >= 5 {
                let pct = if p.total_bytes > 0 {
                    (p.bytes_downloaded as f64 / p.total_bytes as f64 * 100.0) as u32
                } else {
                    0
                };
                eprintln!(
                    "[DL] #{} '{}': {}% ({} / {}) @ {}/s",
                    id, file_name_owned, pct,
                    format_bytes(p.bytes_downloaded), format_bytes(p.total_bytes),
                    format_bytes(current_speed as u64)
                );
                last_log = now;
            }
        }
    });

    let result = be.download_object(&bucket_name, key, dest, offset, total_bytes, &cancel, &progress_tx);

    drop(progress_tx);
    let _ = progress_thread.join();

    match &result {
        Ok(()) => {
            eprintln!("[DL] #{} '{}': download_object completed successfully", id, file_name);
            if source.extract && file_name.to_lowercase().ends_with(".zip") {
                eprintln!("[DL] #{} '{}': Extracting zip archive...", id, file_name);
                if let Err(e) = extract_zip(dest) {
                    eprintln!("[DL] #{} '{}': Extraction failed: {}", id, file_name, e);
                    return Err(BackendError::DownloadFailed(format!("Extraction failed: {}", e)));
                }
                eprintln!("[DL] #{} '{}': Extraction complete, deleting archive", id, file_name);
                let _ = std::fs::remove_file(dest);
            }
        }
        Err(e) => eprintln!("[DL] #{} '{}': download_object error: {}", id, file_name, e),
    }

    result
}

fn extract_zip(archive_path: &std::path::Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let dest_dir = archive_path.parent().ok_or("No parent directory")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();

        if entry.is_dir() {
            continue;
        }

        // Flatten: extract only the file name, ignore any directory structure inside the zip
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        if file_name.is_empty() {
            continue;
        }

        let out_path = dest_dir.join(file_name);
        eprintln!("[DL] Extracting: {} -> {}", name, out_path.display());

        let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn queue_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join(DATA_DIR_NAME).join(QUEUE_FILE)
}

fn save_queue_locked(q: &DownloadQueue) {
    let path = queue_path();
    let persistable: Vec<PersistedEntry> = q
        .entries
        .iter()
        .filter_map(|e| {
            let state = match e.state {
                DownloadState::Queued | DownloadState::Active => PersistedState::Downloading,
                DownloadState::Paused => PersistedState::Paused,
                DownloadState::Failed => PersistedState::Failed,
                DownloadState::Completed => PersistedState::Done,
            };
            Some(PersistedEntry {
                source_name: e.source_name.clone(),
                platform: e.platform.clone(),
                key: e.key.clone(),
                bucket_name: e.bucket_name.clone(),
                state,
            })
        })
        .collect();
    let mut sorted = persistable;
    sorted.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml::to_string(&sorted) {
        let _ = std::fs::write(&path, yaml);
    }
}

fn load_queue() -> Result<Vec<PersistedEntry>, String> {
    let path = queue_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_yaml::from_str(&contents).map_err(|e| e.to_string())
}
