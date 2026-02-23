# 后续优化方案

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 进一步优化代码架构，使用 Platform Trait 替代 lib.rs 中的内联 `#[cfg]`，清理不再使用的代码。

**Architecture:** 使用 `get_platform()` 统一调用平台功能，删除废弃文件。

**Tech Stack:** Rust (Tauri v2), TypeScript/React

---

## Task 1: 迁移 lib.rs 使用 Platform Trait

**当前问题：** `lib.rs` 中仍有大量内联 `#[cfg(target_os = "windows")]` 代码，可以使用 Platform Trait 简化。

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/traits.rs`（如需添加方法）

### Step 1: 分析 lib.rs 中可迁移的代码

当前 lib.rs 中的平台特定代码：

1. **初始化阶段** (约第116-127行):
   ```rust
   #[cfg(target_os = "android")]
   {
       config::init_android_paths(app.handle());
   }

   #[cfg(target_os = "windows")]
   {
       if let Err(e) = platform::windows::setup_tray(app.handle()) {
           eprintln!("Failed to setup tray: {}", e);
       }
   }
   ```
   → 可用 `platform.setup(app.handle())` 替代

2. **开机自启检测** (约第101-105行):
   ```rust
   #[cfg(target_os = "windows")]
   let is_autostart = crate::platform::windows::is_autostart_mode();
   
   #[cfg(not(target_os = "windows"))]
   let is_autostart = false;
   ```
   → 需要在 PlatformService trait 中添加 `is_autostart_mode()` 方法

3. **统计轮询中的托盘图标更新** (约第227-245行):
   ```rust
   #[cfg(target_os = "windows")]
   {
       let new_tray_state = if snapshot.connected_clients > 0 {
           crate::platform::windows::TrayIconState::Active
       } else {
           crate::platform::windows::TrayIconState::Idle
       };
       // ...
   }
   ```
   → 可用 `platform.update_server_state(app, connected_clients)` 替代

4. **开机自启服务器启动** (约第151-187行):
   ```rust
   #[cfg(target_os = "windows")]
   if is_autostart {
       // 启动服务器并更新托盘图标
   }
   ```
   → 需要在 trait 中添加相关方法

### Step 2: 扩展 PlatformService Trait

在 `src-tauri/src/platform/traits.rs` 中添加新方法：

```rust
pub trait PlatformService: Send + Sync {
    // ... 现有方法 ...
    
    /// 检查是否是开机自启模式
    fn is_autostart_mode(&self) -> bool {
        false  // 默认实现，Windows 可覆盖
    }
    
    /// 开机自启时启动服务器后的回调
    fn on_autostart_server_started(&self, _app: &AppHandle) {}
}
```

### Step 3: 更新 WindowsPlatform 实现

```rust
impl PlatformService for WindowsPlatform {
    fn is_autostart_mode(&self) -> bool {
        is_autostart_mode()  // 调用现有的函数
    }
    
    fn on_autostart_server_started(&self, app: &AppHandle) {
        // 更新托盘图标为 idle 状态
        if let Err(e) = update_tray_icon(app, TrayIconState::Idle) {
            tracing::warn!("Failed to update tray icon on autostart: {}", e);
        }
    }
}
```

### Step 4: 重构 lib.rs

```rust
use platform::get_platform;

