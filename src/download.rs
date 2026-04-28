use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::backend::{self, BackendError, DownloadProgress};
use crate::config::{Config, Source};

const DATA_DIR_NAME: &str = ".rom-downloader";
const QUEUE_FILE: &str = "downloads.yaml";
const MAX_ACTIVE: usize = 2;

pub type DownloadId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadEntry {
    pub id: DownloadId,
    pub state: DownloadState,
    pub source_name: String,
    pub platform: String,
    pub key: String,
    pub file_name: String,
    pub game_key: String,
    pub dest_path: PathBuf,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    #[serde(skip)]
    pub speed: f64,
    #[serde(skip)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    StateChanged { id: DownloadId, state: DownloadState },
    Progress { id: DownloadId, downloaded: u64, total: u64, speed: f64 },
    Completed { id: DownloadId },
    Failed { id: DownloadId, error: String },
}

pub enum DownloadCommand {
    Enqueue {
        source: Source,
        platform: String,
        key: String,
        file_name: String,
        game_key: String,
        dest_path: PathBuf,
        total_bytes: u64,
    },
    Pause(DownloadId),
    Resume(DownloadId),
    Cancel(DownloadId),
    Shutdown,
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

impl DownloadManager {
    pub fn new(config: Config) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<DownloadCommand>();
        let (event_tx, event_rx) = std::sync::mpsc::channel::<DownloadEvent>();

        let queue = Arc::new(Mutex::new(DownloadQueue::new()));

