# ROM Downloader

A ROM downloader app for the TrimUI Smart Pro (TG5040) console. Browse and download games from Internet Archive directly to your SD card.

## Features

- Browse game sources with alphabetical navigation
- Download games with progress tracking (pause, resume, retry)
- Automatic ZIP extraction for bin/cue games
- Multi-bucket sources (e.g. split archives)
- Local cache for game catalogs
- My Games library to manage downloaded games

## Configuration

Create a `sources.yaml` file next to the binary. See `sources.yaml.example` for the format:

```yaml
sources:
  - name: "PlayStation CHD"
    type: s3_archive
    credentials: my_archive
    platform: "PS"
    buckets:
      - name: "psx-chd-collection"
        path: "CHD-PSX"
  - name: "PlayStation Redump"
    type: s3_archive
    credentials: my_archive
    platform: "PS"
    extract: true
    buckets:
      - name: "psx_redump_part1"
      - name: "psx_redump_part2"
      - name: "psx_redump_part3"

credentials:
  my_archive:
    access_key: "your-access-key"
    secret_key: "your-secret-key"
```

- **credentials**: shared credentials referenced by name from sources
- **platform**: platform code matching the ROM folder name (e.g. `PS`, `FC`, `GBA`)
- **buckets**: Internet Archive item identifiers, optionally with a `path` prefix to filter files
- **extract**: set to `true` to automatically unzip downloaded archives (for bin/cue games)

## Disclaimer

This application is intended for downloading your own legally owned game backups. The author takes no responsibility for any misuse of this software to download copyrighted content you do not own. Use at your own risk.

## Development

### Prerequisites

- Rust toolchain
- SDL2 (`brew install sdl2` on macOS)

### Run locally

```bash
cp sources.yaml.example sources.yaml
# Edit sources.yaml with your credentials

make dev
```

This creates temporary ROM directories under `/tmp/trimui-rom-downloader/` and runs the app.

## Building for TrimUI Smart Pro

The console build uses Docker with the TG5040 cross-compilation toolchain.

```bash
make tsp
```

This will:
1. Build the Docker toolchain image
2. Cross-compile for aarch64
3. Copy the binary to `ROM Downloader.pak/`

### Install on console

1. Copy `ROM Downloader.pak/` to `/mnt/SDCARD/Tools/tg5040/` on your SD card
2. Place your `sources.yaml` inside `ROM Downloader.pak/`
3. Launch "ROM Downloader" from the Tools menu
