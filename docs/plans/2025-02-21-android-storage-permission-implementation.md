# Android存储路径与权限管理重构实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 彻底重构安卓端的存储路径设置和权限管理，采用Toast提示后直接打开SAF选择器的简化方案

**Architecture:** 使用Tauri命令处理权限验证和配置保存，前端通过自定义Hook管理状态，直接调用SAF选择器，无中间弹窗

**Tech Stack:** React + TypeScript (前端), Rust + Tauri (后端), Android SAF (存储访问框架)

---

## 前置准备

### Task 0: 准备工作

**Step 1: 确认当前代码状态**

检查是否有未提交的更改：

```bash
git status
```

如有更改，请先提交或暂存。

**Step 2: 创建功能分支**

```bash
git checkout -b feature/android-storage-permission-redesign
git branch
```

Expected: 当前分支显示为 `feature/android-storage-permission-redesign`

---

## Phase 1: 后端实现 (Rust)

### Task 1: 更新Android平台适配模块

**Files:**
- Modify: `src-tauri/src/platform/android.rs`
- Test: 手动测试

**Step 1: 添加SAF权限验证函数**

```rust
// src-tauri/src/platform/android.rs

/// 检查SAF权限是否有效
#[cfg(target_os = "android")]
pub fn check_saf_permission(app: &AppHandle, uri: &str) -> bool {
    use jni::objects::JString;
    use jni::signature::JavaType;
    
    let uri = uri.to_string();
    
    app.run_on_android_context(move |env, activity| {
        // 通过ContentResolver验证URI是否可访问
        let uri_str = env.new_string(&uri)?;
        let uri_class = env.find_class("android/net/Uri")?;
        let parse_method = env.get_static_method(
            &uri_class,
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[(&uri_str).into()],
        )?;
        let uri_obj = parse_method.l()?;
        
        // 尝试获取文件描述符，验证是否可读
        let content_resolver = env.call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )?;
        let resolver = content_resolver.l()?;
        
        // 尝试打开文件描述符
        let take_flags = 0x01 | 0x02; // FLAG_GRANT_READ_URI_PERMISSION | FLAG_GRANT_WRITE_URI_PERMISSION
        let fd_result = env.call_method(
            &resolver,
            "openFileDescriptor",
            "(Landroid/net/Uri;Ljava/lang/String;Landroid/os/CancellationSignal;)Landroid/os/ParcelFileDescriptor;",
            &[(&uri_obj).into(), (&env.new_string("rw")?).into(), (&JObject::null()).into()],
        );
        
        match fd_result {
            Ok(fd) => {
                // 关闭文件描述符
                let _ = env.call_method(&fd, "close", "()V", &[]);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }).unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
pub fn check_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}
```

**Step 2: 添加持久化权限保存函数**

```rust
/// 持久化SAF权限
#[cfg(target_os = "android")]
pub fn persist_saf_permission(app: &AppHandle, uri: &str) -> bool {
    use jni::objects::JString;
    
    let uri = uri.to_string();
    
    app.run_on_android_context(move |env, activity| {
        let uri_str = env.new_string(&uri)?;
        let uri_class = env.find_class("android/net/Uri")?;
        let parse_method = env.get_static_method(
            &uri_class,
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[(&uri_str).into()],
        )?;
        let uri_obj = parse_method.l()?;
        
        let content_resolver = env.call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )?;
        let resolver = content_resolver.l()?;
        
        let take_flags = 0x01 | 0x02 | 0x40; // READ | WRITE | PERSISTABLE
        env.call_method(
            &resolver,
            "takePersistableUriPermission",
            "(Landroid/net/Uri;I)V",
            &[(&uri_obj).into(), take_flags.into()],
        )?;
        
        Ok(true)
    }).unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
pub fn persist_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}
```

**Step 3: 添加URI转真实路径函数**

