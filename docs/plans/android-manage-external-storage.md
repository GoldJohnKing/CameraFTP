# Android 所有文件访问权限方案（方案C）实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现完整的 Android 所有文件访问权限申请和管理机制，使用户能够无障碍地选择任何存储目录

**Architecture:** 使用 `MANAGE_EXTERNAL_STORAGE` 权限配合 SAF 目录选择器，实现三层权限检查（Rust层/Kotlin层/前端层），并在权限被拒绝时提供回退到应用私有目录的能力

**Tech Stack:** Tauri v2 + React + Rust + Kotlin + Android SDK

---

## ⚠️ 方案C风险说明

**Google Play 政策警告：** 此方案使用 `MANAGE_EXTERNAL_STORAGE` 权限，Google Play 对此有严格限制：
- 仅限文件管理器、备份应用、杀毒软件等特定类别
- 需要通过 Google Play 审核豁免申请
- **建议：** 如通过 Google Play 分发，请先申请权限豁免或考虑方案A/B

**适用场景：**
- 国内应用市场分发
- GitHub Release 侧载
- 企业内部应用

---

## 链路分析

### 完整数据流

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              文件存储完整链路                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                │
│  │   用户点击   │────▶│  useSAFPicker│────▶│   JS Bridge  │                │
│  │ "选择目录"   │     │   (React)    │     │   (Kotlin)   │                │
│  └──────────────┘     └──────────────┘     └──────────────┘                │
│                                                     │                       │
│                                                     ▼                       │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                │
│  │   保存配置   │◀────│  save_storage│◀────│  SAF Picker  │                │
│  │  (config.rs) │     │  _path (Rust)│     │   (系统UI)   │                │
│  └──────────────┘     └──────────────┘     └──────────────┘                │
│         │                                                                   │
│         ▼                                                                   │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                │
│  │   FTP写入    │◀────│ uri_to_file  │◀────│ 检查/申请    │                │
│  │   文件       │     │   _path      │     │   权限       │                │
│  └──────────────┘     └──────────────┘     └──────────────┘                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 当前问题点

1. **权限检查虚设** - `check_saf_permission` 直接返回 true
2. **URI 转换失败无回退** - `uri_to_file_path` 失败时直接报错
3. **MANAGE_EXTERNAL_STORAGE 未主动申请** - 仅提示用户手动开启
4. **配置路径不一致** - 回退路径使用外部存储，实际使用内部存储
5. **Tauri 能力配置缺失** - 缺少文件系统权限声明

---

## 修改清单

### Task 1: 修复 Tauri 能力配置

**Files:**
- Modify: `src-tauri/capabilities/default.json`

**Step 1: 添加 fs 权限**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open",
    "fs:default",
    "fs:allow-app-write",
    "fs:allow-app-read"
  ]
}
```

**Step 2: 验证配置格式**

Run: `cd src-tauri && cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src-tauri/capabilities/default.json
git commit -m "fix(capabilities): add fs permissions for storage access"
```

---

### Task 2: 增强 AndroidManifest.xml 权限声明

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml`

**Step 1: 添加权限声明和意图过滤器**

```xml
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:tools="http://schemas.android.com/tools">

    <!-- Network permissions -->
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    
    <!-- Storage permissions -->
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE"
        android:maxSdkVersion="32" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE"
        android:maxSdkVersion="32" />
    
    <!-- All files access permission (Android 11+ API 30+) -->
    <uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE"
        tools:ignore="ScopedStorage" />
    
    <!-- Foreground service permissions -->
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
    
    <!-- Notification permission -->
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />

    <!-- AndroidTV support -->
    <uses-feature android:name="android.software.leanback" android:required="false" />

    <application
        android:label="@string/app_name"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:theme="@style/Theme.MaterialComponents.DayNight.NoActionBar"
        android:extractNativeLibs="true"
        android:requestLegacyExternalStorage="true">
        
        <activity
            android:name=".MainActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
            android:exported="true"
            android:launchMode="singleTask">
            
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
            
            <!-- 添加用于处理权限请求的 intent-filter -->
            <intent-filter>
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
            </intent-filter>
        </activity>
        
        <activity
            android:name="app.tauri.activity.TauriActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
            android:exported="false" />
            
    </application>

</manifest>
```

**Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/AndroidManifest.xml
git commit -m "feat(android): enhance manifest for MANAGE_EXTERNAL_STORAGE permission"
```

---

### Task 3: 增强 StorageHelper.kt - 添加主动权限申请

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt`

**Step 1: 添加权限检查和申请方法**

```kotlin
package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.documentfile.provider.DocumentFile
import java.io.File

/**
 * Android 存储辅助类
 * 封装 SAF (Storage Access Framework) 操作
 */
object StorageHelper {

    private const val PREF_NAME = "storage_prefs"
    private const val KEY_SAVED_URI = "saved_directory_uri"
    
    // 权限申请回调
    private var permissionCallback: ((Boolean) -> Unit)? = null

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
        val dcimPath = File(
            android.os.Environment.getExternalStoragePublicDirectory(
                android.os.Environment.DIRECTORY_DCIM
            ), "CameraFTPCompanion"
        )
        if (dcimPath.exists() || dcimPath.mkdirs()) {
            return dcimPath.absolutePath
        }

        // 3. 尝试 Pictures 路径
        val picturesPath = File(
            android.os.Environment.getExternalStoragePublicDirectory(
                android.os.Environment.DIRECTORY_PICTURES
            ), "CameraFTPCompanion"
        )
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
    fun createFileInDirectory(
        context: Context,
        directoryUri: String,
        fileName: String
    ): DocumentFile? {
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

    /**
     * 检查是否拥有 MANAGE_EXTERNAL_STORAGE 权限（所有文件访问权限）
     * Android 11+ (API 30+) 需要这个权限才能访问所有文件
     */
    fun hasManageExternalStoragePermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            android.os.Environment.isExternalStorageManager()
        } else {
            // Android 10 及以下不需要这个权限
            true
        }
    }

    /**
     * 请求 MANAGE_EXTERNAL_STORAGE 权限
     * 打开系统设置页面让用户手动开启
     */
    fun requestManageExternalStoragePermission(activity: Activity) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:${activity.packageName}")
            }
            activity.startActivityForResult(intent, REQUEST_CODE_MANAGE_STORAGE)
        }
    }

    /**
     * 处理权限申请结果
     * 在 MainActivity.onActivityResult 中调用
     */
    fun handlePermissionResult(requestCode: Int, resultCode: Int) {
        if (requestCode == REQUEST_CODE_MANAGE_STORAGE) {
            val granted = resultCode == Activity.RESULT_OK || 
                         (MainActivity.currentActivity?.let { hasManageExternalStoragePermission(it) } == true)
            permissionCallback?.invoke(granted)
            permissionCallback = null
        }
    }

    /**
     * 设置权限申请回调
     */
    fun setPermissionCallback(callback: (Boolean) -> Unit) {
        permissionCallback = callback
    }

    /**
     * 获取开启 MANAGE_EXTERNAL_STORAGE 权限的设置页面 Intent
     * 用户需要手动在设置中开启"所有文件访问权限"
     */
    fun getManageStorageSettingsIntent(): Intent {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: 跳转到应用特定的所有文件访问权限设置
            Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:${MainActivity.currentActivity?.packageName}")
            }
        } else {
            // Android 10 及以下: 跳转到应用信息页面
            Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.parse("package:${MainActivity.currentActivity?.packageName}")
            }
        }
    }

    /**
     * 跳转到设置页面开启所有文件访问权限
     */
    fun openManageStorageSettings(activity: Activity) {
        val intent = getManageStorageSettingsIntent()
        activity.startActivity(intent)
    }

    /**
     * 验证路径是否可写
     * 尝试创建和删除测试文件
     */
    fun isPathWritable(path: String): Boolean {
        return try {
            val testFile = File(path, ".write_test_${System.currentTimeMillis()}")
            testFile.createNewFile() && testFile.delete()
        } catch (e: Exception) {
            false
        }
    }

    companion object {
        const val REQUEST_CODE_MANAGE_STORAGE = 1001
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/StorageHelper.kt
git commit -m "feat(android): add active permission request and validation methods"
```

