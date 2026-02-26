# Windows 自动预览图片功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**目标:** 实现 Windows 平台专属的图片自动预览功能，包含独立悬浮预览窗口、多种打开方式配置、以及从统计界面手动打开预览的能力。

**架构:** 
- 后端新增 `AutoOpenService` 处理文件上传事件，根据配置决定如何打开图片
- 前端新增 `PreviewWindow` 组件作为独立悬浮窗口，支持全屏和自适应缩放
- 配置系统扩展支持自动预览开关、打开方式选择、预览窗口行为设置
- `StatsCard` 最新文件区域整体可点击打开预览窗口

**技术栈:** 
- Rust: Tauri v2, tokio, windows crate (ShellExecuteW)
- Frontend: React, TypeScript, Zustand, TailwindCSS
- IPC: Tauri Commands + Events

---

## 配置定义

### Rust: `src-tauri/src/config.rs` 新增字段

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ImageOpenMethod {
    BuiltInPreview,
    SystemDefault,
    WindowsPhotos,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PreviewWindowConfig {
    pub enabled: bool,
    pub method: ImageOpenMethod,
    pub auto_bring_to_front: bool,
    pub remember_position: bool,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
}

// 添加到 AppConfig
pub struct AppConfig {
    // ... 现有字段 ...
    #[cfg(target_os = "windows")]
    pub preview_config: PreviewWindowConfig,
}

impl Default for PreviewWindowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: ImageOpenMethod::BuiltInPreview,
            auto_bring_to_front: false,
            remember_position: false,
            window_x: None,
            window_y: None,
            window_width: Some(1024),
            window_height: Some(768),
        }
    }
}
```

### TypeScript: `src/types/index.ts` 新增类型

```typescript
export type ImageOpenMethod = 
  | 'built-in-preview'
  | 'system-default'
  | 'windows-photos'
  | 'custom';

export interface PreviewWindowConfig {
  enabled: boolean;
  method: ImageOpenMethod;
  autoBringToFront: boolean;
  rememberPosition: boolean;
  windowX?: number;
  windowY?: number;
  windowWidth?: number;
  windowHeight?: number;
}
```

---

## Task 1: Rust 后端 - AutoOpenService 模块

**文件:**
- 创建: `src-tauri/src/auto_open/mod.rs`
- 创建: `src-tauri/src/auto_open/service.rs`
- 创建: `src-tauri/src/auto_open/windows.rs`
- 修改: `src-tauri/src/lib.rs` 注册模块
- 修改: `src-tauri/src/config.rs` 添加配置字段

### Step 1: 创建 AutoOpenService 结构

**文件:** `src-tauri/src/auto_open/service.rs`

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Manager};
use tracing::{info, error};

use crate::config::{AppConfig, ImageOpenMethod, PreviewWindowConfig};
use crate::error::AppError;

pub struct AutoOpenService {
    app_handle: AppHandle,
    config: Arc<Mutex<PreviewWindowConfig>>,
}

impl AutoOpenService {
    pub fn new(app_handle: AppHandle) -> Self {
        let config = AppConfig::load().preview_config;
        Self {
            app_handle,
            config: Arc::new(Mutex::new(config)),
        }
    }

    /// 处理文件上传事件
    pub async fn on_file_uploaded(&self, file_path: PathBuf) -> Result<(), AppError> {
        let config = self.config.lock().await;
        
        if !config.enabled {
            return Ok(());
        }

        match config.method {
            ImageOpenMethod::BuiltInPreview => {
                // 发送事件到前端，打开/更新预览窗口
                self.emit_preview_event(file_path, config.auto_bring_to_front).await?;
            }
            ImageOpenMethod::SystemDefault => {
                self.open_with_system_default(&file_path).await?;
            }
            ImageOpenMethod::WindowsPhotos => {
                self.open_with_windows_photos(&file_path).await?;
            }
            ImageOpenMethod::Custom(ref program_path) => {
                self.open_with_custom_program(&file_path, program_path).await?;
            }
        }

        Ok(())
    }

    /// 更新配置
    pub async fn update_config(&self, new_config: PreviewWindowConfig) {
        let mut config = self.config.lock().await;
        *config = new_config;
        
        // 持久化到配置文件
        let mut app_config = AppConfig::load();
        app_config.preview_config = new_config;
        if let Err(e) = app_config.save() {
            error!("Failed to save preview config: {}", e);
        }
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> PreviewWindowConfig {
        self.config.lock().await.clone()
    }

    async fn emit_preview_event(&self, file_path: PathBuf, bring_to_front: bool) -> Result<(), AppError> {
        let event = PreviewEvent {
            file_path: file_path.to_string_lossy().to_string(),
            bring_to_front,
        };
        
        self.app_handle.emit("preview-image", event)
            .map_err(|e| AppError::Internal(format!("Failed to emit preview event: {}", e)))?;
        
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn open_with_system_default(&self, file_path: &PathBuf) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_default(file_path)
    }

    #[cfg(target_os = "windows")]
    async fn open_with_windows_photos(&self, file_path: &PathBuf) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_photos(file_path)
    }

    #[cfg(target_os = "windows")]
    async fn open_with_custom_program(&self, file_path: &PathBuf, program: &str) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_program(file_path, program)
    }
}

#[derive(Clone, serde::Serialize)]
struct PreviewEvent {
    file_path: String,
    bring_to_front: bool,
}
```

