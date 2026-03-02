# Agent Instructions

AI agents working on this codebase follow these rules.

---

## ⚠️ Critical Rules

### 1. Build Commands (Required)

Use the build script instead of `cargo build` or `bun`:

| Command | Output |
|---------|--------|
| `./build.sh gen-types` | Generate TypeScript bindings |
| `./build.sh frontend` | Build frontend only |
| `./build.sh windows` | Windows executable |
| `./build.sh android` | Android APK |
| `./build.sh windows android` | Build both in parallel |
| `./build.sh clean` | Clean all build cache |

Verify all code changes with the appropriate build command.

### 2. LSP Tools (Disabled)

LSP tools hang or timeout in this environment. Avoid:
- `lsp_diagnostics`
- `lsp_goto_definition`
- `lsp_find_references`
- `lsp_rename`

---

## Code Style

### TypeScript / React

- **Target**: ES2020
- **Module**: ESNext with Bundler resolution
- **JSX**: react-jsx
- **Strict mode**: Enabled
- **Styling**: TailwindCSS utility classes

```typescript
import { useState } from 'react';

function Component() {
  const [data, setData] = useState<string | null>(null);
  return <div className="p-4 bg-gray-100">{data}</div>;
}
```

### Rust

- **Edition**: 2021
- **Error handling**: `Result<T, AppError>` with `?` operator
- **Logging**: `tracing::info!`, `tracing::error!`
- **Platform code**: `#[cfg(target_os = "...")]`

```rust
#[command]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    tracing::info!("Starting FTP server...");
    Ok(result)
}
```

### Kotlin (Android)

- **Indent**: 4 spaces
- **Logging**: `Log.d(TAG, "message")` with companion object constants
- **JS Bridge**: `@JavascriptInterface` annotation on public methods
- **Null safety**: `?.let` / `?: run` preferred over explicit null checks

```kotlin
class MyBridge(private val activity: MainActivity) {
    companion object {
        private const val TAG = "MyBridge"
    }

    @JavascriptInterface
    fun doSomething(value: String?) {
        Log.d(TAG, "Called with: $value")
        value?.let { activity.process(it) } 
            ?: run { Log.w(TAG, "Null value") }
    }
}
```

### Tauri IPC

**Frontend:**
```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<string>('command_name', { arg: value });
```

**Backend:** Register in `src-tauri/src/lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    command_name,
    // ...
])
```

---

## Common Tasks

### Add Tauri Command

1. Add function in `src-tauri/src/commands.rs`
2. Register in `src-tauri/src/lib.rs`
3. Call from frontend via `invoke()`
4. **Verify**: `./build.sh windows && ./build.sh android`

### Add React Component

1. Create file in `src/components/`
2. Import and use in `src/App.tsx`
3. Style with TailwindCSS
4. **Verify**: `./build.sh frontend`

### Add JS Bridge (Android)

1. Add class in `src-tauri/gen/android/.../MainActivity.kt` or new file
2. Annotate public methods with `@JavascriptInterface`
3. Register in `MainActivity.onWebViewCreate()`: `addJsBridge(webView, bridgeInstance, "BridgeName")`
4. Call from frontend: `window.BridgeName?.methodName()`
5. **Verify**: `./build.sh android`

---

## ⚠️ Common Pitfalls

### Type Generation with ts-rs

All Rust structs shared with TypeScript use ts-rs for automatic type generation. **Always use generated types—never write manual interfaces.**

**1. Add ts-rs to new Rust struct**
```rust
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MyConfig {
    pub enabled: bool,
    pub port_start: u16,  // → portStart in TypeScript
}
```

**2. Generate TypeScript bindings**
```bash
./build.sh gen-types
```

This uses Windows cargo.exe to run tests and generate bindings. Output: `src-tauri/bindings/MyConfig.ts`

**3. Import in TypeScript**
```typescript
import type { MyConfig } from '../types';  // Re-exports from bindings/
```

**4. Update types/index.ts** (add re-export if new type)
```typescript
export type { MyConfig } from '../../src-tauri/bindings/MyConfig';
```

### Config Backward Compatibility

Add `#[serde(default)]` to new `AppConfig` fields:

```rust
#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub port: u16,
    #[serde(default)]  // Supports old config files
    pub new_field: NewConfig,
}
```

Without `#[serde(default)]`, loading configs missing new fields fails.

---

## References

- [Tauri v2](https://tauri.app/)
- [Rust](https://doc.rust-lang.org/)
- [React](https://react.dev/)
- [TailwindCSS](https://tailwindcss.com/)
- [libunftp](https://docs.rs/libunftp/)
