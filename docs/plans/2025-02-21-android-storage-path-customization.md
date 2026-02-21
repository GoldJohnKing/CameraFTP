# Android 存储路径自定义 - 实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 允许 Android 用户自定义 FTP 文件存储路径，默认保存到 /DCIM/CameraFTPCompanion，使用 SAF (Storage Access Framework) 实现。

**Architecture:** 使用 Tauri 的 Android 原生插件机制，通过 Kotlin 实现 SAF 目录选择器，持久化存储权限，并在 Rust 层将 Document URI 转换为可操作的文件路径。

**Tech Stack:** Tauri v2, Rust, Kotlin, Android SAF API

---

## Task 1: 更新 AndroidManifest.xml 添加存储权限

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`

**Step 1: 添加存储相关权限**

```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <uses-permission android:name="android.permission.INTERNET" />
    
    <!-- 添加存储权限 -->
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" 
        android:maxSdkVersion="32" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" 
        android:maxSdkVersion="29" />
    <uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE"
        tools:ignore="ScopedStorage" />

    <!-- AndroidTV support -->
    <uses-feature android:name="android.software.leanback" android:required="false" />

    <application
        android:icon="@mipmap/ic_launcher"
        android:label="@string/app_name"
        android:theme="@style/Theme.camera_ftp_companion"
        android:usesCleartextTraffic="${usesCleartextTraffic}"
        android:requestLegacyExternalStorage="true">
        <activity
            android:configChanges="orientation|keyboardHidden|keyboard|screenSize|locale|smallestScreenSize|screenLayout|uiMode"
            android:launchMode="singleTask"
            android:label="@string/main_activity_title"
            android:name=".MainActivity"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
                <category android:name="android.intent.category.LEANBACK_LAUNCHER" />
            </intent-filter>
        </activity>

        <provider
          android:name="androidx.core.content.FileProvider"
          android:authorities="${applicationId}.fileprovider"
          android:exported="false"
          android:grantUriPermissions="true">
          <meta-data
            android:name="android.support.FILE_PROVIDER_PATHS"
            android:resource="@xml/file_paths" />
        </provider>
    </application>
</manifest>
```

**Step 2: 验证修改**

检查 XML 格式正确，权限声明完整。

---

## Task 2: 创建 Android StorageHelper Kotlin 类

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt`

**Step 1: 实现 SAF 目录选择器和权限管理**

