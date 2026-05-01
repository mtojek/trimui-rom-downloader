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
2. Total uncompressed size calculated for progress bar
3. Files extracted in 1MB chunks with per-chunk progress updates
4. Files are flattened (directory structure inside ZIP is ignored)
5. Archive deleted after successful extraction

This handles bin/cue PlayStation games distributed as ZIP archives.

## Caching

Game catalogs are cached per bucket in YAML files under `.rom-downloader/cache/sources/`.

- Staleness threshold: 7 days
- Cache path: `{source_name}/{bucket_name}_{path}.yaml`
- "Refresh All" from menu invalidates all caches and re-fetches
- Cache age displayed in source browser menu
