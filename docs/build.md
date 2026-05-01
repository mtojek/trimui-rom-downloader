# Build & Deployment

## Dependencies

- Rust stable toolchain
- SDL2 development libraries
- Docker (for console builds only)

## Development Build (macOS)

```bash
brew install sdl2
cp sources.yaml.example sources.yaml
# Edit sources.yaml with your credentials

make dev
```

`make dev` creates temporary ROM directories under `/tmp/trimui-rom-downloader/` and runs the app with `TRD_ROM_BASE_DIR` pointing there.

## Console Build (TrimUI Smart Pro)

```bash
make tsp
```

This runs inside Docker using the `tg5040-toolchain` image:
1. Builds Docker image with Rust toolchain and TG5040 cross-compiler
2. Cross-compiles for `aarch64-unknown-linux-gnu`
3. Links against TG5040 SDL2 libraries with correct RPATH
4. Copies binary to `ROM Downloader.pak/trimui-rom-downloader`

### Docker Image

Based on `ghcr.io/loveretro/tg5040-toolchain:modernize`. Adds:
- Rust stable via rustup
- ca-certificates and curl

### Cross-Compilation Details

- Linker: `aarch64-nextui-linux-gnu-gcc`
- SDL2 headers/libs from TG5040 SDK prefix
- RPATH set to SDK prefix for runtime library resolution
- Target: `aarch64-unknown-linux-gnu`

## Installation on Console

1. Copy `ROM Downloader.pak/` to `/mnt/SDCARD/Tools/tg5040/` on the SD card
2. Place `sources.yaml` inside `ROM Downloader.pak/`
3. Launch from the Tools menu

## Runtime Directories

The app creates `.rom-downloader/` next to the executable for:
- `downloads.yaml` — active download queue
- `mygames.yaml` — installed games library
- `cache/sources/` — cached game catalogs

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TRD_ROM_BASE_DIR` | `/mnt/SDCARD/Roms` | ROM installation base directory |
| `RUST_BACKTRACE` | unset | Enable panic backtraces |
| `LIBRARY_PATH` | unset | SDL2 library path (macOS dev) |

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make dev` | Build and run locally |
| `make build` | Release build for host |
| `make tsp` | Cross-compile for TrimUI |
| `make docker-shell` | Interactive shell in build container |
| `make clean` | Remove build artifacts and temp dirs |
