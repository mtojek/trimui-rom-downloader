# Architecture Overview

ROM Downloader is a Rust SDL2 application for the TrimUI Smart Pro (TG5040) console. It lets users browse Internet Archive game catalogs and download ROMs directly to the SD card.

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    Main Thread (60 FPS)                   │
│                                                          │
│  ┌─────────┐   ┌──────┐   ┌─────────┐   ┌───────────┐  │
│  │  Intro  │──▶│ Menu │──▶│ Loading │──▶│  Browser  │  │
│  │  Scene  │   │ Scene│   │  Scene  │   │   Scene   │  │
│  └─────────┘   └──┬───┘   └─────────┘   └─────┬─────┘  │
│                   │                            │         │
│                   ▼                            ▼         │
│              ┌─────────┐              ┌──────────────┐   │
│              │ MyGames │              │DownloadManager│  │
│              │  Scene  │◀────events───│  (commands)   │  │
│              └─────────┘              └──────┬───────┘   │
│                                              │           │
└──────────────────────────────────────────────┼───────────┘
                                               │
                              ┌────────────────▼──────────┐
                              │     Worker Thread          │
                              │  ┌──────────────────────┐  │
                              │  │ Download Thread (x2) │  │
                              │  │  reqwest + tokio      │  │
                              │  │  Range resume         │  │
                              │  │  ZIP extraction       │  │
                              │  └──────────────────────┘  │
                              └────────────────────────────┘
```

## Scene State Machine

The app is organized as a state machine of scenes. Each scene implements the `Scene` trait with `update()` and `render()` methods. Transitions are driven by user input and async results.

```
Intro ──▶ Menu ──▶ Loading ──▶ Browser
            │         │
            │         └──▶ Menu (on cancel/done)
            │
            └──▶ MyGames ──▶ Menu (on back)
            │
            └──▶ Error (on config failure)
```

## Threading Model

| Thread | Purpose | Communication |
|--------|---------|---------------|
| Main | SDL2 event loop, rendering, scene management | Polls events from DownloadManager |
| Worker | Manages download queue, spawns download threads | Receives commands via mpsc channel |
| Download (x2 max) | HTTP download + optional ZIP extraction | Updates shared queue via Arc<Mutex>, progress via mpsc |
| Loading | Fetches game catalogs from backend | Sends log messages via mpsc, cancellable via AtomicBool |

## Module Map

```
src/
├── main.rs          SDL2 init, window creation (1280×720)
├── app.rs           Main loop, scene orchestration, event polling
├── scene.rs         Scene trait definition
│
├── intro.rs         Animated intro (crab sprites, cart slide)
├── menu.rs          Main menu + source browser
├── loading.rs       Catalog fetch with progress log
├── browser.rs       Alphabetical game browser
├── mygames.rs       Download queue + installed games UI
├── error.rs         Error display scene
│
├── config.rs        YAML config parsing + validation
├── backend.rs       Internet Archive S3 API (SourceBackend trait)
├── download.rs      Download manager, queue, worker, ZIP extraction
├── cache.rs         Catalog cache (7-day staleness)
├── library.rs       MyGames persistence (mygames.yaml)
├── install_dir.rs   Platform → ROM directory mapping
│
├── input.rs         Keyboard + controller input, key repeat
├── text.rs          CPU font rasterization (fontdue)
├── texture.rs       PNG/JPEG → SDL2 texture
├── background.rs    Background image + version overlay
└── widget.rs        Reusable menu widget
```