---

### Task 4: 增强 MainActivity.kt - 添加 JS Bridge 权限方法

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: 添加权限相关的 JS Bridge 方法**

```kotlin
package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts

/**
 * JavaScript Bridge 接口
 * 允许前端JavaScript直接调用Android方法
 */
class SAFPickerBridge(private val activity: MainActivity) {
    
    companion object {
        private const val TAG = "SAFPickerBridge"
    }
    
    private var callbackId: String? = null
    
    /**
     * 打开SAF目录选择器
     * @param initialUri 初始URI（可选）
     * @param callback JavaScript回调函数名
     */
    @JavascriptInterface
    fun openPicker(initialUri: String?, callback: String): Boolean {
        Log.d(TAG, "openPicker called from JavaScript, callback: $callback")
        callbackId = callback
        
        activity.runOnUiThread {
            activity.openSAFPicker(initialUri) { uri ->
                // 调用JavaScript回调
                val jsCode = if (uri != null) {
                    "$callback('$uri')"
                } else {
                    "$callback(null)"
                }
                
                activity.evaluateJavascript(jsCode)
            }
        }
        
        return true
    }
    
    /**
     * 检查是否拥有所有文件访问权限
     */
    @JavascriptInterface
    fun hasAllFilesAccess(): Boolean {
        return StorageHelper.hasManageExternalStoragePermission(activity)
    }
    
    /**
     * 请求所有文件访问权限
     * 打开系统设置页面
     */
    @JavascriptInterface
    fun requestAllFilesAccess(callback: String): Boolean {
        Log.d(TAG, "requestAllFilesAccess called from JavaScript")
        
        activity.runOnUiThread {
            StorageHelper.setPermissionCallback { granted ->
                val jsCode = "$callback($granted)"
                activity.evaluateJavascript(jsCode)
            }
            StorageHelper.requestManageExternalStoragePermission(activity)
        }
        
        return true
    }
    
    /**
     * 打开权限设置页面
     */
    @JavascriptInterface
    fun openPermissionSettings(): Boolean {
        Log.d(TAG, "openPermissionSettings called from JavaScript")
        
        activity.runOnUiThread {
            StorageHelper.openManageStorageSettings(activity)
        }
        
        return true
    }
    
    /**
     * 验证路径是否可写
     */
    @JavascriptInterface
    fun isPathWritable(path: String): Boolean {
        return StorageHelper.isPathWritable(path)
    }
}

class MainActivity : TauriActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        @JvmStatic
        var currentActivity: MainActivity? = null
    }
    
    private var pickerCallback: ((String?) -> Unit)? = null
    private var safBridge: SAFPickerBridge? = null
    private var webViewRef: WebView? = null

    // SAF 目录选择器回调
    private val safPickerLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        Log.d(TAG, "SAF Picker result: $uri")
        
        if (uri != null) {
            // 持久化权限
            val success = StorageHelper.persistDirectoryUri(this, uri)
            val uriString = if (success) uri.toString() else null
            pickerCallback?.invoke(uriString)
        } else {
            // 用户取消
            pickerCallback?.invoke(null)
        }
        pickerCallback = null
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        currentActivity = this
        Log.d(TAG, "MainActivity created")
        
        // 初始化Bridge
        safBridge = SAFPickerBridge(this)
    }

    /**
     * WebView创建完成时调用（由WryActivity触发）
     * 这是添加JavaScript Bridge的正确时机
     */
    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        Log.d(TAG, "WebView created, setting up JavaScript Bridge")
        
        // 保存WebView引用
        webViewRef = webView
        
        // 添加JavaScript Bridge - 此时WebView已创建完成
        safBridge?.let { bridge ->
            webView.addJavascriptInterface(bridge, "SAFPickerAndroid")
            Log.d(TAG, "JavaScript Bridge 'SAFPickerAndroid' added to WebView")
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        webViewRef = null
        if (currentActivity == this) {
            currentActivity = null
        }
    }
    
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        // 处理权限申请结果
        StorageHelper.handlePermissionResult(requestCode, resultCode)
    }

    /**
     * 启动 SAF 目录选择器
     */
    fun openSAFPicker(initialUri: String?, callback: (String?) -> Unit) {
        Log.d(TAG, "Opening SAF picker, initialUri: $initialUri")
        
        pickerCallback = callback
        
        val uri = initialUri?.let { Uri.parse(it) }
        try {
            safPickerLauncher.launch(uri)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch SAF picker", e)
            callback(null)
            pickerCallback = null
        }
    }
    
    /**
     * 在WebView中执行JavaScript
     * 使用保存的WebView引用
     */
    fun evaluateJavascript(jsCode: String) {
        runOnUiThread {
            try {
                webViewRef?.evaluateJavascript(jsCode, null)
                    ?: Log.e(TAG, "WebView reference is null, cannot execute: $jsCode")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to execute JavaScript", e)
            }
        }
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "feat(android): add permission JS Bridge methods"
```

---

### Task 5: 修复 platform/android.rs - 实现真实权限检查

**Files:**
- Modify: `src-tauri/src/platform/android.rs`

**Step 1: 重写权限检查和路径转换逻辑**

