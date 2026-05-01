# Configuration

The app reads `sources.yaml` from the same directory as the executable.

## Format

```yaml
sources:
  - name: "Display Name"
    type: s3_archive
    credentials: credential_key
    platform: "PS"
    extract: false
    buckets:
      - name: "ia-item-identifier"
        path: "optional/prefix"

credentials:
  credential_key:
    access_key: "your-access-key"
    secret_key: "your-secret-key"
```

## Fields

### Source

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Display name in menu |
| `type` | yes | Backend type, currently only `s3_archive` |
| `credentials` | yes | Reference to a key in `credentials` section |
| `platform` | yes | Platform code matching ROM folder (e.g. `PS`, `FC`, `GBA`) |
| `extract` | no | Auto-extract ZIP files after download (default: `false`) |
| `buckets` | yes | List of Internet Archive items to browse |

### Bucket

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | Internet Archive item identifier |
| `path` | no | Path prefix to filter files within the item (default: empty = all files) |

### Credentials

| Field | Required | Description |
|-------|----------|-------------|
| `access_key` | yes | Internet Archive S3 access key |
| `secret_key` | yes | Internet Archive S3 secret key |

## Platform Codes

The platform code must match the suffix in parentheses of a ROM directory name on the SD card:

| Directory Name | Code |
|---------------|------|
| `Sony PlayStation (PS)` | `PS` |
| `Nintendo Entertainment System (FC)` | `FC` |
| `Game Boy Advance (GBA)` | `GBA` |
| `Super Nintendo Entertainment System (SFC)` | `SFC` |
| `Sega Genesis (MD)` | `MD` |

The app scans the ROM base directory and extracts platform codes from directory names automatically.

## Multi-Bucket Sources

A single source can have multiple buckets. All games from all buckets are merged into one browsable list. Each game remembers which bucket it came from for downloading.

```yaml
- name: "PlayStation Collection"
  type: s3_archive
  credentials: archive_org
  platform: "PS"
  buckets:
    - name: "ps1_games_part1"
    - name: "ps1_games_part2"
    - name: "ps1_games_part3"
```

## Path Filtering

When a bucket has a `path`, only files under that prefix are shown. The path is normalized at load time (leading/trailing slashes removed, double slashes collapsed).

```yaml
buckets:
  - name: "mixed-rom-collection"
    path: "CHD-PSX-EUR"    # only files under this directory
```

## Validation

The app validates config at startup and shows an error screen if:
- No sources defined
- Source has empty name, credentials, or platform
- Referenced credentials not found
- Source has no buckets
- Bucket has empty name