```rust
/// 尝试将content:// URI转换为文件路径
#[cfg(target_os = "android")]
pub fn uri_to_file_path(app: &AppHandle, uri: &str) -> Option<String> {
    // 对于SAF返回的URI，通常无法直接转换为文件路径
    // 这里使用DocumentsContract API尝试解析
    // 如果失败，返回None，后续通过ContentResolver操作文件
    
    if uri.starts_with("content://com.android.externalstorage.documents/tree/primary:") {
        // 尝试解析外部存储路径
        let path_part = uri.split("/tree/primary:").nth(1)?;
        let decoded = urlencoding::decode(path_part).ok()?;
        Some(format!("/sdcard/{}", decoded))
    } else {
        None
    }
}

#[cfg(not(target_os = "android"))]
pub fn uri_to_file_path(_app: &AppHandle, _uri: &str) -> Option<String> {
    None
}
```

**Step 4: 验证编译**

```bash
cd src-tauri
cargo check --target aarch64-linux-android 2>&1 | head -50
```

Expected: 无错误（可能有警告）

**Step 5: 提交**

```bash
git add src-tauri/src/platform/android.rs
git commit -m "feat: add SAF permission management functions for Android"
```

---

### Task 2: 创建存储权限管理模块

**Files:**
- Create: `src-tauri/src/storage_permission.rs`
- Modify: `src-tauri/src/lib.rs` (添加模块引用)
- Test: 手动测试

**Step 1: 创建模块文件**

```rust
// src-tauri/src/storage_permission.rs

use std::path::PathBuf;
use tauri::AppHandle;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::platform::android;

/// 存储路径信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoragePathInfo {
    pub path_name: String,
    pub uri: String,
    pub raw_path: Option<String>,
    pub is_valid: bool,
}

/// 启动服务器前检查结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub current_path: Option<StoragePathInfo>,
}

/// 验证存储路径权限是否有效
#[tauri::command]
pub async fn validate_storage_permission(
    app: AppHandle,
    uri: String,
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let is_valid = android::check_saf_permission(&app, &uri);
        Ok(is_valid)
    }
    
    #[cfg(not(target_os = "android"))]
    {
        // 非Android平台直接检查路径是否存在
        let path = PathBuf::from(&uri);
        Ok(path.exists() && path.is_dir())
    }
}

/// 保存存储路径配置
#[tauri::command]
pub async fn save_storage_path(
    app: AppHandle,
    path_name: String,
    uri: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // 持久化权限
        if !android::persist_saf_permission(&app, &uri) {
            warn!("Failed to persist SAF permission for URI: {}", uri);
        }
        
        // 尝试获取真实路径
        let raw_path = android::uri_to_file_path(&app, &uri);
        
        // 更新配置
        let config_path = AppConfig::config_path();
        let mut config = AppConfig::load(&config_path).unwrap_or_default();
        
        config.save_path = PathBuf::from(&path_name);
        // 将URI和原始路径保存到config的扩展字段
        // 需要修改AppConfig结构体添加这些字段
        
        config.save().map_err(|e| format!("保存配置失败: {}", e))?;
        
        info!("Storage path saved: name={}, uri={}, raw={:?}", path_name, uri, raw_path);
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let config_path = AppConfig::config_path();
        let mut config = AppConfig::load(&config_path).unwrap_or_default();
        config.save_path = PathBuf::from(&path_name);
        config.save().map_err(|e| format!("保存配置失败: {}", e))?;
    }
    
    Ok(())
}

/// 获取当前存储路径配置
#[tauri::command]
pub fn get_storage_path(app: AppHandle) -> Result<Option<StoragePathInfo>, String> {
    let config_path = AppConfig::config_path();
    let config = AppConfig::load(&config_path).unwrap_or_default();
    
    if config.save_path.to_string_lossy().is_empty() {
        return Ok(None);
    }
    
    let path_name = config.save_path.to_string_lossy().to_string();
    
    // TODO: 从配置中读取保存的URI
    // 这里需要修改AppConfig结构体
    let uri = String::new(); // 临时
    
    #[cfg(target_os = "android")]
    let is_valid = if !uri.is_empty() {
        android::check_saf_permission(&app, &uri)
    } else {
        false
    };
    
    #[cfg(not(target_os = "android"))]
    let is_valid = config.save_path.exists();
    
    Ok(Some(StoragePathInfo {
        path_name,
        uri,
        raw_path: None,
        is_valid,
    }))
}

/// 检查服务器启动前提条件
#[tauri::command]
pub async fn check_server_start_prerequisites(
    app: AppHandle,
) -> Result<ServerStartCheckResult, String> {
    let storage_info = get_storage_path(app.clone())?;
    
    match storage_info {
        Some(info) => {
            if info.is_valid {
                Ok(ServerStartCheckResult {
                    can_start: true,
                    reason: None,
                    current_path: Some(info),
                })
            } else {
                Ok(ServerStartCheckResult {
                    can_start: false,
                    reason: Some("存储权限已失效，需要重新选择目录".to_string()),
                    current_path: Some(info),
                })
            }
        }
        None => {
            Ok(ServerStartCheckResult {
                can_start: false,
                reason: Some("未配置存储路径".to_string()),
                current_path: None,
            })
        }
    }
}

/// 获取上次使用的URI（用于预选中）
#[tauri::command]
pub fn get_last_storage_uri() -> Result<Option<String>, String> {
    let config_path = AppConfig::config_path();
    let config = AppConfig::load(&config_path).unwrap_or_default();
    
    // TODO: 从配置中读取URI
    Ok(None)
}
```