```kotlin
package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.documentfile.provider.DocumentFile
import java.io.File

/**
 * Android 存储辅助类
 * 封装 SAF (Storage Access Framework) 操作
 */
object StorageHelper {
    
    private const val PREF_NAME = "storage_prefs"
    private const val KEY_SAVED_URI = "saved_directory_uri"
    
    /**
     * 获取持久化的存储目录 URI
     */
    fun getPersistedDirectoryUri(context: Context): String? {
        val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        return prefs.getString(KEY_SAVED_URI, null)
    }
    
    /**
     * 保存目录 URI 并持久化权限
     */
    fun persistDirectoryUri(context: Context, uri: Uri): Boolean {
        return try {
            // 持久化权限
            val takeFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION or 
                           Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            context.contentResolver.takePersistableUriPermission(uri, takeFlags)
            
            // 保存 URI 到 SharedPreferences
            val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
            prefs.edit().putString(KEY_SAVED_URI, uri.toString()).apply()
            
            true
        } catch (e: Exception) {
            e.printStackTrace()
            false
        }
    }
    
    /**
     * 检查持久化权限是否仍然有效
     */
    fun checkPersistedPermission(context: Context, uriString: String?): Boolean {
        if (uriString == null) return false
        
        return try {
            val uri = Uri.parse(uriString)
            val persistedUris = context.contentResolver.persistedUriPermissions
            persistedUris.any { it.uri == uri && it.isWritePermission }
        } catch (e: Exception) {
            false
        }
    }
    
    /**
     * 清除保存的目录 URI
     */
    fun clearPersistedDirectory(context: Context) {
        val savedUri = getPersistedDirectoryUri(context)
        if (savedUri != null) {
            try {
                val uri = Uri.parse(savedUri)
                context.contentResolver.releasePersistableUriPermission(
                    uri, 
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
        
        val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        prefs.edit().remove(KEY_SAVED_URI).apply()
    }
    
    /**
     * 获取推荐存储路径（供 Rust 层调用）
     * 优先级：1. 持久化路径 2. /DCIM/CameraFTPCompanion 3. /Pictures/CameraFTPCompanion 4. 应用私有目录
     */
    fun getRecommendedStoragePath(context: Context): String {
        // 1. 检查是否有持久化且有效的权限
        val persistedUri = getPersistedDirectoryUri(context)
        if (persistedUri != null && checkPersistedPermission(context, persistedUri)) {
            return persistedUri
        }
        
        // 2. 尝试 DCIM 路径（传统方式，Android 10+ 可能受限）
        val dcimPath = File(android.os.Environment.getExternalStoragePublicDirectory(
            android.os.Environment.DIRECTORY_DCIM), "CameraFTPCompanion")
        if (dcimPath.exists() || dcimPath.mkdirs()) {
            return dcimPath.absolutePath
        }
        
        // 3. 尝试 Pictures 路径
        val picturesPath = File(android.os.Environment.getExternalStoragePublicDirectory(
            android.os.Environment.DIRECTORY_PICTURES), "CameraFTPCompanion")
        if (picturesPath.exists() || picturesPath.mkdirs()) {
            return picturesPath.absolutePath
        }
        
        // 4. 回退到应用私有目录
        return File(context.getExternalFilesDir(null), "ftp_uploads").absolutePath
    }
    
    /**
     * 通过 DocumentFile API 在 SAF 目录下创建文件
     * 用于 FTP 服务器保存文件时
     */
    fun createFileInDirectory(context: Context, directoryUri: String, fileName: String): DocumentFile? {
        return try {
            val uri = Uri.parse(directoryUri)
            val parentDoc = DocumentFile.fromTreeUri(context, uri)
            parentDoc?.createFile("application/octet-stream", fileName)
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }
    
    /**
     * 获取目录下的文件列表
     */
    fun listFilesInDirectory(context: Context, directoryUri: String): List<String> {
        return try {
            val uri = Uri.parse(directoryUri)
            val docFile = DocumentFile.fromTreeUri(context, uri)
            docFile?.listFiles()?.map { it.name ?: "unknown" } ?: emptyList()
        } catch (e: Exception) {
            emptyList()
        }
    }
    
    /**
     * 创建目录选择器的 Intent
     */
    fun createDirectoryPickerIntent(): Intent {
        return Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            // 添加标志以获得持久权限
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
            
            // 尝试设置初始目录为 DCIM
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val dcimUri = DocumentsContract.buildDocumentUri(
                    "com.android.externalstorage.documents",
                    "primary:DCIM"
                )
                putExtra(DocumentsContract.EXTRA_INITIAL_URI, dcimUri)
            }
        }
    }
}
```

**Step 2: 编译验证**

确保 Kotlin 文件编译通过，无语法错误。

---

## Task 3: 修改 MainActivity.kt 集成 SAF 选择器

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: 添加 Activity Result 回调**

```kotlin
package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import androidx.activity.result.ActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import app.tauri.plugin.JSObject
import com.gjk.cameraftpcompanion.generated.TauriActivity

class MainActivity : TauriActivity() {
    
    companion object {
        // 存储选择的回调，供 Rust 调用
        @JvmStatic
        var directoryPickerCallback: ((String?) -> Unit)? = null
        
        // 静态引用当前 Activity，供 Rust 启动选择器
        @JvmStatic
        var currentActivity: MainActivity? = null
    }
    
    // SAF 目录选择器回调
    private val directoryPickerLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result: ActivityResult ->
        if (result.resultCode == Activity.RESULT_OK) {
            val uri = result.data?.data
            if (uri != null) {
                // 持久化权限和 URI
                val success = StorageHelper.persistDirectoryUri(this, uri)
                if (success) {
                    directoryPickerCallback?.invoke(uri.toString())
                } else {
                    directoryPickerCallback?.invoke(null)
                }
            } else {
                directoryPickerCallback?.invoke(null)
            }
        } else {
            directoryPickerCallback?.invoke(null)
        }
    }
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        currentActivity = this
    }
    
    override fun onDestroy() {
        super.onDestroy()
        if (currentActivity == this) {
            currentActivity = null
        }
    }
    
    /**
     * 启动 SAF 目录选择器
     */
    fun openDirectoryPicker(callback: (String?) -> Unit) {
        directoryPickerCallback = callback
        val intent = StorageHelper.createDirectoryPickerIntent()
        directoryPickerLauncher.launch(intent)
    }
}
```