pub fn run() {
    setup_logging();
    
    let platform = get_platform();
    let is_autostart = platform.is_autostart_mode();
    
    if is_autostart {
        tracing::info!("Running in autostart mode");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
            // 统一初始化
            if let Err(e) = platform.setup(app.handle()) {
                eprintln!("Platform setup failed: {}", e);
            }

            // 窗口事件处理（保持不变）
            // ...

            // 开机自启服务器启动
            if is_autostart {
                let app_handle = app.handle().clone();
                let platform_ref = platform;
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    // 启动服务器...
                    platform_ref.on_autostart_server_started(&app_handle);
                });
            }

            // 统计轮询（使用 platform.update_server_state）
            // ...

            Ok(())
        })
        // ...
}
```

### Step 5: 验证编译

```bash
cd src-tauri && cargo build
```

### Step 6: 提交

```bash
git add src-tauri/src/lib.rs src-tauri/src/platform/traits.rs src-tauri/src/platform/windows.rs
git commit -m "refactor: use Platform trait in lib.rs, remove inline #[cfg]"
```

---

## Task 2: 删除废弃的 useTauriListeners.ts

**当前问题：** `useTauriListeners.ts` 不再被任何文件使用，所有监听器已合并到 `serverStore.ts`。

**Files:**
- Delete: `src/hooks/useTauriListeners.ts`

### Step 1: 确认文件未被引用

```bash
grep -r "useTauriListeners" src/ --include="*.tsx" --include="*.ts"
```

Expected: 仅出现 `src/hooks/useTauriListeners.ts` 自身

### Step 2: 删除文件

```bash
rm src/hooks/useTauriListeners.ts
```

### Step 3: 验证构建

```bash
bun run build
```

### Step 4: 提交

```bash
git add -A
git commit -m "refactor: remove unused useTauriListeners hook"
```

---

## Task 3: 更新 lib.rs 的 autostart 逻辑

**当前问题：** 开机自启服务器的启动逻辑仍在 lib.rs 中，需要清理和简化。

### 当前代码结构

```rust
#[cfg(target_os = "windows")]
if is_autostart {
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        let state: tauri::State<'_, FtpServerState> = app_handle.state();
        
        match crate::ftp::server_factory::start_ftp_server(&state.0, Default::default()).await {
            Ok(ctx) => {
                // 大量内联代码...
            }
            Err(e) => {
                tracing::error!("Failed to auto-start server: {}", e);
            }
        }
    });
}
```

### 重构方案

将启动逻辑移到 PlatformService trait 中：

```rust
// traits.rs
pub trait PlatformService: Send + Sync {
    /// 执行开机自启逻辑（如果有）
    fn execute_autostart(&self, _app: &AppHandle, _state: &FtpServerState) {}
}
```

```rust
// windows.rs
impl PlatformService for WindowsPlatform {
    fn execute_autostart(&self, app: &AppHandle, state: &FtpServerState) {
        let app_handle = app.clone();
        let state_clone = state.0.clone();
        
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            match crate::ftp::server_factory::start_ftp_server(&state_clone, Default::default()).await {
                Ok(ctx) => {
                    tracing::info!("FTP server auto-started on {}:{}", ctx.ip, ctx.port);
                    crate::ftp::server_factory::spawn_event_processor(
                        app_handle.clone(),
                        ctx.event_bus,
                        500
                    );
                    crate::ftp::server_factory::emit_server_started(&app_handle, &ctx.ip, ctx.port);
                }
                Err(e) => {
                    tracing::error!("Failed to auto-start server: {}", e);
                }
            }
        });
    }
}
```

```rust
// lib.rs
.setup(move |app| {
    let platform = get_platform();
    let state: tauri::State<'_, FtpServerState> = app.state();
    
    // 统一调用
    platform.setup(app.handle())?;
    
    if platform.is_autostart_mode() {
        platform.execute_autostart(app.handle(), &state);
    }
    
    // ...
})
```

---

## 风险与注意事项

1. **lib.rs 重构风险较高**：涉及 Tauri 生命周期管理，需要仔细测试
2. **autostart 逻辑**：Windows 开机自启功能需要实际测试
3. **建议分阶段执行**：先删除 useTauriListeners.ts（低风险），再逐步迁移 lib.rs

---

## 执行优先级

| 优先级 | 任务 | 风险 | 预估时间 |
|--------|------|------|----------|
| P1 | 删除 useTauriListeners.ts | 低 | 5分钟 |
| P2 | 扩展 Platform Trait | 中 | 30分钟 |
| P3 | 重构 lib.rs 使用 Trait | 高 | 2小时 |