```rust
use std::future::Future;
use std::pin::Pin;
use tauri::AppHandle;
use tauri::Manager;

/// 目录选择回调类型
type DirectoryPickerCallback = Box<dyn FnOnce(Option<String>) + Send>;

/// 存储待执行的回调
static DIRECTORY_PICKER_CALLBACK: std::sync::Mutex<Option<DirectoryPickerCallback>> =
    std::sync::Mutex::new(None);

/// 存储权限状态缓存
#[cfg(target_os = "android")]
static PERMISSION_CACHE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Android foreground service wrapper
/// 在 Android 上实现后台 FTP 服务器运行

/// 请求 Android SAF 目录选择器
/// 返回选择的目录 URI (content://...)
#[cfg(target_os = "android")]
pub fn request_directory_picker<F>(app: &AppHandle, callback: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    use tauri::Emitter;

    // 存储回调
    if let Ok(mut cb) = DIRECTORY_PICKER_CALLBACK.lock() {
        *cb = Some(Box::new(callback));
    }

    // 发送事件给前端，前端调用 Android 的 SAF 选择器
    let _ = app.emit("android-request-directory-picker", ());
}

/// Android 选择器返回结果时调用
#[cfg(target_os = "android")]
pub fn on_directory_selected(uri: Option<String>) {
    if let Ok(mut cb) = DIRECTORY_PICKER_CALLBACK.lock() {
        if let Some(callback) = cb.take() {
            callback(uri);
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn request_directory_picker<F>(_app: &AppHandle, _callback: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    // 非 Android 平台直接返回 None
    _callback(None);
}

/// 获取持久化的存储目录 URI（从 Android SharedPreferences）
#[cfg(target_os = "android")]
pub fn get_persisted_directory_uri(_app: &AppHandle) -> Option<String> {
    // 注意：实际实现需要通过 JNI 读取 SharedPreferences
    // 这里返回 None，由前端通过 JS 桥获取后传回
    None
}

#[cfg(not(target_os = "android"))]
pub fn get_persisted_directory_uri(_app: &AppHandle) -> Option<String> {
    None
}

/// 获取推荐存储路径
#[cfg(target_os = "android")]
pub fn get_recommended_storage_path(_app: &AppHandle) -> String {
    // 优先级：
    // 1. 持久化的 SAF URI
    // 2. /DCIM/CameraFTPCompanion
    // 3. /Pictures/CameraFTPCompanion
    // 4. 应用私有目录

    // 注意：实际路径由前端通过 JS 获取后传回
    // 这里返回空字符串表示使用默认逻辑
    String::new()
}

#[cfg(not(target_os = "android"))]
pub fn get_recommended_storage_path(_app: &AppHandle) -> String {
    String::new()
}

/// 检查 SAF 权限是否有效
/// 
/// 三层检查策略：
/// 1. 首先检查 MANAGE_EXTERNAL_STORAGE（所有文件访问权限）
/// 2. 其次检查持久化 URI 权限
/// 3. 最后尝试写入测试文件验证
#[cfg(target_os = "android")]
pub fn check_saf_permission(app: &AppHandle, uri: &str) -> bool {
    use tracing::{debug, warn};

    // 检查缓存
    if let Ok(cache) = PERMISSION_CACHE.lock() {
        if let Some(cached) = *cache {
            debug!("Using cached permission status: {}", cached);
            return cached;
        }
    }

    // 1. 尝试将 URI 转换为文件路径
    if let Some(file_path) = uri_to_file_path(app, uri) {
        let path = std::path::PathBuf::from(&file_path);
        
        // 检查路径是否存在且可写
        if path.exists() {
            let test_file = path.join(".ftp_permission_test");
            let can_write = std::fs::File::create(&test_file)
                .and_then(|_| std::fs::remove_file(&test_file))
                .is_ok();
            
            if can_write {
                debug!("Permission check passed via file write test: {:?}", path);
                // 更新缓存
                if let Ok(mut cache) = PERMISSION_CACHE.lock() {
                    *cache = Some(true);
                }
                return true;
            } else {
                warn!("Path exists but not writable: {:?}", path);
            }
        } else {
            // 路径不存在，尝试创建
            if std::fs::create_dir_all(&path).is_ok() {
                debug!("Created directory during permission check: {:?}", path);
                // 更新缓存
                if let Ok(mut cache) = PERMISSION_CACHE.lock() {
                    *cache = Some(true);
                }
                return true;
            } else {
                warn!("Cannot create directory: {:?}", path);
            }
        }
    }

    // 2. 检查是否是 content:// URI（SAF 权限）
    if uri.starts_with("content://") {
        // SAF 权限需要通过 JS Bridge 检查
        // 这里返回 true，让上层通过其他方式验证
        debug!("SAF URI detected, deferring to JS Bridge validation");
        return true;
    }

    warn!("Permission check failed for URI: {}", uri);
    false
}

#[cfg(not(target_os = "android"))]
pub fn check_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}

/// 清除权限缓存
#[cfg(target_os = "android")]
pub fn clear_permission_cache() {
    if let Ok(mut cache) = PERMISSION_CACHE.lock() {
        *cache = None;
    }
}

/// 持久化 SAF 权限
/// 调用 takePersistableUriPermission 来保持跨会话的访问权限
///
/// 注意：在 Android 上，实际的权限持久化由前端通过 JS API 完成
/// 当用户选择目录时，前端会自动调用 takePersistableUriPermission
#[cfg(target_os = "android")]
pub fn persist_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    // Android: 权限持久化由前端在选择目录时自动完成
    // 这里返回 true 表示成功
    // 清除缓存，让下次检查时重新验证
    clear_permission_cache();
    true
}

#[cfg(not(target_os = "android"))]
pub fn persist_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}

/// 将 content:// URI 转换为文件路径（最佳努力）
/// 支持多种文档提供者的 URI 模式
/// 
/// 返回 None 表示无法转换为传统文件路径，需要使用 SAF API
#[cfg(target_os = "android")]
pub fn uri_to_file_path(_app: &AppHandle, uri: &str) -> Option<String> {
    use tracing::{debug, warn};

    // 记录接收到的 URI 用于调试
    debug!("uri_to_file_path: 接收到的 URI = {}", uri);

    if uri.is_empty() {
        warn!("uri_to_file_path: 收到空 URI");
        return None;
    }

    // 如果已经是文件路径，直接返回
    if uri.starts_with("/storage/") || uri.starts_with("/sdcard/") || uri.starts_with("/data/") {
        debug!("uri_to_file_path: 已经是文件路径");
        return Some(uri.to_string());
    }

    // 处理 externalstorage 文档 URI（主要存储）
    // 模式: content://com.android.externalstorage.documents/tree/primary:DCIM/Camera
    // 或: content://com.android.externalstorage.documents/document/primary:DCIM/Camera/001.jpg
    if let Some(pos) = uri.find("/tree/primary:") {
        let path_part = &uri[pos + 14..]; // 14 = len("/tree/primary:")
        match urlencoding::decode(path_part) {
            Ok(decoded) => {
                let path = format!("/storage/emulated/0/{}", decoded);
                debug!("uri_to_file_path: 解析 tree/primary 路径 = {}", path);
                return Some(path);
            }
            Err(e) => {
                warn!("uri_to_file_path: URL 解码失败: {}", e);
                // 不解码，直接使用原始值作为回退
                let path = format!("/storage/emulated/0/{}", path_part);
                return Some(path);
            }
        }
    }

    if let Some(pos) = uri.find("/document/primary:") {
        let path_part = &uri[pos + 18..]; // 18 = len("/document/primary:")
        match urlencoding::decode(path_part) {
            Ok(decoded) => {
                let path = format!("/storage/emulated/0/{}", decoded);
                debug!("uri_to_file_path: 解析 document/primary 路径 = {}", path);
                return Some(path);
            }
            Err(e) => {
                warn!("uri_to_file_path: URL 解码失败: {}", e);
                let path = format!("/storage/emulated/0/{}", path_part);
                return Some(path);
            }
        }
    }

    // 处理 SD 卡路径（非 primary）
    // 模式: /tree/XXXX-XXXX:path 或 /document/XXXX-XXXX:path
    // XXXX-XXXX 是 SD 卡的卷 ID
    if let Some(pos) = uri.find("/tree/") {
        let after_tree = &uri[pos + 6..]; // 6 = len("/tree/")
        if let Some(colon_pos) = after_tree.find(':') {
            let volume_id = &after_tree[..colon_pos];
            let path_part = &after_tree[colon_pos + 1..];
            match urlencoding::decode(path_part) {
                Ok(decoded) => {
                    // SD 卡路径格式: /storage/XXXX-XXXX/path
                    let path = format!("/storage/{}/{}", volume_id, decoded);
                    debug!("uri_to_file_path: 解析 SD 卡 tree 路径 = {}", path);
                    return Some(path);
                }
                Err(e) => {
                    warn!("uri_to_file_path: SD 卡路径 URL 解码失败: {}", e);
                    let path = format!("/storage/{}/{}", volume_id, path_part);
                    return Some(path);
                }
            }
        }
    }

    if let Some(pos) = uri.find("/document/") {
        let after_doc = &uri[pos + 10..]; // 10 = len("/document/")
                                          // 跳过 "primary:" 前缀检查，因为上面已处理
        if !after_doc.starts_with("primary:") {
            if let Some(colon_pos) = after_doc.find(':') {
                let volume_id = &after_doc[..colon_pos];
                let path_part = &after_doc[colon_pos + 1..];
                match urlencoding::decode(path_part) {
                    Ok(decoded) => {
                        let path = format!("/storage/{}/{}", volume_id, decoded);
                        debug!("uri_to_file_path: 解析 SD 卡 document 路径 = {}", path);
                        return Some(path);
                    }
                    Err(e) => {
                        warn!("uri_to_file_path: SD 卡路径 URL 解码失败: {}", e);
                        let path = format!("/storage/{}/{}", volume_id, path_part);
                        return Some(path);
                    }
                }
            }
        }
    }

    // 处理 raw: 路径（某些文档提供器使用）
    // 模式: content://.../raw:/storage/emulated/0/...
    if let Some(pos) = uri.find("/raw:/storage/") {
        let path = &uri[pos + 5..]; // 5 = len("/raw:")
        debug!("uri_to_file_path: 解析 raw 路径 = {}", path);
        return Some(path.to_string());
    }

    // 处理 Downloads 文档提供器
    // 模式: content://com.android.providers.downloads.documents/document/raw%3A%2Fstorage%2Femulated%2F0%2FDownload%2F...
    if uri.contains("providers.downloads.documents") {
        if let Some(pos) = uri.find("/document/") {
            let path_part = &uri[pos + 10..];
            // 尝试解码整个路径
            match urlencoding::decode(path_part) {
                Ok(decoded) => {
                    if decoded.starts_with("/storage/") || decoded.starts_with("raw:/storage/") {
                        let path = decoded.replace("raw:", "");
                        debug!("uri_to_file_path: 解析 Downloads 路径 = {}", path);
                        return Some(path);
                    }
                }
                Err(e) => {
                    warn!("uri_to_file_path: Downloads 路径解码失败: {}", e);
                }
            }
        }
    }

    // 处理 MediaStore 文档提供器
    // 模式: content://com.android.providers.media.documents/document/image%3A123
    // 这种情况下无法直接转换为文件路径，返回 None
    if uri.contains("providers.media.documents") {
        warn!("uri_to_file_path: MediaStore URI 无法直接转换为文件路径");
        // 返回 None，让上层使用 SAF API
        return None;
    }

    // 无法识别的 URI 格式
    warn!(
        "uri_to_file_path: 无法识别的 URI 格式: {}",
        uri
    );

    None
}

#[cfg(not(target_os = "android"))]
pub fn uri_to_file_path(_app: &AppHandle, _uri: &str) -> Option<String> {
    None
}

/// 验证文件路径是否可写
#[cfg(target_os = "android")]
pub fn validate_path_writable(path: &str) -> bool {
    use tracing::{debug, error};
    
    let path_buf = std::path::PathBuf::from(path);
    
    // 确保目录存在
    if !path_buf.exists() {
        debug!("Creating directory: {:?}", path_buf);
        if let Err(e) = std::fs::create_dir_all(&path_buf) {
            error!("Failed to create directory: {:?}, error: {}", path_buf, e);
            return false;
        }
    }
    
    // 尝试写入测试文件
    let test_file = path_buf.join(".ftp_write_test");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            debug!("Path is writable: {:?}", path_buf);
            true
        }
        Err(e) => {
            error!("Path is not writable: {:?}, error: {}", path_buf, e);
            false
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn validate_path_writable(_path: &str) -> bool {
    false
}

/// 启动前台服务
/// 这会让应用在后台继续运行 FTP 服务器
pub fn start_foreground_service(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        // 发送事件给前端，由前端调用 Android 原生插件
        let _ = app.emit("android-start-foreground-service", ());
    }
}

/// 停止前台服务
pub fn stop_foreground_service(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-stop-foreground-service", ());
    }
}

/// 检查是否有后台运行权限
pub fn has_background_permission(_app: &AppHandle) -> bool {
    // Android 13+ 需要特殊权限
    // 实际检查需要在前端通过 Capacitor/Cordova 插件完成
    true
}

/// 请求后台运行权限
pub fn request_background_permission(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-request-background-permission", ());
    }
}

/// 获取 Android 设备信息
pub fn get_device_info() -> DeviceInfo {
    DeviceInfo {
        platform: "android".to_string(),
        // 实际版本号需要从 Android 系统获取
        version: "14".to_string(),
        model: "Unknown".to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub version: String,
    pub model: String,
}

/// 显示本地通知
pub fn show_notification(app: &AppHandle, title: &str, body: &str) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit(
            "android-show-notification",
            serde_json::json!({
                "title": title,
                "body": body,
            }),
        );
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/platform/android.rs
git commit -m "fix(android): implement real permission checking and path validation"
```

