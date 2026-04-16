# AI Edit Feature Design

## Overview

Add an automatic AI image editing feature to CameraFTP. After successfully receiving JPG or HEIF files via FTP, the image is sent to an AI provider (currently Volcengine SeedEdit) for enhancement. The edited image is saved separately, preserving the original.

## Requirements

- Trigger automatically after FTP file reception (JPG/HEIF)
- Support manual trigger from Gallery UI
- Send image + preset prompt to OpenAI-compatible API provider
- Currently support Volcengine SeedEdit only, but design for extensibility
- Pre-process images: resize long side to configurable max, re-encode as JPEG, base64 encode
- Save edited images to `{save_path}/AIEdit/` subdirectory
- Filename format: `{original_stem}_AIEdit_{YYYYMMDD_HHmmss}.jpg`
- Serial queue processing — no parallelism, no API rate limit concerns
- Non-blocking: must not affect FTP receive/write pipeline in any way
- Two independent toggles: master `enabled` switch + `auto_edit` (auto-trigger on receive)
  - `enabled=false`: entire AI edit feature disabled (both auto and manual)
  - `enabled=true, auto_edit=false`: only manual trigger from Gallery
  - `enabled=true, auto_edit=true`: both auto and manual trigger
- All configuration persisted in config.json, editable from Config UI

## Module Structure

```
src-tauri/src/ai_edit/
├── mod.rs              — Module entry, public exports
├── config.rs           — AiEditConfig, ProviderConfig, SeedEditConfig
├── service.rs          — AiEditService (serial queue, lifecycle)
├── image_processor.rs  — Image preprocessing (resize, HEIF→JPEG, base64)
└── providers/
    ├── mod.rs          — AiEditProvider trait + factory
    └── seededit.rs     — Volcengine SeedEdit provider implementation
```

## Provider Abstraction

```rust
// providers/mod.rs
#[async_trait]
pub trait AiEditProvider: Send + Sync {
    async fn edit_image(&self, image_base64: &str, prompt: &str) -> Result<Vec<u8>, AppError>;
}

pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn AiEditProvider>, AppError> {
    match config {
        ProviderConfig::SeedEdit(cfg) => Ok(Box::new(SeedEditProvider::new(cfg)?)),
    }
}
```

Adding a new provider requires:
1. New file in `providers/` (e.g., `gpt_edit.rs`)
2. New variant in `ProviderConfig` enum
3. New match arm in `create_provider`

## Configuration

```rust
// config.rs

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AiEditConfig {
    pub enabled: bool,
    pub auto_edit: bool,
    pub prompt: String,
    /// Provider-specific config (currently only SeedEdit)
    pub provider: ProviderConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProviderConfig {
    SeedEdit(SeedEditConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct SeedEditConfig {
    /// API Key — the only user-configurable field for this provider
    pub api_key: String,
}
```

Provider constants (base_url, model) are hardcoded in the provider implementation, not exposed in config. When `enabled=true`, the user only needs to provide an API Key.

### Config JSON Example

```json
{
  "aiEdit": {
    "enabled": false,
    "autoEdit": true,
    "prompt": "提升画质，使照片更清晰",
    "provider": {
      "type": "seed-edit",
      "apiKey": ""
    }
  }
}
```

### Config UI Fields

| Field | Type | Editable | Notes |
|-------|------|----------|-------|
| enabled | toggle | yes | Master switch — always visible in CardHeader |
| autoEdit | toggle | yes | Auto-trigger on FTP receive (hidden when enabled=false) |
| prompt | text | yes | Preset prompt for AI editing (hidden when enabled=false) |
| apiKey | password | yes | SeedEdit API Key (hidden when enabled=false) |

### Config UI Behavior

Follow the existing "Advanced Connection Settings" pattern (`ConfigCard.tsx`):

- `CardHeader` with title + `ToggleSwitch` always renders
- When `enabled=false`, the entire config content section is hidden (conditional rendering: `{enabled && <AiEditConfigPanel ... />}`)
- When `enabled=true`, show the full config panel with autoEdit toggle, prompt textarea, and apiKey input

### Hardcoded Constants

These values are defined in code, not exposed in config:

| Constant | Value | Location |
|----------|-------|----------|
| `MAX_LONG_SIDE` | 4096 | `image_processor.rs` |
| SeedEdit base_url | `https://ark.cn-beijing.volces.com/api/v3` | `providers/seededit.rs` |
| SeedEdit model | `doubao-seededit-3-0-i2i-250628` | `providers/seededit.rs` |

### Integration with AppConfig

Add `ai_edit: AiEditConfig` field to `AppConfig` in `config.rs`.

TypeScript bindings auto-generated via ts-rs, re-exported from `src/types/index.ts`.

## AiEditService

```rust
pub struct AiEditService {
    config_service: Arc<ConfigService>,
    sender: mpsc::Sender<AiEditTask>,
}
```

### Lifecycle

1. `new(config_service)` — create `mpsc::channel(32)`, spawn background worker task
2. `on_file_uploaded(path)` — non-blocking push to channel, returns immediately
3. Worker consumes tasks serially: read config → check `enabled` AND `auto_edit` → preprocess → call provider → save result

### Automatic Trigger

Auto-trigger fires only when `enabled=true` AND `auto_edit=true`. The check happens inside the worker (not at enqueue), so config changes take effect immediately.