**Step 2: 修改AppConfig添加URI字段**

```rust
// src-tauri/src/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub port: u16,
    pub autostart: bool,
    
    // Android专用字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_raw: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            port: 2121,
            autostart: false,
            save_path_uri: None,
            save_path_raw: None,
        }
    }
}
```

**Step 3: 更新storage_permission.rs使用新字段**

```rust
// 修改 save_storage_path 函数
config.save_path_uri = Some(uri.clone());
config.save_path_raw = raw_path.clone();

// 修改 get_storage_path 函数
let uri = config.save_path_uri.clone().unwrap_or_default();
```

**Step 4: 在lib.rs中添加模块**

```rust
// src-tauri/src/lib.rs

mod storage_permission;

use storage_permission::{
    check_server_start_prerequisites,
    get_last_storage_uri,
    get_storage_path,
    save_storage_path,
    validate_storage_permission,
};

// 在 generate_handler! 中添加:
.invoke_handler(tauri::generate_handler![
    // ... 现有命令
    validate_storage_permission,
    save_storage_path,
    get_storage_path,
    check_server_start_prerequisites,
    get_last_storage_uri,
])
```

**Step 5: 验证编译**

```bash
cd src-tauri
cargo check 2>&1 | head -30
```

Expected: 无错误

**Step 6: 提交**

```bash
git add src-tauri/src/storage_permission.rs src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: add storage permission management module with SAF support"
```

---

### Task 3: 修改AndroidManifest.xml

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`
- Test: 手动测试

**Step 1: 更新manifest**

```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:tools="http://schemas.android.com/tools">

    <!-- 网络权限 -->
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    
    <!-- 存储权限 -->
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE"
        android:maxSdkVersion="32" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE"
        android:maxSdkVersion="32" />
    <uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE"
        tools:ignore="ScopedStorage" />
    
    <!-- 前台服务权限 -->
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
    
    <!-- 通知权限 -->
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />

    <application
        android:label="@string/app_name"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:theme="@style/Theme.CameraFtpCompanion"
        android:extractNativeLibs="true"
        android:requestLegacyExternalStorage="true"
        android:preserveLegacyExternalStorage="true">
        
        <activity
            android:name=".MainActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
            android:exported="true"
            android:launchMode="singleTask">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
        
        <!-- Tauri plugin activity -->
        <activity
            android:name="app.tauri.activity.TauriActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
            android:exported="false" />
            
    </application>