**Step 2: 验证集成**

确保 MainActivity 继承 TauriActivity，且代码能正常编译。

---

## Task 4: 修改 Rust Android 平台适配层

**Files:**
- Modify: `src-tauri/src/platform/android.rs`

**Step 1: 添加 SAF 相关 Rust 函数**

```rust
use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

/// 目录选择结果
#[derive(Debug, Clone, Serialize)]
pub struct DirectorySelection {
    pub uri: String,
    pub display_name: String,
}

/// 打开 SAF 目录选择器
#[cfg(target_os = "android")]
pub async fn open_directory_picker(app: &AppHandle) -> Option<String> {
    use std::sync::Arc;
    use tokio::sync::oneshot;
    
    let (tx, rx) = oneshot::channel::<Option<String>>();
    let tx = Arc::new(std::sync::Mutex::new(Some(tx)));
    
    // 通过 JNI 调用 Android 方法
    app.run_on_android_context(move |activity| {
        let activity = activity.as_obj();
        
        // 设置回调
        let tx_clone = tx.clone();
        set_directory_picker_callback(Box::new(move |uri: Option<String>| {
            if let Some(sender) = tx_clone.lock().unwrap().take() {
                let _ = sender.send(uri);
            }
        }));
        
        // 调用 openDirectoryPicker
        unsafe {
            let env = activity.get_env();
            let activity_class = env.find_class("com/gjk/cameraftpcompanion/MainActivity").ok()?;
            let method_id = env.get_method_id(
                &activity_class,
                "openDirectoryPicker",
                "()V"
            ).ok()?;
            
            env.call_method_unchecked(
                activity,
                method_id,
                jni::signature::JavaType::Primitive(jni::signature::Primitive::Void),
                &[]
            ).ok()?;
        }
        
        Some(())
    });
    
    rx.await.ok().flatten()
}

#[cfg(not(target_os = "android"))]
pub async fn open_directory_picker(_app: &AppHandle) -> Option<String> {
    None
}

/// 存储全局回调
static mut DIRECTORY_PICKER_CALLBACK: Option<Box<dyn Fn(Option<String>) + Send>> = None;

#[cfg(target_os = "android")]
fn set_directory_picker_callback<F: Fn(Option<String>) + Send + 'static>(callback: F) {
    unsafe {
        DIRECTORY_PICKER_CALLBACK = Some(Box::new(callback));
    }
}

/// 供 Android 调用回调
#[no_mangle]
pub extern "C" fn on_directory_selected(uri: *const c_char) {
    unsafe {
        let uri_str = if uri.is_null() {
            None
        } else {
            Some(CStr::from_ptr(uri).to_string_lossy().to_string())
        };
        
        if let Some(ref callback) = DIRECTORY_PICKER_CALLBACK {
            callback(uri_str);
        }
    }
}

/// 获取推荐存储路径（Android）
#[cfg(target_os = "android")]
pub fn get_recommended_storage_path(app: &AppHandle) -> String {
    use jni::objects::JString;
    use jni::signature::JavaType;
    
    let mut result = String::new();
    
    app.run_on_android_context(|activity| {
        let env = activity.get_env();
        
        // 调用 StorageHelper.getRecommendedStoragePath
        let helper_class = env.find_class("com/gjk/cameraftpcompanion/StorageHelper")?;
        let method_id = env.get_static_method_id(
            &helper_class,
            "getRecommendedStoragePath",
            "(Landroid/content/Context;)Ljava/lang/String;"
        ).ok()?;
        
        let j_string: JString = env.call_static_method_unchecked(
            &helper_class,
            method_id,
            JavaType::Object("java/lang/String".to_string()),
            &[activity.as_obj().into()]
        ).ok()?.l().ok()?.into();
        
        result = env.get_string(&j_string).ok()?.to_string_lossy().to_string();
        
        Some(())
    });
    
    if result.is_empty() {
        // 回退到应用私有目录
        app.path().app_data_dir()
            .map(|p| p.join("ftp_uploads").to_string_lossy().to_string())
            .unwrap_or_else(|_| "/sdcard/Android/data/com.gjk.cameraftpcompanion/files/ftp_uploads".to_string()
            )
    } else {
        result
    }
}

#[cfg(not(target_os = "android"))]
pub fn get_recommended_storage_path(_app: &AppHandle) -> String {
    String::new()
}

/// 检查持久化权限是否有效
#[cfg(target_os = "android")]
pub fn check_storage_permission(app: &AppHandle, uri: &str) -> bool {
    use jni::objects::JString;
    
    let mut result = false;
    
    app.run_on_android_context(|activity| {
        let env = activity.get_env();
        
        let helper_class = env.find_class("com/gjk/cameraftpcompanion/StorageHelper")?;
        let method_id = env.get_static_method_id(
            &helper_class,
            "checkPersistedPermission",
            "(Landroid/content/Context;Ljava/lang/String;)Z"
        ).ok()?;
        
        let uri_jstring = env.new_string(uri).ok()?;
        
        result = env.call_static_method_unchecked(
            &helper_class,
            method_id,
            JavaType::Primitive(jni::signature::Primitive::Boolean),
            &[activity.as_obj().into(), (&uri_jstring).into()]
        ).ok()?.z().ok()?;
        
        Some(())
    });
    
    result
}

#[cfg(not(target_os = "android"))]
pub fn check_storage_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}

use std::ffi::{c_char, CStr, CString};
```