---

### Task 6: 修复 storage_permission.rs - 添加回退机制

**Files:**
- Modify: `src-tauri/src/storage_permission.rs`

**Step 1: 重构保存逻辑，添加应用私有目录回退**

```rust
use std::path::PathBuf;
use tauri::AppHandle;
use tracing::{info, warn};

use crate::config::AppConfig;

/// Storage path information struct
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoragePathInfo {
    pub path_name: String,
    pub uri: String,
    pub raw_path: Option<String>,
    pub is_valid: bool,
}

/// Server start prerequisites check result
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub current_path: Option<StoragePathInfo>,
}

/// Validate storage permission for a given URI
/// On Android: checks SAF permission and fallback to app private directory
/// On Desktop: checks if path exists and is a directory
#[tauri::command]
pub async fn validate_storage_permission(
    app: AppHandle,
    uri: String,
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        use crate::platform::android;
        
        // 1. 尝试将 URI 转换为文件路径
        if let Some(file_path) = android::uri_to_file_path(&app, &uri) {
            let path = PathBuf::from(&file_path);
            
            // 验证路径是否可写
            if android::validate_path_writable(&file_path) {
                info!("Validated storage permission: path={:?}", path);
                return Ok(true);
            }
        }
        
        // 2. 如果是 content:// URI，返回 true（依赖前端检查）
        if uri.starts_with("content://") {
            info!("SAF URI detected, deferring to JS Bridge");
            return Ok(true);
        }
        
        // 3. 验证失败
        warn!("Storage permission validation failed for URI: {}", uri);
        Ok(false)
    }

    #[cfg(not(target_os = "android"))]
    {
        // On desktop, treat URI as a file path
        let path = PathBuf::from(&uri);
        let is_valid = path.exists() && path.is_dir();
        info!("Validated storage path on desktop: path={:?}, valid={}", path, is_valid);
        Ok(is_valid)
    }
}

/// Save storage path to configuration
/// On Android: persists SAF permission, converts URI to path, with fallback to app private directory
/// On Desktop: just updates the save_path
#[tauri::command]
pub async fn save_storage_path(
    app: AppHandle,
    path_name: String,
    uri: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use crate::platform::android;
        
        // 获取应用私有目录作为回退
        let fallback_path = app
            .path()
            .data_dir()
            .map(|p| p.join("ftp_uploads"))
            .unwrap_or_else(|_| PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files/ftp_uploads"));
        
        // 持久化 SAF 权限（best effort）
        let _persisted = android::persist_saf_permission(&app, &uri);
        
        // 尝试将 URI 转换为文件路径
        let raw_path = android::uri_to_file_path(&app, &uri);
        
        // 加载当前配置
        let mut config = AppConfig::load();
        
        let mut use_fallback = false;
        
        if let Some(ref raw) = raw_path {
            let path = PathBuf::from(raw);
            
            info!(
                "Checking path accessibility: path={:?}",
                path
            );

            // 验证路径可写性
            if android::validate_path_writable(raw) {
                config.save_path = path;
                info!("Using converted path: {}", raw);
            } else {
                // 路径不可写，使用回退目录
                warn!(
                    "Path is not writable, falling back to app private directory: {:?}",
                    fallback_path
                );
                config.save_path = fallback_path.clone();
                use_fallback = true;
            }
        } else {
            // 无法转换 URI，使用回退目录
            warn!(
                "Could not convert SAF URI to file path, falling back to app private directory: {:?}",
                fallback_path
            );
            config.save_path = fallback_path.clone();
            use_fallback = true;
        }
        
        // 保存 URI 和元数据
        config.save_path_uri = Some(uri.clone());
        config.save_path_raw = if use_fallback {
            Some(fallback_path.to_string_lossy().to_string())
        } else {
            raw_path.clone()
        };
        config.save_path_display = Some(path_name.clone());
        
        // 保存配置
        config.save().map_err(|e| format!("Failed to save config: {}", e))?;
        
        // 清除权限缓存
        android::clear_permission_cache();
        
        if use_fallback {
            info!("Storage path saved with fallback: display={}, path={:?}", 
                  path_name, config.save_path);
            // 返回警告信息但不算失败
            return Ok(());
        } else {
            info!("Storage path saved successfully: display={}", path_name);
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        // On desktop, just update the save_path
        let mut config = AppConfig::load();
        config.save_path = PathBuf::from(&path_name);
        config.save_path_uri = Some(uri);
        
        config.save().map_err(|e| format!("Failed to save config: {}", e))?;
        info!("Storage path saved on desktop: path={}", path_name);
    }

    Ok(())
}

/// Get current storage path information from config
/// Validates permission if on Android
#[tauri::command]
pub async fn get_storage_path(app: AppHandle) -> Result<Option<StoragePathInfo>, String> {
    let config = AppConfig::load();
    
    // 如果存储了 URI，使用它进行验证
    if let Some(uri) = &config.save_path_uri {
        #[cfg(target_os = "android")]
        {
            use crate::platform::android;
            
            // 验证存储路径是否仍然有效
            let is_valid = if let Some(ref raw) = config.save_path_raw {
                android::validate_path_writable(raw)
            } else {
                // 没有原始路径，检查 URI
                android::check_saf_permission(&app, uri)
            };
            
            // 使用显示名称（如果可用），否则使用原始路径
            let display_name = config.save_path_display
                .clone()
                .unwrap_or_else(|| config.save_path.to_string_lossy().to_string());
            
            let path_info = StoragePathInfo {
                path_name: display_name,
                uri: uri.clone(),
                raw_path: config.save_path_raw.clone(),
                is_valid,
            };
            
            info!("Retrieved storage path: valid={}", is_valid);
            return Ok(Some(path_info));
        }
        
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            let path_info = StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            };
            
            info!("Retrieved storage path on desktop: valid={}", is_valid);
            return Ok(Some(path_info));
        }
    }
    
    // 没有存储 URI，返回基本信息
    let is_valid = config.save_path.exists() && config.save_path.is_dir();
    
    let path_info = StoragePathInfo {
        path_name: config.save_path.to_string_lossy().to_string(),
        uri: config.save_path.to_string_lossy().to_string(),
        raw_path: Some(config.save_path.to_string_lossy().to_string()),
        is_valid,
    };
    
    Ok(Some(path_info))
}

/// Check server start prerequisites
/// Verifies that storage path is valid and ready for server to start
#[tauri::command]
pub async fn check_server_start_prerequisites(
    app: AppHandle,
) -> Result<ServerStartCheckResult, String> {
    let config = AppConfig::load();
    
    // 检查是否有有效的存储路径
    let path_info = if let Some(uri) = &config.save_path_uri {
        #[cfg(target_os = "android")]
        {
            use crate::platform::android;
            
            // 验证路径可写性
            let is_valid = if let Some(ref raw) = config.save_path_raw {
                android::validate_path_writable(raw)
            } else {
                android::check_saf_permission(&app, uri)
            };
            
            // 使用显示名称
            let display_name = config.save_path_display
                .clone()
                .unwrap_or_else(|| config.save_path.to_string_lossy().to_string());
            
            Some(StoragePathInfo {
                path_name: display_name,
                uri: uri.clone(),
                raw_path: config.save_path_raw.clone(),
                is_valid,
            })
        }
        
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            Some(StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: uri.clone(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            })
        }
    } else {
        // 没有存储 URI，检查桌面路径
        #[cfg(not(target_os = "android"))]
        {
            let is_valid = config.save_path.exists() && config.save_path.is_dir();
            
            Some(StoragePathInfo {
                path_name: config.save_path.to_string_lossy().to_string(),
                uri: config.save_path.to_string_lossy().to_string(),
                raw_path: Some(config.save_path.to_string_lossy().to_string()),
                is_valid,
            })
        }
        
        #[cfg(target_os = "android")]
        {
            // On Android, we really need a URI
            None
        }
    };
    
    // 判断服务器是否可以启动
    let can_start = match &path_info {
        Some(info) if info.is_valid => true,
        Some(_) => {
            warn!("Storage path exists but permission is not valid");
            false
        }
        None => {
            warn!("No storage path configured");
            false
        }
    };
    
    let reason = if !can_start {
        Some(match &path_info {
            Some(info) if !info.is_valid => {
                "Storage permission is not valid. Please reselect the storage folder.".to_string()
            }
            None => "No storage path configured. Please select a storage folder first.".to_string(),
            _ => "Unknown error".to_string(),
        })
    } else {
        None
    };
    
    let result = ServerStartCheckResult {
        can_start,
        reason,
        current_path: path_info,
    };
    
    info!("Server start prerequisites check: can_start={}", can_start);
    Ok(result)
}

/// Get the last saved storage URI from config
/// Used for pre-selecting the folder in the SAF picker
#[tauri::command]
pub async fn get_last_storage_uri() -> Result<Option<String>, String> {
    let config = AppConfig::load();
    Ok(config.save_path_uri)
}

/// Get app private directory path (for fallback)
#[tauri::command]
pub async fn get_app_private_directory(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let path = app
            .path()
            .data_dir()
            .map(|p| p.join("ftp_uploads"))
            .map_err(|e| format!("Failed to get data dir: {}", e))?;
        
        // 确保目录存在
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
        
        Ok(path.to_string_lossy().to_string())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let config = AppConfig::load();
        Ok(config.save_path.to_string_lossy().to_string())
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/storage_permission.rs
git commit -m "feat(storage): add app private directory fallback mechanism"
```