</manifest>
```

**Step 2: 更新minSdkVersion**

找到并修改 `build.gradle` 或 `tauri.conf.json`:

```json
// src-tauri/tauri.conf.json
{
  "bundle": {
    "android": {
      "minSdkVersion": 30
    }
  }
}
```

**Step 3: 提交**

```bash
git add src-tauri/gen/android/app/src/main/AndroidManifest.xml src-tauri/tauri.conf.json
git commit -m "feat: update Android manifest for SAF and set minSdkVersion to 30"
```

---

## Phase 2: 前端实现 (React)

### Task 4: 创建useStoragePermission Hook

**Files:**
- Create: `src/hooks/useStoragePermission.ts`
- Test: 手动测试

**Step 1: 创建Hook文件**

```typescript
// src/hooks/useStoragePermission.ts

import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface StoragePathInfo {
  path_name: string;
  uri: string;
  raw_path?: string;
  is_valid: boolean;
}

export interface ServerStartCheckResult {
  can_start: boolean;
  reason?: string;
  current_path?: StoragePathInfo;
}

interface StoragePermissionState {
  pathInfo: StoragePathInfo | null;
  isLoading: boolean;
  isChecking: boolean;
  error: string | null;
}

export function useStoragePermission() {
  const [state, setState] = useState<StoragePermissionState>({
    pathInfo: null,
    isLoading: false,
    isChecking: false,
    error: null,
  });

  // 加载当前存储路径信息
  const loadStoragePath = useCallback(async () => {
    setState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const info = await invoke<StoragePathInfo | null>('get_storage_path');
      setState(prev => ({
        ...prev,
        pathInfo: info,
        isLoading: false,
      }));
      return info;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setState(prev => ({
        ...prev,
        isLoading: false,
        error: errorMsg,
      }));
      return null;
    }
  }, []);

  // 检查服务器启动前提条件
  const checkPrerequisites = useCallback(async (): Promise<ServerStartCheckResult> => {
    setState(prev => ({ ...prev, isChecking: true }));
    
    try {
      const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
      
      if (result.current_path) {
        setState(prev => ({
          ...prev,
          pathInfo: result.current_path || null,
          isChecking: false,
        }));
      }
      
      return result;
    } catch (err) {
      setState(prev => ({ ...prev, isChecking: false }));
      return {
        can_start: false,
        reason: err instanceof Error ? err.message : String(err),
      };
    }
  }, []);

  // 保存存储路径
  const saveStoragePath = useCallback(async (pathName: string, uri: string): Promise<boolean> => {
    try {
      await invoke('save_storage_path', { pathName, uri });
      
      // 更新本地状态
      await loadStoragePath();
      
      return true;
    } catch (err) {
      console.error('Failed to save storage path:', err);
      return false;
    }
  }, [loadStoragePath]);

  // 获取上次使用的URI（用于预选中）
  const getLastUri = useCallback(async (): Promise<string | null> => {
    try {
      const uri = await invoke<string | null>('get_last_storage_uri');
      return uri;
    } catch {
      return null;
    }
  }, []);

  // 初始化时加载
  useEffect(() => {
    loadStoragePath();
  }, [loadStoragePath]);

  return {
    ...state,
    loadStoragePath,
    checkPrerequisites,
    saveStoragePath,
    getLastUri,
  };
}
```

**Step 2: 创建Android SAF选择器Hook**

```typescript
// src/hooks/useSAFPicker.ts

import { useCallback } from 'react';

interface SAFPickerResult {
  uri: string;
  name: string;
}