### Step 2: Windows 平台实现

**文件:** `src-tauri/src/auto_open/windows.rs`

```rust
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows::Win32::System::Com::CoInitialize;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::error::AppError;

/// 使用系统默认程序打开
pub fn open_with_default(file_path: &PathBuf) -> Result<(), AppError> {
    open_with_shell_execute(file_path, None)
}

/// 使用 Windows 照片应用打开
pub fn open_with_photos(file_path: &PathBuf) -> Result<(), AppError> {
    // 尝试使用 ms-photos:viewer?fileName= URI scheme
    let path_str = file_path.to_string_lossy();
    let uri = format!("ms-photos:viewer?fileName={}", urlencoding::encode(&path_str));
    
    open_with_shell_execute(&uri.parse().unwrap(), None)
        .or_else(|_| open_with_shell_execute(file_path, Some("open")))
}

/// 使用自定义程序打开
pub fn open_with_program(file_path: &PathBuf, program: &str) -> Result<(), AppError> {
    open_with_shell_execute(file_path, Some(program))
}

fn open_with_shell_execute(file_path: &PathBuf, operation: Option<&str>) -> Result<(), AppError> {
    unsafe {
        CoInitialize(None).ok();
    }

    let file_wide: Vec<u16> = OsStr::new(&file_path.to_string_lossy())
        .encode_wide()
        .chain(Some(0))
        .collect();

    let operation_wide: Option<Vec<u16>> = operation.map(|op| {
        OsStr::new(op)
            .encode_wide()
            .chain(Some(0))
            .collect()
    });

    let result = unsafe {
        ShellExecuteW(
            None,
            operation_wide.as_ref().map(|v| windows::core::PCWSTR(v.as_ptr())),
            windows::core::PCWSTR(file_wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };

    if result.0 <= 32 {
        return Err(AppError::Internal(format!(
            "ShellExecute failed with code {}",
            result.0
        )));
    }

    Ok(())
}
```

### Step 3: 模块导出

**文件:** `src-tauri/src/auto_open/mod.rs`

```rust
mod service;
#[cfg(target_os = "windows")]
mod windows;

pub use service::AutoOpenService;
```

### Step 4: 修改 lib.rs 集成

**文件:** `src-tauri/src/lib.rs` 添加:

```rust
mod auto_open;

use auto_open::AutoOpenService;

// 在 Builder 中注册状态
.manage(AutoOpenService::new(app.handle().clone()))

// 在 invoke_handler 中添加命令
.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    get_preview_config,
    set_preview_config,
    open_preview_window,
])
```

### Step 5: 添加 Tauri Commands

**文件:** `src-tauri/src/commands.rs` 添加:

```rust
use crate::auto_open::AutoOpenService;
use crate::config::{PreviewWindowConfig, ImageOpenMethod};

#[tauri::command]
pub async fn get_preview_config(
    auto_open: tauri::State<'_, AutoOpenService>,
) -> Result<PreviewWindowConfig, AppError> {
    Ok(auto_open.get_config().await)
}

#[tauri::command]
pub async fn set_preview_config(
    auto_open: tauri::State<'_, AutoOpenService>,
    config: PreviewWindowConfig,
) -> Result<(), AppError> {
    auto_open.update_config(config).await;
    Ok(())
}

#[tauri::command]
pub async fn open_preview_window(
    app: AppHandle,
    file_path: String,
) -> Result<(), AppError> {
    let event = PreviewEvent {
        file_path,
        bring_to_front: true,
    };
    
    app.emit("preview-image", event)
        .map_err(|e| AppError::Internal(format!("Failed to emit preview event: {}", e)))?;
    
    Ok(())
}

#[derive(Clone, serde::Serialize)]
struct PreviewEvent {
    file_path: String,
    bring_to_front: bool,
}
```