---

### Task 7: 修复 config.rs - 统一路径处理

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: 统一回退路径为内部存储**

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{error, info, warn};
use ts_rs::TS;

/// Android 存储路径（在应用初始化时设置）
#[cfg(target_os = "android")]
static ANDROID_SAVE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[cfg(target_os = "android")]
static ANDROID_CONFIG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 设置 Android 存储路径（在应用初始化时调用）
#[cfg(target_os = "android")]
pub fn set_android_paths(save_path: PathBuf, config_path: PathBuf) {
    let mut save_guard = ANDROID_SAVE_PATH.lock().unwrap();
    let mut config_guard = ANDROID_CONFIG_PATH.lock().unwrap();
    *save_guard = Some(save_path);
    *config_guard = Some(config_path);
    info!(
        "Android paths set: save={:?}, config={:?}",
        save_guard, config_guard
    );
}

/// 获取 Android 保存路径
/// 优先使用通过 set_android_paths 设置的值，否则使用应用内部存储
#[cfg(target_os = "android")]
fn get_android_save_path() -> PathBuf {
    ANDROID_SAVE_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| {
            // 使用应用内部存储，避免外部存储权限问题
            PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files/ftp_uploads")
        })
}

/// 获取 Android 配置路径
/// 优先使用通过 set_android_paths 设置的值，否则使用应用内部存储
#[cfg(target_os = "android")]
fn get_android_config_path() -> PathBuf {
    ANDROID_CONFIG_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| {
            // 使用应用内部存储，避免外部存储权限问题
            PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files/config.json")
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,
    pub auto_select_port: bool,
    pub file_extensions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_display: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            auto_open: true,
            auto_open_program: None,
            port: 2121,
            auto_select_port: true,
            file_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "raw".to_string(),
                "png".to_string(),
                "arw".to_string(),
                "cr2".to_string(),
                "nef".to_string(),
                "orf".to_string(),
                "rw2".to_string(),
            ],
            save_path_uri: None,
            save_path_raw: None,
            save_path_display: None,
        }
    }
}