export function useSAFPicker() {
  const openPicker = useCallback(async (initialUri?: string): Promise<SAFPickerResult | null> => {
    // 检查是否在Android环境
    const isAndroid = typeof navigator !== 'undefined' && 
      /Android/i.test(navigator.userAgent);
    
    if (!isAndroid) {
      console.warn('SAF picker is only available on Android');
      return null;
    }

    try {
      // 通过Tauri的Android插件调用SAF选择器
      // 这里需要实现一个Tauri命令来触发Android的Intent
      
      // 方案1: 使用Tauri的HTTP插件或自定义协议
      // 方案2: 使用Kotlin插件（需要额外实现）
      // 方案3: 使用现有的open_directory命令修改
      
      // 临时实现：调用现有的选择目录命令
      const result = await invoke<string | null>('select_save_directory');
      
      if (result) {
        return {
          uri: result,
          name: result.split('/').pop() || 'Selected Folder',
        };
      }
      
      return null;
    } catch (err) {
      console.error('Failed to open SAF picker:', err);
      return null;
    }
  }, []);

  return { openPicker };
}
```

**Step 3: 提交**

```bash
git add src/hooks/useStoragePermission.ts src/hooks/useSAFPicker.ts
git commit -m "feat: add useStoragePermission and useSAFPicker hooks"
```

---

### Task 5: 修改ServerCard组件

**Files:**
- Modify: `src/components/ServerCard.tsx`
- Test: 手动测试

**Step 1: 修改导入和Hook使用**

```typescript
// src/components/ServerCard.tsx

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner'; // 或你使用的toast库
import { useStoragePermission } from '../hooks/useStoragePermission';
import { useSAFPicker } from '../hooks/useSAFPicker';

export function ServerCard() {
  const [isStarting, setIsStarting] = useState(false);
  
  const { 
    pathInfo, 
    isChecking, 
    checkPrerequisites, 
    saveStoragePath,
    getLastUri,
  } = useStoragePermission();
  
  const { openPicker } = useSAFPicker();

  // 检查并启动服务器
  const handleStartServer = useCallback(async () => {
    if (isStarting || isChecking) return;
    
    setIsStarting(true);
    
    try {
      // 1. 检查前提条件
      const check = await checkPrerequisites();
      
      if (!check.can_start) {
        // 显示Toast提示
        const reason = check.reason || '需要配置存储路径';
        toast.info(`${reason}，请选择存储目录`);
        
        // 2. 获取上次使用的URI用于预选中
        const lastUri = await getLastUri();
        
        // 3. 打开SAF选择器
        const pickerResult = await openPicker(lastUri || undefined);
        
        if (!pickerResult) {
          // 用户取消
          toast.warning('未选择存储路径，服务器未启动');
          setIsStarting(false);
          return;
        }
        
        // 4. 保存选择的目录
        const saved = await saveStoragePath(pickerResult.name, pickerResult.uri);
        
        if (!saved) {
          toast.error('保存存储路径失败，请重试');
          setIsStarting(false);
          return;
        }
        
        // 5. 显示成功提示
        toast.success(`存储路径已设置为：${pickerResult.name}`);
      }
      
      // 6. 启动服务器
      await invoke('start_server');
      toast.success('FTP服务器已启动');
      
    } catch (err) {
      console.error('Failed to start server:', err);
      toast.error('启动服务器失败：' + (err instanceof Error ? err.message : String(err)));
    } finally {
      setIsStarting(false);
    }
  }, [isStarting, isChecking, checkPrerequisites, saveStoragePath, getLastUri, openPicker]);

  // ... rest of the component
}
```

**Step 2: 更新按钮状态显示**

```typescript
// 在按钮渲染中添加状态指示
<button
  onClick={handleStartServer}
  disabled={isStarting || isChecking}
  className="..."
>
  {isStarting ? '启动中...' : isChecking ? '检查中...' : pathInfo?.is_valid ? '启动服务器' : '选择路径并启动'}
</button>