### Step 6: 在 FTP 事件监听中集成

**文件:** `src-tauri/src/ftp/listeners.rs`

在 `FtpDataListener` 中注入 `AutoOpenService` 并处理文件上传事件:

```rust
use crate::auto_open::AutoOpenService;

// 在 receive_data_event 中:
DataEvent::Put { path, bytes } => {
    stats.record_upload(path.clone(), bytes).await;
    event_bus.emit_file_uploaded(path.clone(), bytes);
    
    // Windows 平台自动打开
    #[cfg(target_os = "windows")]
    {
        if let Some(auto_open) = app_handle.try_state::<AutoOpenService>() {
            let full_path = std::path::PathBuf::from(&config.save_path).join(&path);
            if let Err(e) = auto_open.on_file_uploaded(full_path).await {
                tracing::error!("Failed to auto open image: {}", e);
            }
        }
    }
}
```

### Step 7: 编译验证

**运行:**
```bash
./build.sh windows
```

**期望:** 编译成功，无错误。

---

## Task 2: 前端 - 预览窗口组件

**文件:**
- 创建: `src/components/PreviewWindow.tsx`
- 创建: `src/components/PreviewConfigCard.tsx` (Windows 专属配置)
- 修改: `src/App.tsx` 添加预览窗口
- 修改: `src/stores/configStore.ts` 添加预览配置管理

### Step 1: 创建预览窗口组件

**文件:** `src/components/PreviewWindow.tsx`

