# Data Flow

## Download Lifecycle

```
User selects game in Browser
        │
        ▼
DownloadCommand::Enqueue ──mpsc──▶ Worker Thread
        │
        ▼
    ┌─────────┐
    │ Queued  │ (persisted to downloads.yaml)
    └────┬────┘
         │ slot available (max 2 concurrent)
         ▼
    ┌─────────┐     HEAD request
    │ Active  │────────────────▶ resolve total_bytes
    └────┬────┘
         │ reqwest stream + Range resume
         │ progress → Arc<Mutex<Queue>>
         ▼
    ┌───────────┐  (if source.extract && .zip)
    │ Unpacking │──────────────▶ extract in 1MB chunks
    └─────┬─────┘               delete .zip after
          │
          ▼
    ┌───────────┐
    │ Completed │──▶ DownloadEvent::Completed ──▶ app.rs adds to MyGames
    └───────────┘
```

### Alternative Paths

```
Active ──▶ Paused (user pause, preserves partial file)
Paused ──▶ Queued (user resume, resumes from file offset)
Active ──▶ Failed (network error, retry available)
Failed ──▶ Queued (user retry)
Any    ──▶ Cancelled (removes entry + partial file + empty dir)
```

## Catalog Loading

```
Menu ──▶ LoadingScene
              │
              ▼
         Background thread
              │
              ├── For each bucket in source:
              │     ├── Cache fresh? ──▶ Load from disk
              │     └── Cache stale? ──▶ GET /metadata/{bucket}/files
              │                              │
              │                              ▼
              │                         Parse JSON, filter by path prefix
              │                              │
              │                              ▼
              │                         Save to cache YAML
              │
              ▼
         Merge all games ──▶ GameBrowser
```

## Persistence

| File | Format | Contents |
|------|--------|----------|
| `sources.yaml` | YAML | User config: sources, credentials |
| `.rom-downloader/downloads.yaml` | YAML | Active download queue (source, platform, key, bucket, state) |
| `.rom-downloader/mygames.yaml` | YAML | Installed games library (key, source, platform) |
| `.rom-downloader/cache/sources/{source}/{bucket}.yaml` | YAML | Cached game listings (key, file_size, bucket_name) |

All `.rom-downloader/` files are stored next to the executable.

## Input Flow

```
SDL2 Event
    │
    ▼
InputHandler::handle_event()
    │
    ├── Keyboard: arrows, O/P, Return, Backspace, X, Y, Escape
    ├── Controller buttons: DPad, A/B/X/Y, shoulders
    ├── Controller axes: TriggerLeft/Right (L2/R2)
    └── Button up events: stop key repeat
    │
    ▼
InputAction enum ──▶ Active scene's handle_input()
    │
    ▼
InputHandler::poll_repeat() ──▶ auto-repeat for held directional buttons
                                (300ms delay, 50ms interval)
```