**Step 2: 添加 JNI 依赖**

确保 Cargo.toml 包含 jni crate：

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
```

---

## Task 5: 修改 Rust Commands 层

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 添加 select_save_directory 命令**

```rust
/// 选择存储目录（Android 使用 SAF，桌面使用原生对话框）
#[tauri::command]
pub async fn select_save_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        // Android: 使用 SAF 目录选择器
        match crate::platform::android::open_directory_picker(&app).await {
            Some(uri) => {
                // 验证权限并返回
                if crate::platform::android::check_storage_permission(&app, &uri) {
                    Ok(Some(uri))
                } else {
                    Err("无法获取目录权限".to_string())
                }
            }
            None => Ok(None), // 用户取消
        }
    }
    
    #[cfg(not(target_os = "android"))]
    {
        // 桌面平台使用原生对话框
        use tauri_plugin_dialog::DialogExt;
        
        let folder_path = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择存储路径")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?;

        Ok(folder_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())))
    }
}

/// 验证存储路径是否可写
#[tauri::command]
pub async fn validate_save_path(app: tauri::AppHandle, path: String) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        // Android: 检查 SAF 权限
        if path.starts_with("content://") {
            Ok(crate::platform::android::check_storage_permission(&app, &path))
        } else {
            // 传统路径检查
            Ok(std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false))
        }
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let path_obj = std::path::PathBuf::from(&path);
        Ok(path_obj.exists() && path_obj.is_dir())
    }
}