        // Load persisted queue
        {
            let mut q = queue.lock().unwrap();
            let path = queue_path();
            match load_queue() {
                Ok(entries) if !entries.is_empty() => {
                    eprintln!("[DL] Loaded {} entries from {}", entries.len(), path.display());
                    let max_id = entries.iter().map(|e| e.id).max().unwrap_or(0);
                    q.next_id = max_id + 1;
                    for mut entry in entries {
                        if entry.state == DownloadState::Active {
                            eprintln!("[DL] #{} '{}': was Active, resetting to Queued (app restarted)", entry.id, entry.file_name);
                            entry.state = DownloadState::Queued;
                        }
                        if entry.state == DownloadState::Queued || entry.state == DownloadState::Paused {
                            eprintln!("[DL] #{} '{}': restored as {}", entry.id, entry.file_name, entry.state);
                            q.entries.push_back(entry);
                        }
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

    loop {
        // Process commands (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                DownloadCommand::Enqueue {
                    source,
                    platform,
                    key,
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
                        "[DL] #{} Enqueued: '{}' ({}) source='{}' platform='{}' dest='{}'",
                        id, file_name, format_bytes(total_bytes), source.name, platform, dest_path.display()
                    );
                    q.entries.push_back(DownloadEntry {
                        id,
                        state: DownloadState::Queued,
                        source_name: source.name.clone(),
                        platform,
                        key,
                        file_name,
                        game_key,
                        dest_path,
                        total_bytes,
                        downloaded_bytes: 0,
                        speed: 0.0,
                        error: None,
                    });
                    save_queue_locked(&q);
                    let _ = event_tx.send(DownloadEvent::StateChanged {
                        id,
                        state: DownloadState::Queued,
                    });
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
                            let _ = event_tx.send(DownloadEvent::StateChanged {
                                id,
                                state: DownloadState::Paused,
                            });
                        }
                    }
                }
                DownloadCommand::Resume(id) => {
                    eprintln!("[DL] #{} Resume requested", id);
                    let mut q = queue.lock().unwrap();
                    if let Some(entry) = q.find_mut(id) {
                        if entry.state == DownloadState::Paused {
                            entry.state = DownloadState::Queued;
                            eprintln!("[DL] #{} '{}': Resumed, re-queued", id, entry.file_name);
                            save_queue_locked(&q);
                            let _ = event_tx.send(DownloadEvent::StateChanged {
                                id,
                                state: DownloadState::Queued,
                            });
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
                        eprintln!("[DL] #{} '{}': Cancelled and removed from queue", id, file_name);
                    }
                }
                DownloadCommand::Shutdown => {
                    eprintln!("[DL] Shutdown requested, stopping worker");
                    return;
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
                let file_name = entry.file_name.clone();
                let dest = entry.dest_path.clone();
                let total_bytes = entry.total_bytes;
                let id = entry.id;

                // Use actual file size on disk for resume offset (survives crash)
                let offset = std::fs::metadata(&dest)
                    .map(|m| m.len())
                    .unwrap_or(0);
                entry.downloaded_bytes = offset;

                if offset > 0 {
                    eprintln!(
                        "[DL] #{} '{}': Resuming from {} / {} (file exists on disk)",
                        id, file_name, format_bytes(offset), format_bytes(total_bytes)
                    );
                } else {
                    eprintln!(
                        "[DL] #{} '{}': Starting download ({}) to {}",
                        id, file_name, format_bytes(total_bytes), dest.display()
                    );
                }

                // Find the Source from config
                let source = config
                    .sources
                    .iter()
                    .find(|s| s.name == source_name)
                    .cloned();

                if source.is_none() {
                    eprintln!("[DL] #{} '{}': ERROR — source '{}' not found in config!", id, file_name, source_name);
                }

                save_queue_locked(&q);
                let _ = event_tx.send(DownloadEvent::StateChanged {
                    id,
                    state: DownloadState::Active,
                });

                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_clone = cancel.clone();
                let event_tx_clone = event_tx.clone();
                let queue_clone = queue.clone();

                let handle = std::thread::spawn(move || {
                    download_worker(
                        id,
                        source,
                        &key,
                        &file_name,
                        &dest,
                        offset,
                        total_bytes,
                        cancel_clone,
                        event_tx_clone,
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
    key: &str,
    file_name: &str,
    dest: &std::path::Path,
    offset: u64,
    total_bytes: u64,
    cancel: Arc<AtomicBool>,
    event_tx: Sender<DownloadEvent>,
    queue: Arc<Mutex<DownloadQueue>>,
) -> Result<(), BackendError> {
    let source = source.ok_or_else(|| {
        BackendError::DownloadFailed("Source not found in config".to_string())
    })?;

    eprintln!("[DL] #{} '{}': Creating backend for source '{}'", id, file_name, source.name);
    let be = backend::create_backend(&source)?;
    eprintln!("[DL] #{} '{}': Backend created, starting download of key '{}'", id, file_name, key);

    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<DownloadProgress>();

    let file_name_owned = file_name.to_string();
    let event_tx2 = event_tx.clone();
    let queue2 = queue.clone();
    let progress_thread = std::thread::spawn(move || {
        let mut last_report = Instant::now();
        let mut last_bytes = offset;
        let mut last_log = Instant::now();

        while let Ok(p) = progress_rx.recv() {
            let now = Instant::now();
            let elapsed = now.duration_since(last_report).as_secs_f64();
            let speed = if elapsed > 0.1 {
                let delta = p.bytes_downloaded.saturating_sub(last_bytes) as f64;
                let s = delta / elapsed;
                last_report = now;
                last_bytes = p.bytes_downloaded;
                s
            } else {
                0.0
            };

            // Update queue entry
            if let Ok(mut q) = queue2.lock() {
                if let Some(entry) = q.find_mut(id) {
                    entry.downloaded_bytes = p.bytes_downloaded;
                    entry.speed = speed;
                }
            }

            // Log progress every 5 seconds
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
                    format_bytes(speed as u64)
                );
                last_log = now;
            }

            // Send UI events at ~10 Hz
            if elapsed > 0.1 || p.bytes_downloaded == p.total_bytes {
                let _ = event_tx2.send(DownloadEvent::Progress {
                    id,
                    downloaded: p.bytes_downloaded,
                    total: p.total_bytes,
                    speed,
                });
            }
        }
    });

    let result = be.download_object(key, dest, offset, total_bytes, &cancel, &progress_tx);

    drop(progress_tx);
    let _ = progress_thread.join();

    match &result {
        Ok(()) => eprintln!("[DL] #{} '{}': download_object completed successfully", id, file_name),
        Err(e) => eprintln!("[DL] #{} '{}': download_object error: {}", id, file_name, e),
    }

    result
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
    let persistable: Vec<&DownloadEntry> = q
        .entries
        .iter()
        .filter(|e| {
            matches!(
                e.state,
                DownloadState::Queued | DownloadState::Active | DownloadState::Paused
            )
        })
        .collect();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml::to_string(&persistable) {
        let _ = std::fs::write(&path, yaml);
    }
}

fn load_queue() -> Result<Vec<DownloadEntry>, String> {
    let path = queue_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_yaml::from_str(&contents).map_err(|e| e.to_string())
}