In `FtpDataListener::receive_data_event()` (`ftp/listeners.rs`), within `DataEvent::Put` branch, after existing auto_open and file_index handling:

```rust
if is_image {
    if let Some(handle) = app_handle.as_ref() {
        let full_path = save_path.join(&path);
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            if wait_for_file_ready(&full_path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                let ai_edit: tauri::State<'_, AiEditService> = handle_clone.state();
                ai_edit.on_file_uploaded(full_path).await;
            }
        });
    }
}
```

### Manual Trigger

New Tauri command:

```rust
#[command]
pub async fn trigger_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_path: String,
) -> Result<String, AppError> {
    let output_path = ai_edit.edit_single(PathBuf::from(&file_path)).await?;
    Ok(output_path.to_string_lossy().to_string())
}
```

`edit_single` enqueues the task with a `oneshot::Sender` to await the result. Manual and automatic requests share the same serial queue — no concurrency conflicts.

Frontend: "AI Edit" button in Gallery image context menu or EXIF panel. Calls `invoke('trigger_ai_edit', { filePath })`, refreshes gallery on success.

### App Initialization

In `lib.rs` setup:

```rust
app.manage(AiEditService::new(config_service));
```

Register `trigger_ai_edit` in `invoke_handler`.

## Non-Blocking Guarantee

| Stage | Mechanism | Blocks FTP? |
|-------|-----------|-------------|
| FTP data receive | `sender.try_send(task)` — O(1), non-blocking | No |
| Queue consumption | Independent tokio task | No |
| Image preprocessing | CPU work in worker task | No |
| API call | Async HTTP in worker task | No |
| Image download | Async HTTP in worker task | No |
| File write | `tokio::fs::write` in worker task | No |
| File indexing | Async call in worker task | No |

```rust
pub async fn on_file_uploaded(&self, file_path: PathBuf) {
    if let Err(e) = self.sender.try_send(AiEditTask { file_path }) {
        tracing::warn!("AI edit queue full, dropping task: {}", e);
    }
}
```

`on_file_uploaded` always enqueues (if queue has capacity). The worker checks `enabled && auto_edit` before processing — if either is false, the task is silently skipped. This means auto-trigger can be toggled at runtime without restarting the server.

## Image Preprocessing

```rust
// image_processor.rs
pub fn prepare_for_upload(file_path: &Path) -> Result<String, AppError> {
    // 1. Read image (image crate with jpeg + heic features)
    // 2. Calculate scale ratio: if long side > MAX_LONG_SIDE (4096), scale proportionally
    // 3. Re-encode as JPEG (quality 85) regardless of original format
    // 4. Base64 encode and return
}
```

## SeedEdit Provider

API endpoint: `POST {base_url}/v1/images/generations`

Request:
```json
{
  "model": "doubao-seededit-3-0-i2i-250628",
  "prompt": "...",
  "image": "data:image/jpeg;base64,...",
  "response_format": "url"
}
```

Response:
```json
{
  "data": [{ "url": "https://..." }],
  "usage": { "generated_images": 1 }
}
```

The provider:
1. Sends POST request with base64 image and prompt
2. Parses response to extract image URL
3. Downloads the image from the returned URL
4. Returns `Vec<u8>` (JPEG bytes)

## Result Storage Flow

1. Preprocess image → base64
2. Provider.edit_image() → Vec<u8> (edited JPEG bytes)
3. Generate filename: `{original_stem}_AIEdit_{YYYYMMDD_HHmmss}.jpg`
4. Ensure `{save_path}/AIEdit/` directory exists (create if needed)
5. Write to `{save_path}/AIEdit/{filename}`
6. Notify FileIndexService::add_file() to index the new file
7. Frontend auto-refreshes via `file-index-changed` event

Example: `save_path/DCIM/AIEdit/IMG_0123_AIEdit_20260414_153207.jpg`

## Error Handling

New `AppError` variant:

```rust
AiEditError(String),
```

- **Automatic trigger**: errors logged via `tracing::error!`, no impact on FTP flow
- **Manual trigger**: errors returned through `Result` to frontend for user display

## Dependency Changes (Cargo.toml)

```toml
# Extend image crate features
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "heic"] }
# New HTTP client
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
```

## Files Changed Summary

| File | Change |
|------|--------|
| `src-tauri/src/ai_edit/mod.rs` | New — module entry |
| `src-tauri/src/ai_edit/config.rs` | New — config structs |
| `src-tauri/src/ai_edit/service.rs` | New — AiEditService |
| `src-tauri/src/ai_edit/image_processor.rs` | New — image preprocessing |
| `src-tauri/src/ai_edit/providers/mod.rs` | New — trait + factory |
| `src-tauri/src/ai_edit/providers/seededit.rs` | New — SeedEdit provider |
| `src-tauri/src/config.rs` | Add `ai_edit` field to AppConfig |
| `src-tauri/src/error.rs` | Add `AiEditError` variant |
| `src-tauri/src/lib.rs` | Register module, manage service, register command |
| `src-tauri/src/ftp/listeners.rs` | Add ai_edit trigger in DataEvent::Put |
| `src-tauri/src/commands/` | New ai_edit command file or addition |
| `src-tauri/Cargo.toml` | Update image features, add reqwest |
| `src/types/index.ts` | Re-export new generated types |
| Frontend Config UI | New AiEdit config panel |
| Frontend Gallery | "AI Edit" button |
