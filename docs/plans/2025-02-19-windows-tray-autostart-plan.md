# Windows系统托盘和开机自启功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在Windows平台上实现系统托盘图标（区分服务器运行/停止状态）、退出确认对话框，以及开机自启功能（开机不显示主界面且自动启动服务器）

**Architecture:** 使用Tauri v2原生tray-icon feature实现系统托盘，Windows注册表实现开机自启，通过命令行参数`--autostart`识别开机启动模式，条件控制窗口显示和服务器自动启动

**Tech Stack:** Tauri v2, Rust, Windows Registry API, React

---

## 前置条件

- Tauri v2 项目已配置
- 项目路径: `/mnt/d/GitRepos/camera-ftp-companion`
- 当前功能：FTP服务器可在前台启动/停止

---

## Task 1: 配置Cargo.toml添加tray-icon feature

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 修改Cargo.toml添加tray-icon feature**

将tauri依赖从：
```toml
tauri = { version = "2", features = [] }
```
修改为：
```toml
tauri = { version = "2", features = ["tray-icon"] }
```

**Step 2: 验证配置**

运行: `cd src-tauri && cargo check`
Expected: 成功编译无错误

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "feat: add tray-icon feature for Windows system tray"
```

---

## Task 2: 配置tauri.conf.json支持窗口隐藏

**Files:**
- Modify: `src-tauri/tauri.conf.json`

**Step 1: 修改窗口配置**

将windows配置从：
```json
"windows": [
  {
    "title": "图传伴侣",
    "width": 400,
    "height": 700,
    "resizable": true,
    "fullscreen": false
  }
]
```

修改为：
```json
"windows": [
  {
    "title": "图传伴侣",
    "width": 400,
    "height": 700,
    "resizable": true,
    "fullscreen": false,
    "visible": true,
    "skipTaskbar": false
  }
]
```

**Step 2: 验证JSON格式**

Expected: 无语法错误

**Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat: configure window for tray support"
```

---

## Task 3: 准备托盘图标资源

**Files:**
- Create: `src-tauri/icons/tray-idle.png` (服务器停止状态 - 灰色图标)
- Create: `src-tauri/icons/tray-active.png` (服务器运行状态 - 彩色/高亮图标)

**Step 1: 创建图标目录**

图标已存在于 `src-tauri/icons/`，只需要添加托盘专用图标

**Step 2: 复制并修改图标**

建议方案：
- `tray-idle.png`: 使用现有的icon.png转换为灰色版本（16x16或32x32）
- `tray-active.png`: 使用现有的icon.png作为彩色版本（16x16或32x32）

注意：托盘图标建议使用16x16或32x32像素的PNG格式

**Step 3: 如果无法创建图标文件，暂时使用现有图标**

在代码中使用 `icons/icon.png` 作为默认图标，后续可替换

**Step 4: Commit**

```bash
git add src-tauri/icons/
git commit -m "feat: add system tray icons"
```

---

## Task 4: 实现Windows系统托盘功能

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`

**Step 1: 重写windows.rs实现完整托盘功能**

```rust
use tauri::{AppHandle, Manager, Wry};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::commands::FtpServerState;
use crate::ftp::FtpServerHandle;

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 创建菜单项
    let show_i = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let start_i = MenuItem::with_id(app, "start", "启动服务器", true, None::<&str>)?;
    let stop_i = MenuItem::with_id(app, "stop", "停止服务器", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_i,
        &start_i,
        &stop_i,
        &separator,
        &quit_i,
    ])?;

    // 获取图标路径（使用默认图标）
    let icon_path = app.path().resolve(
        "icons/icon.png",
        tauri::path::BaseDirectory::Resource,
    )?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .menu_on_left_click(false) // 左键点击不显示菜单，而是显示窗口
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // 左键点击显示主窗口
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "start" => {
                    // 通过命令启动服务器
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state: tauri::State<'_, FtpServerState> = app_handle.state();
                        let mut server_guard = state.0.lock().await;
                        if server_guard.is_none() {
                            // 加载配置并启动服务器
                            let config = crate::config::AppConfig::load().unwrap_or_default();
                            match crate::ftp::FtpServerHandle::start(config.ftp).await {
                                Ok(handle) => {
                                    *server_guard = Some(handle);
                                    let _ = app_handle.emit("server-started", ());
                                }
                                Err(e) => {
                                    tracing::error!("Failed to start server: {}", e);
                                }
                            }
                        }
                    });
                }
                "stop" => {
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state: tauri::State<'_, FtpServerState> = app_handle.state();
                        let mut server_guard = state.0.lock().await;
                        if let Some(server) = server_guard.take() {
                            let _ = server.stop().await;
                            let _ = app_handle.emit("server-stopped", ());
                        }
                    });
                }
                "quit" => {
                    // 发送退出事件给前端确认
                    let _ = app.emit("tray-quit-request", ());
                }
                _ => {}
            }
        })
        .build(app)?;

    tracing::info!("System tray initialized successfully");
    Ok(())
}