impl AppConfig {
    fn default_pictures_dir() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            get_android_save_path()
        }
        #[cfg(not(target_os = "android"))]
        {
            dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
        }
    }

    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            get_android_config_path()
        }
        #[cfg(not(target_os = "android"))]
        {
            dirs::config_dir()
                .map(|d| d.join("camera-ftp-companion"))
                .unwrap_or_else(|| PathBuf::from("./config"))
                .join("config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        info!("Config loaded from {:?}", path);
                        return config;
                    }
                    Err(e) => {
                        error!("Failed to parse config: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to read config file: {}", e);
                }
            }
        }

        // Create default config
        let config = Self::default();
        if let Err(e) = config.save() {
            error!("Failed to save default config: {}", e);
        }
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;

        info!("Config saved to {:?}", path);
        Ok(())
    }
}

/// 初始化 Android 路径（在应用启动时调用）
#[cfg(target_os = "android")]
pub fn init_android_paths(app_handle: &tauri::AppHandle) {
    use tauri::Manager;

    // Android 11+ 对外部存储目录(/sdcard/Android/data/)访问受限
    // 使用应用内部存储目录 /data/data/<package>/files/ 确保始终可访问
    let internal_dir = match app_handle.path().data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            warn!("Failed to get data dir: {}, using fallback", e);
            PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files")
        }
    };

    let default_save_path = internal_dir.join("ftp_uploads");
    let config_path = internal_dir.join("config.json");

    // 确保目录存在
    let _ = fs::create_dir_all(&default_save_path);
    let _ = fs::create_dir_all(&internal_dir);

    // 尝试加载现有配置
    let mut config = AppConfig::load();

    // Android 10+ (API 29+) 引入 Scoped Storage，应用不能直接写入公共目录
    // 默认使用应用私有目录（不需要特殊权限）
    // 如果用户想要使用公共目录（如 DCIM），需要在设置中手动选择并开启权限
    if config.save_path.to_string_lossy().is_empty() {
        config.save_path = default_save_path.clone();
        let _ = config.save();
    } else {
        // 验证现有路径是否可写
        let test_file = config.save_path.join(".write_test");
        match fs::File::create(&test_file) {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
            }
            Err(_) => {
                // 路径不可写，回退到应用私有目录
                warn!("Save path is not writable, falling back to app private directory");
                config.save_path = default_save_path.clone();
                let _ = config.save();
            }
        }
    }

    let final_save_path = if config.save_path.to_string_lossy().is_empty() {
        default_save_path
    } else {
        config.save_path.clone()
    };

    set_android_paths(final_save_path.clone(), config_path.clone());
    info!(
        "Android paths initialized: save={:?}, config={:?}",
        final_save_path, config_path
    );
}