```typescript
import { useEffect, useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

interface PreviewEvent {
  file_path: string;
  bring_to_front: boolean;
}

interface PreviewWindowState {
  isOpen: boolean;
  currentImage: string | null;
  isFullscreen: boolean;
  autoBringToFront: boolean;
}

export function PreviewWindow() {
  const [state, setState] = useState<PreviewWindowState>({
    isOpen: false,
    currentImage: null,
    isFullscreen: false,
    autoBringToFront: false,
  });
  
  const windowRef = useRef<WebviewWindow | null>(null);

  // 监听 Rust 发来的预览事件
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      unlisten = await listen<PreviewEvent>('preview-image', (event) => {
        const { file_path, bring_to_front } = event.payload;
        
        setState(prev => ({
          ...prev,
          isOpen: true,
          currentImage: file_path,
          autoBringToFront: bring_to_front,
        }));
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 打开/创建预览窗口
  useEffect(() => {
    if (state.isOpen && !windowRef.current) {
      createPreviewWindow();
    }
  }, [state.isOpen]);

  const createPreviewWindow = async () => {
    try {
      const window = new WebviewWindow('preview-window', {
        url: '/preview',
        title: '图片预览',
        width: 1024,
        height: 768,
        center: true,
        resizable: true,
        alwaysOnTop: false,
        decorations: true,
      });

      windowRef.current = window;

      // 窗口关闭时
      window.onCloseRequested(async () => {
        // 显示确认弹窗
        const shouldDisable = await showCloseConfirmation();
        if (shouldDisable) {
          // 禁用自动预览
          await invoke('set_preview_config', {
            config: {
              ...currentConfig,
              enabled: false,
            },
          });
        }
        
        setState(prev => ({ ...prev, isOpen: false }));
        windowRef.current = null;
      });

    } catch (error) {
      console.error('Failed to create preview window:', error);
    }
  };

  // 实际预览窗口内容组件
  if (!state.isOpen) return null;

  return (
    <PreviewWindowContent
      imagePath={state.currentImage}
      isFullscreen={state.isFullscreen}
      autoBringToFront={state.autoBringToFront}
      onClose={handleClose}
      onFullscreenToggle={() => setState(p => ({ ...p, isFullscreen: !p.isFullscreen }))}
    />
  );
}

// 预览窗口内容组件
function PreviewWindowContent({
  imagePath,
  isFullscreen,
  autoBringToFront,
  onClose,
  onFullscreenToggle,
}: {
  imagePath: string | null;
  isFullscreen: boolean;
  autoBringToFront: boolean;
  onClose: () => void;
  onFullscreenToggle: () => void;
}) {
  const [showToolbar, setShowToolbar] = useState(true);
  const toolbarTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // 自动隐藏工具栏
  useEffect(() => {
    if (toolbarTimeoutRef.current) {
      clearTimeout(toolbarTimeoutRef.current);
    }
    
    toolbarTimeoutRef.current = setTimeout(() => {
      setShowToolbar(false);
    }, 3000);

    return () => {
      if (toolbarTimeoutRef.current) {
        clearTimeout(toolbarTimeoutRef.current);
      }
    };
  }, [showToolbar]);

  const handleMouseMove = () => {
    setShowToolbar(true);
  };

  const handleOpenFolder = async () => {
    if (imagePath) {
      await invoke('open_folder_select_file', { filePath: imagePath });
    }
  };

  const handleToggleAutoFront = async () => {
    // 更新配置
    const newConfig = {
      ...currentConfig,
      autoBringToFront: !autoBringToFront,
    };
    await invoke('set_preview_config', { config: newConfig });
  };

  if (!imagePath) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-gray-900">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  return (
    <div 
      className={`w-full h-full flex flex-col bg-gray-900 ${isFullscreen ? 'fixed inset-0 z-50' : ''}`}
      onMouseMove={handleMouseMove}
    >
      {/* 图片区域 - 始终填满，object-cover 保持比例裁剪 */}
      <div className="flex-1 relative overflow-hidden">
        <img
          src={`file://${imagePath}`}
          alt="Preview"
          className="w-full h-full object-cover"
          draggable={false}
          onDoubleClick={onFullscreenToggle}
        />
      </div>

      {/* 底部工具栏 */}
      <div 
        className={`
          bg-gray-800 px-4 py-3 flex items-center justify-between
          transition-opacity duration-300
          ${showToolbar ? 'opacity-100' : 'opacity-0'}
        `}
      >
        {/* 左侧：图片信息 */}
        <div className="text-sm text-gray-300 truncate flex-1">
          {imagePath.split('/').pop()}
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-3">
          {/* 全屏按钮 */}
          <button
            onClick={onFullscreenToggle}
            className="p-2 text-gray-300 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title={isFullscreen ? '退出全屏' : '全屏'}
          >
            {isFullscreen ? '⛶' : '⛶'}
          </button>

          {/* 自动前台按钮 */}
          <button
            onClick={handleToggleAutoFront}
            className={`
              p-2 rounded transition-colors
              ${autoBringToFront 
                ? 'text-blue-400 bg-blue-400/20 hover:bg-blue-400/30' 
                : 'text-gray-400 hover:text-gray-300 hover:bg-gray-700'
              }
            `}
            title={autoBringToFront ? '新图片时自动前台显示 (已开启)' : '新图片时自动前台显示 (已关闭)'}
          >
            🔔
          </button>

          {/* 打开文件夹 */}
          <button
            onClick={handleOpenFolder}
            className="p-2 text-gray-300 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="打开文件夹"
          >
            📂
          </button>
        </div>
      </div>
    </div>
  );
}

// 关闭确认弹窗
async function showCloseConfirmation(): Promise<boolean> {
  // 使用 confirm 或自定义对话框
  return confirm(
    '关闭预览窗口后，自动预览功能将被禁用。\n\n' +
    '点击"确定"关闭并禁用自动预览。\n' +
    '点击"取消"仅关闭窗口（下次有图片时自动打开）。'
  );
}
```

### Step 2: 创建 Windows 专属配置卡片

**文件:** `src/components/PreviewConfigCard.tsx`

```typescript
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { Card, CardHeader } from './ui';
import type { PreviewWindowConfig, ImageOpenMethod } from '../types';

interface PreviewConfigCardProps {
  platform: string;
}