// 更新托盘图标（根据服务器状态）
pub fn update_tray_icon(app: &AppHandle, is_running: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id("main") {
        // 这里可以根据状态切换图标
        // 需要准备两个图标资源：tray-idle.png 和 tray-active.png
        // let icon_path = if is_running {
        //     app.path().resolve("icons/tray-active.png", tauri::path::BaseDirectory::Resource)?
        // } else {
        //     app.path().resolve("icons/tray-idle.png", tauri::path::BaseDirectory::Resource)?
        // };
        // tray.set_icon(tauri::image::Image::from_path(icon_path)?)?;
        
        // 暂时只记录日志，后续添加图标切换
        tracing::info!("Tray icon would update to: {}", if is_running { "active" } else { "idle" });
    }
    Ok(())
}
```

**Step 2: 修改lib.rs中setup_tray调用（确保错误处理）**

lib.rs中的调用保持不变，但要确保能够编译

**Step 3: 编译测试**

运行: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 4: Commit**

```bash
git add src-tauri/src/platform/windows.rs
git commit -m "feat: implement Windows system tray with menu and click handlers"
```

---

## Task 5: 实现开机自启注册表操作

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`

**Step 1: 在windows.rs添加开机自启函数**

在文件末尾添加：

```rust
use winreg::enums::*;
use winreg::RegKey;
use std::env;
use std::path::PathBuf;

const AUTOSTART_REGISTRY_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_REGISTRY_NAME: &str = "CameraFtpCompanion";

/// 设置开机自启
pub fn set_autostart(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(AUTOSTART_REGISTRY_KEY)?;
    
    if enable {
        // 获取当前可执行文件路径
        let exe_path = env::current_exe()?;
        let exe_path_str = exe_path.to_string_lossy();
        // 添加 --autostart 参数
        let value = format!("\"{}\" --autostart", exe_path_str);
        key.set_value(APP_REGISTRY_NAME, &value)?;
        tracing::info!("Autostart enabled: {}", value);
    } else {
        key.delete_value(APP_REGISTRY_NAME)?;
        tracing::info!("Autostart disabled");
    }
    
    Ok(())
}

/// 检查是否已设置开机自启
pub fn is_autostart_enabled() -> Result<bool, Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(AUTOSTART_REGISTRY_KEY)?;
    
    match key.get_value::<String, _>(APP_REGISTRY_NAME) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// 检查当前是否是通过开机自启启动的
pub fn is_autostart_mode() -> bool {
    env::args().any(|arg| arg == "--autostart")
}
```

**Step 2: 添加winreg依赖到Cargo.toml**

修改 `src-tauri/Cargo.toml`，在[dependencies]部分添加：

```toml
winreg = "0.52"
```

**Step 3: 编译测试**

运行: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/platform/windows.rs
git commit -m "feat: implement Windows autostart registry operations"
```

---

## Task 6: 添加Tauri命令暴露自启设置给前端

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 在commands.rs添加自启相关命令**

在文件末尾添加：

```rust
/// 设置开机自启
#[tauri::command]
pub fn set_autostart_command(enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::set_autostart(enable)
            .map_err(|e| format!("Failed to set autostart: {}", e))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Autostart is only supported on Windows".to_string())
    }
}

/// 获取开机自启状态
#[tauri::command]
pub fn get_autostart_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::is_autostart_enabled()
            .map_err(|e| format!("Failed to get autostart status: {}", e))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}
```

**Step 2: 在lib.rs中注册新命令**

在invoke_handler中添加新命令：

```rust
.invoke_handler(tauri::generate_handler![
    start_server,
    stop_server,
    get_server_status,
    get_network_info,
    load_config,
    save_config,
    check_port_available,
    get_diagnostic_info,
    set_autostart_command,  // 新增
    get_autostart_status,   // 新增
])
```

**Step 3: 编译测试**

运行: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add autostart commands for frontend"
```