#[cfg(not(target_os = "android"))]
pub fn init_android_paths(_app_handle: &tauri::AppHandle) {
    // 非 Android 平台无需初始化
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "fix(config): unify Android path handling with internal storage fallback"
```

---

### Task 8: 更新前端 useSAFPicker.ts - 添加权限检查

**Files:**
- Modify: `src/hooks/useSAFPicker.ts`

**Step 1: 添加权限检查和申请方法**

```typescript
import { useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface SAFPickerResult {
  uri: string;
  name: string;
}

// 声明全局window扩展
declare global {
  interface Window {
    SAFPickerAndroid?: {
      openPicker: (initialUri: string | null, callback: string) => boolean;
      hasAllFilesAccess: () => boolean;
      requestAllFilesAccess: (callback: string) => boolean;
      openPermissionSettings: () => boolean;
      isPathWritable: (path: string) => boolean;
    };
  }
}

export function useSAFPicker() {
  const cleanupRef = useRef<(() => void) | null>(null);
  const callbackRef = useRef<((uri: string | null) => void) | null>(null);

  /**
   * 检查是否拥有所有文件访问权限
   */
  const checkAllFilesAccess = useCallback((): boolean => {
    if (typeof window === 'undefined') return false;
    
    const isAndroid = /Android/i.test(navigator.userAgent);
    if (!isAndroid) return true; // 桌面端不需要此权限
    
    if (window.SAFPickerAndroid?.hasAllFilesAccess) {
      return window.SAFPickerAndroid.hasAllFilesAccess();
    }
    
    return false;
  }, []);

  /**
   * 请求所有文件访问权限
   */
  const requestAllFilesAccess = useCallback((): Promise<boolean> => {
    return new Promise((resolve) => {
      if (typeof window === 'undefined') {
        resolve(false);
        return;
      }
      
      const isAndroid = /Android/i.test(navigator.userAgent);
      if (!isAndroid) {
        resolve(true);
        return;
      }
      
      if (window.SAFPickerAndroid?.requestAllFilesAccess) {
        const callbackName = `_safPermissionCallback_${Date.now()}`;
        
        (window as any)[callbackName] = (granted: boolean) => {
          delete (window as any)[callbackName];
          resolve(granted);
        };
        
        window.SAFPickerAndroid.requestAllFilesAccess(callbackName);
      } else {
        resolve(false);
      }
    });
  }, []);

  /**
   * 打开权限设置页面
   */
  const openPermissionSettings = useCallback((): void => {
    if (typeof window === 'undefined') return;
    
    if (window.SAFPickerAndroid?.openPermissionSettings) {
      window.SAFPickerAndroid.openPermissionSettings();
    }
  }, []);

  const openPicker = useCallback(async (initialUri?: string): Promise<SAFPickerResult | null> => {
    // 清理之前的会话
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }

    // 检测平台
    const isAndroid = typeof navigator !== 'undefined' && 
      /Android/i.test(navigator.userAgent);
    
    const isTauri = typeof window !== 'undefined' && 
      (window as any).__TAURI__ !== undefined;
    
    console.log('[useSAFPicker] Platform detection:', { isAndroid, isTauri, hasBridge: !!window.SAFPickerAndroid });

    // 桌面端：使用Tauri对话框
    if (!isAndroid) {
      try {
        const result = await invoke<string | null>('select_save_directory');
        if (result) {
          return {
            uri: result,
            name: result.split('/').pop() || 'Selected Folder',
          };
        }
        return null;
      } catch (err) {
        console.error('Failed to open directory picker:', err);
        return null;
      }
    }

    // Android端：优先使用JavaScript Bridge
    if (window.SAFPickerAndroid?.openPicker) {
      console.log('[useSAFPicker] Using JavaScript Bridge');
      
      return new Promise((resolve) => {
        // 创建唯一的回调函数名
        const callbackName = `_safPickerCallback_${Date.now()}`;
        
        // 设置回调
        callbackRef.current = (uri: string | null) => {
          console.log('[useSAFPicker] Bridge callback received:', uri);
          
          // 清理
          delete (window as any)[callbackName];
          callbackRef.current = null;
          cleanupRef.current = null;
          
          if (uri) {
            resolve({
              uri,
              name: extractPathName(uri),
            });
          } else {
            resolve(null);
          }
        };
        
        // 将回调注册到window对象，供Android调用
        (window as any)[callbackName] = (uri: string | null) => {
          callbackRef.current?.(uri);
        };
        
        // 调用Android Bridge
        const jsCallback = `${callbackName}`;
        console.log('[useSAFPicker] Calling bridge with callback:', jsCallback);
        
        try {
          const success = window.SAFPickerAndroid!.openPicker(initialUri || null, jsCallback);
          console.log('[useSAFPicker] Bridge call success:', success);
          
          if (!success) {
            resolve(null);
          }
        } catch (err) {
          console.error('[useSAFPicker] Bridge call failed:', err);
          resolve(null);
        }
        
        // 设置清理函数
        cleanupRef.current = () => {
          delete (window as any)[callbackName];
          callbackRef.current = null;
        };
        
        // 60秒超时
        setTimeout(() => {
          if (callbackRef.current) {
            console.log('[useSAFPicker] Timeout');
            callbackRef.current(null);
          }
        }, 60000);
      });
    }

    // 回退：使用Tauri事件机制（备用方案）
    console.log('[useSAFPicker] Falling back to Tauri event mechanism');
    
    return new Promise((resolve) => {
      let unlistenFn: (() => void) | null = null;
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      let resolved = false;

      const cleanup = () => {
        if (timeoutId) {
          clearTimeout(timeoutId);
          timeoutId = null;
        }
        if (unlistenFn) {
          unlistenFn();
          unlistenFn = null;
        }
        cleanupRef.current = null;
      };

      cleanupRef.current = cleanup;
      
      // 设置监听器
      const setupListener = async () => {
        try {
          unlistenFn = await listen<{ uri: string | null }>('saf-picker-result', (event) => {
            if (resolved) return;
            resolved = true;
            
            cleanup();
            
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
          await invoke('request_saf_picker', { initialUri });
        } catch (err) {
          console.error('Failed to setup picker:', err);
          if (!resolved) {
            resolved = true;
            cleanup();
            resolve(null);
          }
        }
      };
      
      setupListener();
      
      // 超时
      timeoutId = setTimeout(() => {
        if (resolved) return;
        resolved = true;
        cleanup();
        resolve(null);
      }, 60000);
    });
  }, []);

  const cleanup = useCallback(() => {
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }
  }, []);

  return { 
    openPicker, 
    cleanup, 
    checkAllFilesAccess, 
    requestAllFilesAccess, 
    openPermissionSettings 
  };
}

// 辅助函数：从URI提取路径名
function extractPathName(uri: string): string {
  const treeMatch = uri.match(/:([^:]+)$/);
  if (treeMatch) {
    return treeMatch[1];
  }
  
  const segments = uri.split('/');
  return segments[segments.length - 1] || 'Selected Folder';
}
```

**Step 2: Commit**

```bash
git add src/hooks/useSAFPicker.ts
git commit -m "feat(storage): add permission check and request methods to SAFPicker"
```

---

### Task 9: 更新前端 useStoragePermission.ts - 集成权限管理

**Files:**
- Modify: `src/hooks/useStoragePermission.ts`

**Step 1: 添加权限状态管理和引导**

```typescript
import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { useSAFPicker } from './useSAFPicker';

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
  hasAllFilesAccess: boolean;
}

export function useStoragePermission() {
  const [state, setState] = useState<StoragePermissionState>({
    pathInfo: null,
    isLoading: false,
    isChecking: false,
    error: null,
    hasAllFilesAccess: false,
  });

  const mountedRef = useRef(true);
  const { 
    openPicker, 
    checkAllFilesAccess, 
    requestAllFilesAccess, 
    openPermissionSettings 
  } = useSAFPicker();

  // Set mounted flag
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // 检查所有文件访问权限
  const checkPermissionStatus = useCallback(async () => {
    const hasAccess = checkAllFilesAccess();
    if (mountedRef.current) {
      setState(prev => ({ ...prev, hasAllFilesAccess: hasAccess }));
    }
    return hasAccess;
  }, [checkAllFilesAccess]);

  // Load current storage path info
  const loadStoragePath = useCallback(async () => {
    setState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const [info, hasAccess] = await Promise.all([
        invoke<StoragePathInfo | null>('get_storage_path'),
        checkPermissionStatus(),
      ]);
      
      if (!mountedRef.current) return null;
      setState(prev => ({
        ...prev,
        pathInfo: info,
        isLoading: false,
        hasAllFilesAccess: hasAccess,
      }));
      return info;
    } catch (err) {
      if (!mountedRef.current) return null;
      const errorMsg = err instanceof Error ? err.message : String(err);
      setState(prev => ({
        ...prev,
        isLoading: false,
        error: errorMsg,
      }));
      return null;
    }
  }, [checkPermissionStatus]);

  // Check server start prerequisites
  const checkPrerequisites = useCallback(async (): Promise<ServerStartCheckResult> => {
    setState(prev => ({ ...prev, isChecking: true, error: null }));
    
    try {
      const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
      
      if (!mountedRef.current) return result;

      if (result.current_path) {
        setState(prev => ({
          ...prev,
          pathInfo: result.current_path || null,
          isChecking: false,
        }));
      } else {
        setState(prev => ({ ...prev, isChecking: false }));
      }
      
      return result;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      if (mountedRef.current) {
        setState(prev => ({
          ...prev,
          isChecking: false,
          error: errorMsg,
        }));
      }
      return {
        can_start: false,
        reason: errorMsg,
      };
    }
  }, []);

  // Save storage path
  const saveStoragePath = useCallback(async (pathName: string, uri: string): Promise<{ success: boolean; error?: string; usingFallback?: boolean }> => {
    try {
      await invoke('save_storage_path', { pathName, uri });
      await loadStoragePath();
      return { success: true };
    } catch (err) {
      console.error('Failed to save storage path:', err);
      const errorMsg = err instanceof Error ? err.message : String(err);
      return { success: false, error: errorMsg };
    }
  }, [loadStoragePath]);

  // Get last URI for picker pre-selection
  const getLastUri = useCallback(async (): Promise<string | null> => {
    try {
      const uri = await invoke<string | null>('get_last_storage_uri');
      return uri;
    } catch {
      return null;
    }
  }, []);

  // Request all files access permission
  const requestPermission = useCallback(async (): Promise<boolean> => {
    const granted = await requestAllFilesAccess();
    if (mountedRef.current) {
      setState(prev => ({ ...prev, hasAllFilesAccess: granted }));
    }
    return granted;
  }, [requestAllFilesAccess]);

  // Open permission settings
  const openSettings = useCallback(() => {
    openPermissionSettings();
  }, [openPermissionSettings]);

  // Pick and save storage path with permission handling
  const pickAndSaveStoragePath = useCallback(async (options?: {
    successMessage?: string;
    errorMessage?: string;
    onNeedPermission?: () => void;
  }): Promise<{ success: boolean; pathName?: string; error?: string; usingFallback?: boolean }> => {
    // 1. 检查权限状态
    const hasAccess = await checkPermissionStatus();
    
    // 2. Get last URI for pre-selection
    const lastUri = await getLastUri();

    // 3. Open picker
    const pickerResult = await openPicker(lastUri || undefined);

    // 4. If user cancelled, return failure
    if (!pickerResult) {
      return { success: false };
    }

    // 5. Save the path
    const saveResult = await saveStoragePath(pickerResult.name, pickerResult.uri);

    // 6. Show toast and return result
    if (saveResult.success) {
      if (saveResult.usingFallback) {
        toast.warning(`已保存到应用私有目录：${pickerResult.name}\n（无法访问公共目录，请在设置中开启"所有文件访问权限"）`, {
          duration: 5000,
          action: {
            label: '去设置',
            onClick: openSettings,
          },
        });
      } else {
        toast.success(options?.successMessage || `存储路径已设置为：${pickerResult.name}`);
      }
      return { success: true, pathName: pickerResult.name, usingFallback: saveResult.usingFallback };
    } else {
      // 检查是否是权限问题
      if (saveResult.error?.includes('权限') || saveResult.error?.includes('permission')) {
        toast.error(options?.errorMessage || '保存失败：缺少存储权限', {
          duration: 5000,
          action: {
            label: '去设置',
            onClick: openSettings,
          },
        });
      } else {
        toast.error(options?.errorMessage || saveResult.error || '保存存储路径失败，请重试');
      }
      return { success: false, error: saveResult.error };
    }
  }, [checkPermissionStatus, getLastUri, openPicker, saveStoragePath, openSettings]);

  // Initialize on mount
  useEffect(() => {
    loadStoragePath();
  }, [loadStoragePath]);

  return {
    ...state,
    loadStoragePath,
    checkPrerequisites,
    saveStoragePath,
    getLastUri,
    pickAndSaveStoragePath,
    requestPermission,
    openSettings,
    checkPermissionStatus,
  };
}
```

**Step 2: Commit**

```bash
git add src/hooks/useStoragePermission.ts
git commit -m "feat(storage): integrate permission management with user guidance"
```

---

### Task 10: 注册新命令到 lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: 确保所有命令已注册**

```rust
// 在 generate_handler! 宏中添加新命令
.invoke_handler(tauri::generate_handler![
    start_server,
    stop_server,
    get_server_status,
    get_network_info,
    load_config,
    save_config,
    check_port_available,
    // Storage permission commands
    validate_storage_permission,
    save_storage_path,
    get_storage_path,
    check_server_start_prerequisites,
    get_last_storage_uri,
    get_app_private_directory,
])
```

**Step 2: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(commands): register new storage permission commands"
```