// 显示当前路径状态
{pathInfo && (
  <div className="text-sm text-gray-600">
    存储路径：{pathInfo.path_name}
    {pathInfo.is_valid ? ' ✅' : ' ❌'}
  </div>
)}
```

**Step 3: 提交**

```bash
git add src/components/ServerCard.tsx
git commit -m "feat: integrate storage permission check into ServerCard"
```

---

### Task 6: 修改ConfigCard组件

**Files:**
- Modify: `src/components/ConfigCard.tsx`
- Test: 手动测试

**Step 1: 修改配置页面**

```typescript
// src/components/ConfigCard.tsx

import { useStoragePermission } from '../hooks/useStoragePermission';
import { useSAFPicker } from '../hooks/useSAFPicker';
import { toast } from 'sonner';

export function ConfigCard() {
  const { pathInfo, isLoading, loadStoragePath, saveStoragePath, getLastUri } = useStoragePermission();
  const { openPicker } = useSAFPicker();

  const handleChangePath = useCallback(async () => {
    // 直接打开SAF选择器，无弹窗
    const lastUri = await getLastUri();
    const result = await openPicker(lastUri || undefined);
    
    if (!result) {
      // 用户取消，静默返回
      return;
    }
    
    // 保存新路径
    const saved = await saveStoragePath(result.name, result.uri);
    
    if (saved) {
      toast.success(`存储路径已更新为：${result.name}`);
    } else {
      toast.error('更新存储路径失败');
    }
  }, [openPicker, saveStoragePath, getLastUri]);

  return (
    <div className="...">
      {/* 存储路径设置 */}
      <div className="...">
        <h3>存储设置</h3>
        
        <div className="...">
          <span>当前路径：</span>
          {isLoading ? (
            <span>加载中...</span>
          ) : pathInfo ? (
            <span>
              {pathInfo.path_name}
              {pathInfo.is_valid ? ' ✅' : ' ❌ 权限已失效'}
            </span>
          ) : (
            <span className="text-gray-400">未配置</span>
          )}
        </div>
        
        <button 
          onClick={handleChangePath}
          disabled={isLoading}
        >
          {pathInfo ? '更改存储路径' : '选择存储路径'}
        </button>
      </div>
      
      {/* ... 其他配置项 */}
    </div>
  );
}
```

**Step 2: 提交**

```bash
git add src/components/ConfigCard.tsx
git commit -m "feat: simplify ConfigCard storage path selection with direct SAF picker"
```

---

## Phase 3: Android原生实现

### Task 7: 实现SAF选择器Activity

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/SAFActivity.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: 创建SAF选择器Activity**

```kotlin
// src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/SAFActivity.kt

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity

class SAFActivity : AppCompatActivity() {
    
    private val openDocumentTree = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            // 持久化权限
            val takeFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            contentResolver.takePersistableUriPermission(it, takeFlags)
            
            // 返回结果给调用者
            val result = Intent().apply {
                data = it
            }
            setResult(Activity.RESULT_OK, result)
        } ?: run {
            setResult(Activity.RESULT_CANCELED)
        }
        finish()
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // 获取初始URI（用于预选中）
        val initialUri = intent.getStringExtra("initial_uri")?.let {
            Uri.parse(it)
        }
        
        // 打开SAF选择器
        openDocumentTree.launch(initialUri)
    }
    
    companion object {
        const val REQUEST_CODE = 1001
        
        fun createIntent(activity: Activity, initialUri: Uri? = null): Intent {
            return Intent(activity, SAFActivity::class.java).apply {
                initialUri?.let {
                    putExtra("initial_uri", it.toString())
                }
            }
        }
    }
}
```

**Step 2: 在MainActivity中集成**

```kotlin
// src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import app.tauri.plugin.PluginManager
import androidx.activity.result.contract.ActivityResultContracts

class MainActivity : TauriActivity() {
    private var pickerCallback: ((String?) -> Unit)? = null
    