---

## Task 7: 修改lib.rs处理启动参数和自动启动

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 修改run函数处理开机启动模式**

修改lib.rs中的`run`函数：

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging to file
    setup_logging();

    // 检查是否是开机启动模式
    let is_autostart = cfg!(target_os = "windows") && crate::platform::windows::is_autostart_mode();
    
    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = platform::windows::setup_tray(app.handle()) {
                    eprintln!("Failed to setup tray: {}", e);
                }
            }

            // 获取主窗口并控制显示
            if let Some(window) = app.get_webview_window("main") {
                if is_autostart {
                    // 开机启动模式：隐藏窗口
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                }
            }

            // 如果是开机启动模式，自动启动服务器
            if is_autostart {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 延迟一点时间确保配置加载完成
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    
                    let state: tauri::State<'_, FtpServerState> = app_handle.state();
                    let mut server_guard = state.0.lock().await;
                    
                    if server_guard.is_none() {
                        let config = crate::config::AppConfig::load().unwrap_or_default();
                        match crate::ftp::FtpServerHandle::start(config.ftp).await {
                            Ok(handle) => {
                                *server_guard = Some(handle);
                                let _ = app_handle.emit("server-started", ());
                                tracing::info!("Server auto-started on autostart");
                            }
                            Err(e) => {
                                tracing::error!("Failed to auto-start server: {}", e);
                            }
                        }
                    }
                });
            }

            // 启动统计信息推送定时器
            let app_handle = app.handle().clone();
            let state: tauri::State<'_, FtpServerState> = app.state();
            let state_clone = state.0.clone();

            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
                let mut last_snapshot: Option<crate::ftp::types::ServerStateSnapshot> = None;

                loop {
                    interval.tick().await;

                    let server_guard = state_clone.lock().await;
                    if let Some(server) = server_guard.as_ref() {
                        let snapshot = server.get_snapshot().await;

                        if snapshot.is_running {
                            let should_emit = match &last_snapshot {
                                None => true,
                                Some(last) => {
                                    last.connected_clients != snapshot.connected_clients
                                        || last.files_received != snapshot.files_received
                                        || last.bytes_received != snapshot.bytes_received
                                        || last.last_file != snapshot.last_file
                                }
                            };

                            if should_emit {
                                let _ = app_handle.emit("stats-update", &snapshot);
                                last_snapshot = Some(snapshot);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_status,
            get_network_info,
            load_config,
            save_config,
            check_port_available,
            get_diagnostic_info,
            set_autostart_command,
            get_autostart_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**Step 2: 处理托盘退出事件**

在setup中添加托盘退出事件监听：

```rust
// 监听托盘退出请求事件
let app_handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    use tauri::event::Event;
    
    // 这里需要通过前端来显示确认对话框
    // 前端监听到 tray-quit-request 后显示对话框
});
```

**Step 3: 编译测试**

运行: `cd src-tauri && cargo check`
Expected: 编译成功

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: handle autostart mode - hide window and auto-start server"
```

---

## Task 8: 前端实现退出确认对话框和自启设置

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/InfoCard.tsx` 或创建新组件

**Step 1: 在App.tsx中添加退出确认逻辑**

修改App.tsx添加托盘退出事件监听：

```typescript
import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { exit } from '@tauri-apps/plugin-process';
import { getCurrentWindow } from '@tauri-apps/api/window';

function App() {
  const { initializeListeners } = useServerStore();
  const [showQuitDialog, setShowQuitDialog] = useState(false);

  useEffect(() => {
    // 初始化事件监听器
    let cleanup: (() => Promise<void>) | null = null;
    let trayQuitUnlisten: (() => void) | null = null;
    
    const setupListeners = async () => {
      cleanup = await initializeListeners();
      
      // 监听托盘退出请求
      trayQuitUnlisten = await listen('tray-quit-request', async () => {
        setShowQuitDialog(true);
      });
    };
    
    setupListeners();
    
    return () => {
      if (cleanup) {
        cleanup();
      }
      if (trayQuitUnlisten) {
        trayQuitUnlisten();
      }
    };
  }, [initializeListeners]);

  // 处理退出确认
  const handleQuitConfirm = async (quit: boolean) => {
    if (quit) {
      // 真正退出程序
      await exit(0);
    } else {
      // 最小化到托盘（隐藏窗口）
      const window = getCurrentWindow();
      await window.hide();
      setShowQuitDialog(false);
    }
  };

  // ... 其余代码不变

  return (
    <div className="min-h-screen bg-gray-50">
      {/* 退出确认对话框 */}
      {showQuitDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-sm mx-4 shadow-xl">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              确认退出
            </h3>
            <p className="text-gray-600 mb-4">
              您是要退出程序还是最小化到系统托盘？
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => handleQuitConfirm(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
              >
                最小化到托盘
              </button>
              <button
                onClick={() => handleQuitConfirm(true)}
                className="px-4 py-2 text-white bg-red-600 rounded-lg hover:bg-red-700 transition-colors"
              >
                退出程序
              </button>
            </div>
          </div>
        </div>
      )}
      
      {/* 原有内容 */}
      {/* ... */}
    </div>
  );
}
```

**Step 2: 添加前端依赖**

需要添加dialog和process插件。检查package.json，如果没有则安装：

```bash
bun add @tauri-apps/plugin-dialog @tauri-apps/plugin-process
```

**Step 3: 修改tauri.conf.json添加权限**

在capabilities/default.json中添加：

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open",
    "dialog:allow-ask",
    "process:allow-exit"
  ]
}
```

**Step 4: 在InfoCard或其他适当位置添加自启开关**

```typescript
// 添加到InfoCard.tsx
const [autostartEnabled, setAutostartEnabled] = useState(false);

useEffect(() => {
  // 获取当前自启状态
  invoke('get_autostart_status')
    .then(status => setAutostartEnabled(status as boolean))
    .catch(console.error);
}, []);

const toggleAutostart = async () => {
  try {
    await invoke('set_autostart_command', { enable: !autostartEnabled });
    setAutostartEnabled(!autostartEnabled);
  } catch (e) {
    console.error('Failed to toggle autostart:', e);
  }
};
```

**Step 5: Commit**

```bash
git add src/App.tsx package.json src-tauri/capabilities/default.json
git commit -m "feat: frontend quit confirmation dialog and autostart toggle"
```

---

## Task 9: 完整构建测试

**Step 1: 安装前端依赖**

```bash
bun install
```

**Step 2: 构建完整应用**

```bash
bun run tauri build
```

Expected: 构建成功，生成exe文件

**Step 3: 测试托盘功能**

1. 运行生成的exe
2. 验证托盘图标是否显示
3. 验证左键点击是否显示窗口
4. 验证右键菜单是否可用
5. 测试启动/停止服务器菜单项

**Step 4: 测试退出确认**

1. 点击托盘"退出"
2. 验证确认对话框是否显示
3. 测试"最小化到托盘"按钮
4. 测试"退出程序"按钮

**Step 5: 测试开机自启**

1. 在前端开启"开机自启"开关
2. 验证注册表是否写入（使用regedit查看）
3. 模拟开机自启启动：`camera-ftp-companion.exe --autostart`
4. 验证窗口是否隐藏
5. 验证服务器是否自动启动

**Step 6: Commit**

```bash
git commit -m "test: verify system tray and autostart functionality"
```

---

## Task 10: 更新版本和文档

**Step 1: 更新Cargo.toml版本号**（如果有版本管理需求）

**Step 2: 更新CHANGELOG.md**

记录新增功能：
- Windows系统托盘图标
- 服务器状态图标区分
- 退出确认对话框
- 开机自启功能

**Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: update changelog for system tray and autostart features"
```

---

## 总结

完成以上任务后，应用将具备：

1. ✅ Windows系统托盘图标（始终显示）
2. ✅ 托盘菜单（显示窗口、启动/停止服务器、退出）
3. ✅ 退出确认对话框（退出程序或最小化到托盘）
4. ✅ 开机自启功能（通过注册表）
5. ✅ 开机启动时自动隐藏窗口并启动服务器
6. ⚠️ 托盘图标状态切换（需要准备两个图标文件后启用）

**后续优化（可选）：**
- 准备不同状态的托盘图标（灰色/彩色）
- 添加托盘图标 tooltip 显示服务器状态
- 添加双击托盘图标显示窗口

---

**执行注意事项：**

1. 每次修改后都运行 `cargo check` 确保编译通过
2. 使用 `bun run tauri build` 进行完整构建测试
3. 在Windows虚拟机或实体机上测试最终功能
4. 注册表操作需要管理员权限（在Windows上自动处理）