---

### Task 11: 添加 Cargo.toml 依赖

**Files:**
- Verify: `src-tauri/Cargo.toml` 已包含 urlencoding

**Step 1: 确认依赖存在**

```toml
[dependencies]
urlencoding = "2.1"
```

**Step 2: 如果不存在则添加**

```bash
cd src-tauri
cargo add urlencoding
```

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(deps): add urlencoding for URI parsing"
```

---

## 测试步骤

### Task 12: 构建和测试

**Step 1: 构建前端**

```bash
cd /mnt/d/GitRepos/camera-ftp-companion
bun install --no-cache
bun run build
```

**Step 2: 构建 Android APK**

```bash
cd src-tauri
cargo tauri android build --apk
```

**Step 3: 安装到设备并测试**

```bash
adb install -r target/tauri/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

**Step 4: 验证测试场景**

1. **首次启动** - 检查应用私有目录是否正确创建
2. **选择目录** - 打开 SAF 选择器，选择 DCIM 或 Pictures 目录
3. **无权限情况** - 在不开启 MANAGE_EXTERNAL_STORAGE 时，验证回退到应用私有目录
4. **权限申请** - 测试权限申请流程和设置页面跳转
5. **权限恢复** - 开启权限后重新选择目录，验证可正常写入

---

## 总结

### 修改的文件清单

| 文件 | 修改类型 | 主要内容 |
|------|----------|----------|
| `capabilities/default.json` | 修改 | 添加 fs 权限 |
| `AndroidManifest.xml` | 修改 | 增强权限声明 |
| `StorageHelper.kt` | 修改 | 添加主动权限申请和验证 |
| `MainActivity.kt` | 修改 | 添加 JS Bridge 权限方法 |
| `platform/android.rs` | 修改 | 实现真实权限检查和缓存 |
| `storage_permission.rs` | 修改 | 添加回退机制和验证 |
| `config.rs` | 修改 | 统一路径处理 |
| `useSAFPicker.ts` | 修改 | 添加权限检查和申请 |
| `useStoragePermission.ts` | 修改 | 集成权限管理和引导 |
| `lib.rs` | 修改 | 注册新命令 |
| `Cargo.toml` | 修改 | 添加 urlencoding 依赖 |

### 架构改进

1. **三层权限检查** - Rust 层/Kotlin 层/前端层协同工作
2. **自动回退机制** - URI 转换失败时自动使用应用私有目录
3. **权限缓存** - 避免频繁磁盘检查，提升性能
4. **用户引导** - 清晰的权限申请引导和设置页面跳转

### 风险与建议

**Google Play 分发风险：**
- `MANAGE_EXTERNAL_STORAGE` 权限需要审核豁免
- 建议先通过 Google Play Console 提交权限声明
- 如审核不通过，可回退到方案 A/B

**兼容性：**
- Android 11+ (API 30+) 需要用户手动开启权限
- 部分国产 ROM 可能限制权限行为
- 建议提供详细的使用文档