/// 获取推荐的存储路径（Android）
#[tauri::command]
pub fn get_recommended_save_path(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        Ok(crate::platform::android::get_recommended_storage_path(&app))
    }
    
    #[cfg(not(target_os = "android"))]
    {
        use crate::config::AppConfig;
        let config = AppConfig::load();
        Ok(config.save_path.to_string_lossy().to_string())
    }
}
```

**Step 2: 更新命令注册（lib.rs）**

确保在 `lib.rs` 中注册新命令。

---

## Task 6: 修改 Config 模块支持 Android 自定义路径

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: 更新 Android 路径初始化逻辑**

```rust
/// 初始化 Android 路径（在应用启动时调用）
#[cfg(target_os = "android")]
pub fn init_android_paths(app_handle: &tauri::AppHandle) {
    use tauri::Manager;

    // 获取应用外部存储目录
    let external_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files"));

    let default_save_path = external_dir.join("ftp_uploads");
    let config_path = external_dir.join("config.json");

    // 尝试加载现有配置
    let mut config = AppConfig::load();
    
    // 检查是否有自定义存储路径（从 SAF 持久化）
    let recommended = crate::platform::android::get_recommended_storage_path(app_handle);
    
    // 如果配置中的路径是默认路径，且推荐路径不同，则更新
    if config.save_path == default_save_path || config.save_path.to_string_lossy().is_empty() {
        if !recommended.is_empty() && recommended != default_save_path.to_string_lossy() {
            config.save_path = PathBuf::from(&recommended);
            // 保存更新后的配置
            let _ = config.save();
        }
    }

    set_android_paths(config.save_path.clone(), config_path);
    info!("Android paths initialized: save={:?}, config={:?}", config.save_path, config_path);
}

/// 更新 Android 保存路径
#[cfg(target_os = "android")]
pub fn update_android_save_path(new_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // 更新全局路径
    set_android_paths(new_path.clone(), get_android_config_path());
    
    // 更新配置文件
    let mut config = AppConfig::load();
    config.save_path = new_path;
    config.save()?;
    
    info!("Android save path updated to: {:?}", config.save_path);
    Ok(())
}
```

---

## Task 7: 更新 lib.rs 注册新命令

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 在 generate_handler 中添加新命令**

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
    select_save_directory,        // 新增
    validate_save_path,           // 新增
    get_recommended_save_path,    // 新增
])
```

---

## Task 8: 添加 Cargo.toml JNI 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 添加 Android 特定依赖**

```toml
[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"
```

---

## Task 9: 创建前端存储路径设置界面

**Files:**
- Create: `src/components/StorageSettings.tsx`

**Step 1: 实现设置组件**

