# Backend & Network

## Internet Archive S3 API

The app uses Internet Archive's S3-compatible API to list and download files.

### Authentication

```
Authorization: LOW {access_key}:{secret_key}
```

Credentials are shared across sources via named references in `sources.yaml`.

### Listing Games

```
GET https://archive.org/metadata/{bucket_name}/files
```

Returns JSON with file entries. Files are filtered by optional `path` prefix from bucket config. Each `RemoteGame` stores `key`, `file_size`, and `bucket_name`.

### Downloading

```
GET https://archive.org/download/{bucket_name}/{url_encoded_key}
```

- Redirect handling: follows 301/302/307/308 up to 10 hops
- Resume: `Range: bytes={offset}-` header, accepts 200 (restart) or 206 (resume)
- Retry: up to 5 attempts on 500/502/503 with 5s delay
- Progress: streamed via `reqwest::Response::bytes_stream()`

### HEAD Request

Used at startup to resolve `total_bytes` for persisted downloads that don't have file size yet.

## SourceBackend Trait

```rust
pub trait SourceBackend: Send + Sync {
    fn list_bucket(&self, bucket, log, cancel) -> Result<Vec<RemoteGame>>;
    fn download_object(&self, bucket_name, key, dest, offset, total_bytes, cancel, progress) -> Result<()>;
    fn head_object(&self, bucket_name, key) -> Result<u64>;
}
```

Currently one implementation: `IABackend` (Internet Archive). The trait allows adding other backends in the future.

## ZIP Extraction

When `source.extract = true` and the downloaded file is a `.zip`:

1. Download completes → state changes to `Unpacking`
2. Archive is scanned for `.cue` files to detect bin/cue disc images
3. Total uncompressed size calculated for progress bar
4. Files extracted in 1MB chunks with per-chunk progress updates
5. Directory structure inside ZIP is flattened (only file names kept)
6. Archive deleted after successful extraction

### Extraction Destination

- **bin/cue archives** (ZIP contains `.cue` file): extracted into a game subdirectory (`platform_dir/game_key/`), since bin/cue games consist of multiple related files
- **All other archives**: extracted flat into the platform directory (`platform_dir/`), no subdirectory created

## File Installation Layout

Downloaded files are placed directly in the platform ROM directory:

```
/mnt/SDCARD/Roms/
├── Nintendo Entertainment System (FC)/
│   ├── Game1.nes                    ← single file, flat
│   └── Game2.nes
├── Sony PlayStation (PS)/
│   ├── Game1.chd                    ← single file, flat
│   └── Game2/                       ← bin/cue, subdirectory
│       ├── Game2.bin
│       └── Game2.cue
```

Previously all games were placed in their own subdirectory. Now only bin/cue games (extracted from ZIP) get a subdirectory. Single files (`.chd`, `.nes`, `.gba`, etc.) and non-bin/cue ZIP contents are placed directly in the platform directory.

## Caching

Game catalogs are cached per bucket in YAML files under `.rom-downloader/cache/sources/`.

- Staleness threshold: 7 days
- Cache path: `{source_name}/{bucket_name}_{path}.yaml`
- "Refresh All" from menu invalidates all caches and re-fetches
- Cache age displayed in source browser menu
