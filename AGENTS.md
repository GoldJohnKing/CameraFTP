# Agent Instructions

Instructions for AI agents working on this codebase.

---

## ⚠️ Critical Rules

### 1. Build Commands (MUST)

Always use the build scripts. Never use `cargo build` or `bun` directly.

```bash
./build.sh <command>
```

| Command | Description |
|---------|-------------|
| `./build.sh windows` | Build Windows executable |
| `./build.sh android` | Build Android APK (release) |
| `./build.sh frontend` | Build frontend only |

### 2. Code Verification (MUST)

**Never use `lsp_diagnostics`**. Always verify by compiling.

```bash
# After Rust changes
./build.sh windows && ./build.sh android

# After frontend changes
./build.sh frontend

# After Kotlin changes
./build.sh android
```

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
- **Error handling**: Use `Result<T, AppError>` and `?` operator
- **Logging**: Use `tracing::info!`, `tracing::error!`
- **Platform code**: Use `#[cfg(target_os = "...")]`

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
- **Logging**: Use `Log.d(TAG, "message")` with `companion object` constants
- **JS Bridge**: Annotate with `@JavascriptInterface`
- **Null safety**: Prefer `?.let` / `?: run` over null checks

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

### Add New Tauri Command

1. Add function in `src-tauri/src/commands.rs`
2. Register in `src-tauri/src/lib.rs`
3. Call from frontend via `invoke()`
4. **Verify**: `./build.sh windows && ./build.sh android`

### Add New React Component

1. Create file in `src/components/`
2. Import and use in `src/App.tsx`
3. Use TailwindCSS for styling
4. **Verify**: `./build.sh frontend`

### Add New JS Bridge (Android)

1. Add class in `src-tauri/gen/android/.../MainActivity.kt` or create new file
2. Annotate public methods with `@JavascriptInterface`
3. Register in `MainActivity.onWebViewCreate()`: `addJsBridge(webView, bridgeInstance, "BridgeName")`
4. Call from frontend: `window.BridgeName?.methodName()`
5. **Verify**: `./build.sh android`

---

## Android Debugging (WSL)

Use Windows `adb.exe` when device is connected to Windows host:

```bash
# Check devices
adb.exe devices

# View crash logs
adb.exe logcat -d -s AndroidRuntime:E

# Install APK
adb.exe install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk

# Start app
adb.exe shell am start -n com.gjk.cameraftpcompanion/.MainActivity
```

---

## References

- [Tauri v2](https://tauri.app/)
- [Rust](https://doc.rust-lang.org/)
- [React](https://react.dev/)
- [TailwindCSS](https://tailwindcss.com/)
- [libunftp](https://docs.rs/libunftp/)