export function PreviewConfigCard({ platform }: PreviewConfigCardProps) {
  const isWindows = platform === 'windows';
  const [config, setConfig] = useState<PreviewWindowConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [customPath, setCustomPath] = useState('');

  useEffect(() => {
    if (isWindows) {
      loadConfig();
    }
  }, [isWindows]);

  const loadConfig = async () => {
    try {
      const loaded = await invoke<PreviewWindowConfig>('get_preview_config');
      setConfig(loaded);
      if (loaded.method === 'custom') {
        // 从配置中恢复自定义路径
      }
    } catch (error) {
      console.error('Failed to load preview config:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const updateConfig = async (updates: Partial<PreviewWindowConfig>) => {
    if (!config) return;
    
    const newConfig = { ...config, ...updates };
    setConfig(newConfig);
    
    try {
      await invoke('set_preview_config', { config: newConfig });
    } catch (error) {
      console.error('Failed to save preview config:', error);
    }
  };

  const handleSelectCustomProgram = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{
        name: '可执行文件',
        extensions: ['exe'],
      }],
    });
    
    if (selected && typeof selected === 'string') {
      setCustomPath(selected);
      updateConfig({ method: 'custom' });
    }
  };

  if (!isWindows) return null;

  return (
    <Card className="overflow-hidden">
      <CardHeader 
        title="🖼️ 自动预览图片" 
        description="Windows 专属功能：相机上传图片后自动显示" 
      />

      <div className="p-4 space-y-6">
        {/* 总开关 */}
        <div className="flex items-center justify-between">
          <div>
            <h4 className="text-sm font-medium text-gray-700">启用自动预览</h4>
            <p className="text-xs text-gray-500 mt-0.5">
              新图片到达时自动打开预览窗口
            </p>
          </div>
          <ToggleSwitch
            checked={config?.enabled ?? false}
            onChange={(checked) => updateConfig({ enabled: checked })}
            disabled={isLoading}
          />
        </div>

        {config?.enabled && (
          <>
            <hr className="border-gray-100" />

            {/* 打开方式选择 */}
            <div className="space-y-3">
              <h4 className="text-sm font-medium text-gray-700">打开方式</h4>
              
              <RadioGroup
                value={config.method}
                onChange={(value) => updateConfig({ method: value as ImageOpenMethod })}
              >
                <RadioOption 
                  value="built-in-preview"
                  label="内置预览窗口"
                  description="独立窗口，支持全屏，单窗口始终显示最新图片"
                  recommended
                />
                
                <RadioOption 
                  value="system-default"
                  label="系统默认程序"
                  description="使用 Windows 默认的图片查看器"
                />
                
                <RadioOption 
                  value="windows-photos"
                  label="Microsoft 照片应用"
                  description="Windows 自带的照片应用"
                />
                
                <RadioOption 
                  value="custom"
                  label="自定义程序"
                  description={customPath || '选择其他程序打开图片'}
                />
              </RadioGroup>

              {config.method === 'custom' && (
                <button
                  onClick={handleSelectCustomProgram}
                  className="mt-2 w-full text-left px-3 py-2 text-sm border border-gray-300 rounded-lg hover:border-blue-400 hover:bg-blue-50 transition-colors"
                >
                  {customPath || '点击选择程序...'}
                </button>
              )}
            </div>

            {/* 内置预览专属设置 */}
            {config.method === 'built-in-preview' && (
              <>
                <hr className="border-gray-100" />
                
                <div className="space-y-3">
                  <h4 className="text-sm font-medium text-gray-700">预览窗口设置</h4>
                  
                  <Checkbox
                    checked={config.autoBringToFront}
                    onChange={(checked) => updateConfig({ autoBringToFront: checked })}
                    label="新图片时自动前台显示"
                    description="预览窗口将获得焦点并显示在最前"
                  />
                  
                  <Checkbox
                    checked={config.rememberPosition}
                    onChange={(checked) => updateConfig({ rememberPosition: checked })}
                    label="记住窗口位置和大小"
                    description="下次打开时恢复到上次关闭的状态"
                  />
                </div>
              </>
            )}
          </>
        )}
      </div>
    </Card>
  );
}

// 辅助组件
function ToggleSwitch({ checked, onChange, disabled }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={`
        relative inline-flex h-6 w-11 items-center rounded-full transition-colors
        ${checked ? 'bg-blue-600' : 'bg-gray-200'}
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
      `}
    >
      <span
        className={`
          inline-block h-4 w-4 transform rounded-full bg-white transition-transform
          ${checked ? 'translate-x-6' : 'translate-x-1'}
        `}
      />
    </button>
  );
}

function RadioGroup({ value, onChange, children }: {
  value: string;
  onChange: (value: string) => void;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-2">
      {children}
    </div>
  );
}

function RadioOption({ 
  value, 
  label, 
  description, 
  recommended 
}: {
  value: string;
  label: string;
  description: string;
  recommended?: boolean;
}) {
  return (
    <label className="flex items-start gap-3 p-3 rounded-lg border border-gray-200 hover:border-blue-300 hover:bg-blue-50/50 cursor-pointer transition-colors">
      <input 
        type="radio" 
        name="open-method" 
        value={value}
        className="mt-0.5"
      />
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700">{label}</span>
          {recommended && (
            <span className="text-xs px-1.5 py-0.5 bg-blue-100 text-blue-700 rounded">
              推荐
            </span>
          )}
        </div>
        <p className="text-xs text-gray-500 mt-0.5">{description}</p>
      </div>
    </label>
  );
}