```typescript
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Folder, RefreshCw, AlertCircle, CheckCircle } from 'lucide-react';

interface StorageSettingsProps {
  platform: string;
}

export function StorageSettings({ platform }: StorageSettingsProps) {
  const [currentPath, setCurrentPath] = useState<string>('');
  const [isSelecting, setIsSelecting] = useState(false);
  const [permissionStatus, setPermissionStatus] = useState<'valid' | 'invalid' | 'checking'>('checking');

  useEffect(() => {
    loadCurrentPath();
  }, []);

  const loadCurrentPath = async () => {
    try {
      const config = await invoke<AppConfig>('load_config');
      setCurrentPath(config.save_path);
      checkPermission(config.save_path);
    } catch (e) {
      console.error('Failed to load config:', e);
    }
  };

  const checkPermission = async (path: string) => {
    setPermissionStatus('checking');
    try {
      const isValid = await invoke<boolean>('validate_save_path', { path });
      setPermissionStatus(isValid ? 'valid' : 'invalid');
    } catch (e) {
      setPermissionStatus('invalid');
    }
  };

  const handleSelectDirectory = async () => {
    setIsSelecting(true);
    try {
      const result = await invoke<string | null>('select_save_directory');
      if (result) {
        setCurrentPath(result);
        // 保存到配置
        const config = await invoke<AppConfig>('load_config');
        config.save_path = result;
        await invoke('save_config', { config });
        await checkPermission(result);
      }
    } catch (e) {
      console.error('Failed to select directory:', e);
      alert('选择目录失败: ' + e);
    } finally {
      setIsSelecting(false);
    }
  };

  const handleUseRecommended = async () => {
    try {
      const recommended = await invoke<string>('get_recommended_save_path');
      if (recommended) {
        setCurrentPath(recommended);
        const config = await invoke<AppConfig>('load_config');
        config.save_path = recommended;
        await invoke('save_config', { config });
        await checkPermission(recommended);
      }
    } catch (e) {
      console.error('Failed to get recommended path:', e);
    }
  };

  return (
    <div className="bg-white rounded-lg shadow-md p-4 mb-4">
      <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
        <Folder className="w-5 h-5" />
        存储路径设置
      </h3>
      
      <div className="space-y-4">
        {/* 当前路径显示 */}
        <div className="bg-gray-50 p-3 rounded-md">
          <label className="text-sm text-gray-600">当前存储路径</label>
          <div className="flex items-center gap-2 mt-1">
            <code className="text-sm bg-gray-100 px-2 py-1 rounded flex-1 break-all">
              {currentPath || '未设置'}
            </code>
            {permissionStatus === 'valid' && (
              <CheckCircle className="w-5 h-5 text-green-500" />
            )}
            {permissionStatus === 'invalid' && (
              <AlertCircle className="w-5 h-5 text-red-500" />
            )}
          </div>
          {permissionStatus === 'invalid' && (
            <p className="text-xs text-red-500 mt-1">
              权限已失效，请重新选择存储路径
            </p>
          )}
        </div>

        {/* 按钮组 */}
        <div className="flex gap-2">
          <button
            onClick={handleSelectDirectory}
            disabled={isSelecting}
            className="flex-1 bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700 
                       disabled:opacity-50 disabled:cursor-not-allowed flex items-center 
                       justify-center gap-2"
          >
            {isSelecting ? (
              <RefreshCw className="w-4 h-4 animate-spin" />
            ) : (
              <Folder className="w-4 h-4" />
            )}
            {isSelecting ? '选择中...' : '更改目录'}
          </button>
          
          {platform === 'android' && (
            <button
              onClick={handleUseRecommended}
              className="flex-1 bg-gray-200 text-gray-700 py-2 px-4 rounded-md 
                         hover:bg-gray-300 flex items-center justify-center gap-2"
            >
              <RefreshCw className="w-4 h-4" />
              使用推荐路径
            </button>
          )}
        </div>

        {/* Android 特有提示 */}
        {platform === 'android' && (
          <div className="text-xs text-gray-500 bg-blue-50 p-2 rounded">
            <p>提示：建议选择 DCIM/CameraFTPCompanion 或 Pictures/CameraFTPCompanion</p>
            <p>这样可以在相册中直接查看传输的照片</p>
          </div>
        )}
      </div>
    </div>
  );
}
```

---

## Task 10: 构建和测试

**Step 1: Android 构建**

```bash
cd src-tauri
cargo build --target aarch64-linux-android --release
```

**Step 2: 在 Android 设备/模拟器测试**

1. 安装 APK
2. 首次启动检查默认路径
3. 打开设置 → 更改存储目录
4. 选择 DCIM/CameraFTPCompanion
5. 上传照片验证保存位置

**Step 3: 验证场景**

- [ ] 首次启动使用推荐路径
- [ ] 手动选择新目录
- [ ] 权限被撤销后正确提示
- [ ] 目录不存在时自动创建
- [ ] FTP 上传文件到正确位置

---

## 实施完成检查清单

- [ ] AndroidManifest.xml 权限声明正确
- [ ] StorageHelper.kt 实现完整
- [ ] MainActivity.kt 集成 SAF 选择器
- [ ] Rust platform/android.rs JNI 调用正确
- [ ] commands.rs 新命令可用
- [ ] config.rs Android 路径初始化正确
- [ ] lib.rs 命令注册完整
- [ ] Cargo.toml JNI 依赖添加
- [ ] 前端 StorageSettings 组件可用
- [ ] Android 构建成功
- [ ] 功能测试通过

---

**执行方式选择：**

1. **子代理驱动（当前会话）** - 我逐个任务调度子代理，任务间审查，快速迭代
2. **并行会话（独立）** - 打开新会话执行，批量处理带检查点

请选择执行方式。