    private val openDocumentTree = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        uri?.let {
            // 持久化权限
            val takeFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            contentResolver.takePersistableUriPermission(it, takeFlags)
            
            pickerCallback?.invoke(it.toString())
        } ?: run {
            pickerCallback?.invoke(null)
        }
        pickerCallback = null
    }
    
    fun openSAFPicker(initialUri: String?, callback: (String?) -> Unit) {
        pickerCallback = callback
        val uri = initialUri?.let { Uri.parse(it) }
        openDocumentTree.launch(uri)
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        PluginManager.init(this)
    }
}
```

**Step 3: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/
git commit -m "feat: implement SAF picker activity for Android"
```

---

### Task 8: 创建Tauri插件桥接

**Files:**
- Create: `src-tauri/src/commands/storage.rs` 或修改现有commands
- Test: 手动测试

**Step 1: 修改select_save_directory命令**

```rust
// src-tauri/src/commands.rs

use tauri::{AppHandle, Runtime};

/// 打开SAF选择器（Android）或系统选择器（桌面端）
#[tauri::command]
pub async fn select_save_directory(
    app: AppHandle,
    initial_uri: Option<String>,
) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        // 使用Tauri的Android插件机制调用MainActivity
        let result = app.run_on_android_context(move |env, activity| {
            let initial = initial_uri.as_ref()
                .map(|s| env.new_string(s).ok())
                .flatten();
            
            // 调用MainActivity的openSAFPicker方法
            // 这里需要使用Tauri的插件系统或JNI调用
            
            // 临时实现：返回None，实际需要通过回调机制
            Ok(None::<String>)
        }).map_err(|e| e.to_string())?;
        
        Ok(result)
    }
    
    #[cfg(not(target_os = "android"))]
    {
        // 桌面端使用Tauri的dialog插件
        use tauri_plugin_dialog::DialogExt;
        
        let folder = app.dialog().file().blocking_pick_folder();
        Ok(folder.map(|p| p.to_string_lossy().to_string()))
    }
}
```

**Step 2: 使用Tauri事件机制实现异步选择**

```rust
// 方案：使用Tauri事件系统

/// 请求打开SAF选择器
#[tauri::command]
pub fn request_saf_picker(app: AppHandle, initial_uri: Option<String>) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        
        let _ = app.emit("android-request-saf-picker", serde_json::json!({
            "initial_uri": initial_uri,
        }));
    }
}

/// 接收SAF选择结果（从Android端调用）
#[tauri::command]
pub fn on_saf_picker_result(
    app: AppHandle,
    uri: Option<String>,
) -> Result<(), String> {
    use tauri::Emitter;
    
    let _ = app.emit("saf-picker-result", serde_json::json!({
        "uri": uri,
    }));
    
    Ok(())
}
```

**Step 3: 前端监听选择结果**

```typescript
// src/hooks/useSAFPicker.ts (更新)

import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export function useSAFPicker() {
  const openPicker = useCallback(async (initialUri?: string): Promise<SAFPickerResult | null> => {
    return new Promise((resolve) => {
      // 监听选择结果
      const unlisten = listen<{ uri: string | null }>('saf-picker-result', (event) => {
        unlisten.then(f => f());
        
        if (event.payload.uri) {
          resolve({
            uri: event.payload.uri,
            name: extractPathName(event.payload.uri),
          });
        } else {
          resolve(null);
        }
      });
      
      // 请求打开选择器
      invoke('request_saf_picker', { initialUri });
      
      // 超时处理
      setTimeout(() => {
        unlisten.then(f => f());
        resolve(null);
      }, 60000); // 60秒超时
    });
  }, []);

  return { openPicker };
}

function extractPathName(uri: string): string {
  // 从content:// URI中提取路径名
  // content://com.android.externalstorage.documents/tree/primary:DCIM/Camera
  const match = uri.match(/:([^:]+)$/);
  return match ? match[1] : 'Selected Folder';
}
```

**Step 4: 提交**

```bash
git add src-tauri/src/commands.rs src/hooks/useSAFPicker.ts
git commit -m "feat: implement async SAF picker with Tauri event bridge"
```

---

## Phase 4: 集成与测试