function Checkbox({ checked, onChange, label, description }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description: string;
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
      />
      <div>
        <span className="text-sm text-gray-700">{label}</span>
        <p className="text-xs text-gray-500">{description}</p>
      </div>
    </label>
  );
}
```

### Step 3: 修改 ConfigCard 集成预览配置

**文件:** `src/components/ConfigCard.tsx` 修改:

```typescript
import { PreviewConfigCard } from './PreviewConfigCard';

// 在 return 中添加：
<ConfigCard>
  {/* ... 现有内容 ... */}
</ConfigCard>

// 预览配置卡片（Windows 专属）
<PreviewConfigCard platform={platform} />
```

### Step 4: 修改 StatsCard 最新文件区域

**文件:** `src/components/StatsCard.tsx` 修改:

```typescript
import { invoke } from '@tauri-apps/api/core';

// 在最新文件显示区域：
const handleOpenPreview = async () => {
  if (lastUploadedFile) {
    const fullPath = `${savePath}/${lastUploadedFile}`;
    await invoke('open_preview_window', { filePath: fullPath });
  }
};

// 修改渲染部分：
<button
  onClick={handleOpenPreview}
  disabled={!lastUploadedFile}
  className={`
    w-full text-left p-3 rounded-lg border transition-colors
    ${lastUploadedFile 
      ? 'border-gray-200 hover:border-blue-300 hover:bg-blue-50 cursor-pointer' 
      : 'border-gray-100 bg-gray-50 cursor-not-allowed'
    }
  `}
>
  <p className="text-xs text-gray-500 mb-1">最新文件</p>
  <p className={`font-medium ${lastUploadedFile ? 'text-gray-900' : 'text-gray-400'}`}>
    {lastUploadedFile || '无'}
  </p>
  <p className="text-xs text-gray-500 mt-1">
    {lastUploadTime || '--'}
  </p>
</button>
```

### Step 5: 更新 configStore 添加预览配置

**文件:** `src/stores/configStore.ts` 添加:

```typescript
import type { PreviewWindowConfig, ImageOpenMethod } from '../types';

interface ConfigState {
  // ... 现有字段 ...
  previewConfig: PreviewWindowConfig | null;
  loadPreviewConfig: () => Promise<void>;
  updatePreviewConfig: (config: PreviewWindowConfig) => Promise<void>;
}

export const useConfigStore = create<ConfigState>()((set, get) => ({
  // ... 现有实现 ...
  
  previewConfig: null,
  
  loadPreviewConfig: async () => {
    try {
      const config = await invoke<PreviewWindowConfig>('get_preview_config');
      set({ previewConfig: config });
    } catch (error) {
      console.error('Failed to load preview config:', error);
    }
  },
  
  updatePreviewConfig: async (config) => {
    try {
      await invoke('set_preview_config', { config });
      set({ previewConfig: config });
    } catch (error) {
      console.error('Failed to update preview config:', error);
      throw error;
    }
  },
}));
```

---

## Task 3: 前端构建验证

### Step 1: 构建验证

**运行:**
```bash
./build.sh frontend
```

**期望:** TypeScript 编译成功，无类型错误。

---

## 完整构建验证

### Step 1: Windows 构建

**运行:**
```bash
./build.sh windows
```

**期望:** Rust 编译成功，生成 Windows 可执行文件。

### Step 2: 功能测试

1. 启动应用，确认配置界面显示"自动预览图片"区块
2. 启用自动预览，选择"内置预览窗口"
3. 模拟或实际上传图片，确认预览窗口自动打开
4. 确认图片填满窗口（object-cover）
5. 测试全屏功能（双击或按钮）
6. 关闭窗口，测试确认弹窗逻辑
7. 从 StatsCard 点击最新文件，确认可手动打开预览

---

## 实现完成

**文档位置:** `docs/plans/2025-02-26-windows-auto-preview-design.md`

**功能概要:**
- ✅ Windows 平台专属自动预览功能
- ✅ 独立悬浮窗口，默认 1024x768，屏幕中心
- ✅ 图片始终填满窗口（object-cover）
- ✅ 支持全屏查看
- ✅ 多种打开方式：内置预览、系统默认、照片应用、自定义程序
- ✅ "自动前台显示"开关
- ✅ 关闭窗口时确认弹窗（可选禁用自动预览）
- ✅ StatsCard 最新文件区域整体可点击打开预览
