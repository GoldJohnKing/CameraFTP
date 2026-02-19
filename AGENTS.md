# Agent Instructions

Follow these rules when working on this codebase.

---

## Critical Rules

### 1. Build Commands

NEVER use `bun` or `cargo build` directly. Use `./build.sh windows android` instead.

This command builds Android and Windows in parallel, making it much faster than building them one by one.

### 2. LSP Tools

Do not use LSP tools in this environment. They hang or timeout.

- `lsp_diagnostics`
- `lsp_goto_definition`
- `lsp_find_references`
- `lsp_rename`

### 3. Verify Code Changes

ALWAYS build both platform to verify code changes: run `./build.sh windows android`.

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
- **Null safety**: Prefer `?.let` / `?: run` to explicit null checks

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

**Backend:** Register commands in `src-tauri/src/lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    command_name,
    // ...
])
```

---

## License Headers

**Always add SPDX license headers to new source files.**

The project is licensed under AGPL-3.0. Use the appropriate comment syntax for each language:

```rust
// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later
```

- **Rust**: `//` single-line comments
- **TypeScript/Kotlin**: `/** */` block comments (same format)

---

## Common Tasks

### Add Tauri Command

1. Add the function to `src-tauri/src/commands.rs`
2. Register it in `src-tauri/src/lib.rs`
3. Call it from the frontend via `invoke()`
4. Verify: `./build.sh windows && ./build.sh android`

### Add React Component

1. Create the file in `src/components/`
2. Import and use it in `src/App.tsx`
3. Style it with TailwindCSS
4. Verify: `./build.sh frontend`

### Add JS Bridge (Android)

1. Add the class to `src-tauri/gen/android/.../MainActivity.kt` or a new file
2. Annotate public methods with `@JavascriptInterface`
3. Register it in `MainActivity.onWebViewCreate()`:
   ```kotlin
   addJsBridge(webView, bridgeInstance, "BridgeName")
   ```
4. Call it from the frontend: `window.BridgeName?.methodName()`
5. Verify: `./build.sh android`

---

## Common Pitfalls

### Type Generation with ts-rs

The project uses ts-rs to generate TypeScript bindings from Rust structs. **Use generated types. Never write manual interfaces.**

**1. Add ts-rs to a Rust struct:**
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

**2. Generate TypeScript bindings:**
```bash
./build.sh gen-types
```

This runs Windows cargo.exe to generate bindings. Output: `src-tauri/bindings/MyConfig.ts`

**3. Import the type in TypeScript:**
```typescript
import type { MyConfig } from '../types';  // Re-exports from bindings/
```

**4. Update types/index.ts:**
```typescript
export type { MyConfig } from '../../src-tauri/bindings/MyConfig';
```

---

## References

- [Tauri v2](https://tauri.app/)
- [Rust](https://doc.rust-lang.org/)
- [React](https://react.dev/)
- [TailwindCSS](https://tailwindcss.com/)
- [libunftp](https://docs.rs/libunftp/)