### Task 9: 集成测试

**Step 1: 编译Android项目**

```bash
cd src-tauri
cargo tauri android build --debug
```

Expected: 编译成功，生成APK

**Step 2: 安装到设备**

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/debug/app-debug.apk
```

**Step 3: 手动测试清单**

- [ ] 首次打开APP，点击"启动服务器"
  - 预期：显示Toast提示，自动打开SAF选择器
- [ ] 在SAF选择器中选择DCIM目录
  - 预期：返回APP，显示"存储路径已设置"，自动启动服务器
- [ ] 进入设置页
  - 预期：显示当前路径DCIM
- [ ] 在设置页点击"更改存储路径"
  - 预期：直接打开SAF选择器（无弹窗）
- [ ] 选择Pictures目录
  - 预期：返回设置页，显示新路径Pictures
- [ ] 撤销存储权限（系统设置中）
- [ ] 返回APP，点击"启动服务器"
  - 预期：显示Toast提示权限失效，打开SAF选择器

**Step 4: 提交测试版本**

```bash
git add .
git commit -m "test: verify Android storage permission flow"
```

---

### Task 10: 修复问题并完善

根据测试结果修复问题，常见修复：

1. **权限持久化失败** - 检查takePersistableUriPermission调用
2. **URI解析错误** - 改进extractPathName函数
3. **选择器未预选中** - 检查EXTRA_INITIAL_URI传递
4. **Toast不显示** - 确保使用正确的toast库

**最终提交：**

```bash
git add .
git commit -m "fix: resolve issues from testing storage permission flow"
```

---

### Task 11: 更新文档

**Files:**
- Modify: `docs/plans/2025-02-21-android-storage-permission-redesign.md`

在文档中添加：
- 实现细节说明
- 测试步骤
- 已知限制

```bash
git add docs/plans/
git commit -m "docs: update implementation documentation"
```

---

## Phase 5: 完成

### Task 12: 最终检查与提交

**Step 1: 代码检查**

```bash
cd src-tauri
cargo clippy --all-targets --all-features 2>&1 | head -50
```

**Step 2: 最终提交**

```bash
git log --oneline -10
git status
```

确保：
- 所有文件已提交
- 提交信息符合规范
- 分支干净

**Step 3: 创建PR或合并**

```bash
git checkout main
git merge feature/android-storage-permission-redesign
git push
```

---

## 总结

### 修改的文件清单

**后端 (Rust):**
- `src-tauri/src/platform/android.rs` - SAF权限管理
- `src-tauri/src/storage_permission.rs` - 存储权限模块（新建）
- `src-tauri/src/config.rs` - 添加URI字段
- `src-tauri/src/lib.rs` - 注册新命令
- `src-tauri/src/commands.rs` - 修改选择目录命令
- `src-tauri/tauri.conf.json` - 更新minSdkVersion

**前端 (React/TypeScript):**
- `src/hooks/useStoragePermission.ts` - 权限管理Hook（新建）
- `src/hooks/useSAFPicker.ts` - SAF选择器Hook（新建）
- `src/components/ServerCard.tsx` - 集成权限检查
- `src/components/ConfigCard.tsx` - 简化路径选择

**Android原生:**
- `src-tauri/gen/android/app/src/main/AndroidManifest.xml` - 权限声明
- `src-tauri/gen/android/app/src/main/java/.../MainActivity.kt` - 集成SAF

### 关键实现点

1. **Toast + SAF**: 权限检查失败时显示Toast，直接打开SAF选择器
2. **配置同步**: ServerCard和ConfigCard共享useStoragePermission Hook
3. **预选中**: 使用EXTRA_INITIAL_URI预选中上次路径
4. **权限持久化**: 使用takePersistableUriPermission保持长期访问权限
5. **事件桥接**: 使用Tauri事件系统实现异步SAF选择

---

**计划完成时间**: 4-6小时（含测试）  
**最后更新**: 2025-02-21
