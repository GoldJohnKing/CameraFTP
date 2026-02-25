package com.gjk.cameraftpcompanion

import android.Manifest
import android.annotation.SuppressLint
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

/**
 * JavaScript Bridge 抽象基类
 * 提供通用的日志和 UI 线程处理功能
 */
abstract class BaseJsBridge(
    protected val activity: MainActivity,
    private val bridgeName: String
) {
    protected fun log(msg: String) = Log.d(bridgeName, msg)
    protected fun runOnUi(block: () -> Unit) = activity.runOnUiThread(block)
}

/**
 * 文件上传事件监听器
 * 监听Rust后端的file-uploaded事件，触发媒体扫描
 */
class FileUploadListener(private val activity: MainActivity) {
    companion object {
        private const val TAG = "FileUploadListener"
        // 默认存储路径
        private const val DEFAULT_STORAGE_PATH = "/storage/emulated/0/DCIM/CameraFTP"
    }

    /**
     * 处理文件上传事件
     * 由Rust后端通过Tauri事件系统调用
     * @param path 文件路径（可能是相对路径或绝对路径）
     * @param size 文件大小（字节）
     */
    fun onFileUploaded(path: String?, size: Long) {
        if (path.isNullOrEmpty()) {
            Log.w(TAG, "Received empty file path, skipping media scan")
            return
        }
        
        // 构建完整文件路径
        val fullPath = if (path.startsWith("/")) {
            // 已经是绝对路径
            path
        } else {
            // 相对路径，拼接基础路径
            "$DEFAULT_STORAGE_PATH/$path"
        }
        
        Log.i(TAG, "File uploaded: relativePath=$path, fullPath=$fullPath, size=$size bytes")
        
        // 触发媒体扫描，让照片出现在相册中
        activity.runOnUiThread {
            MediaScannerHelper.scanFile(activity, fullPath)
        }
    }
}

/**
 * 文件上传JavaScript Bridge
 * 接收来自WebView的file-uploaded事件
 * 注：此类不继承BaseJsBridge，因为它依赖的是FileUploadListener而非MainActivity
 */
class FileUploadBridge(private val listener: FileUploadListener) {
    companion object {
        private const val TAG = "FileUploadBridge"
    }
    
    /**
     * 由JavaScript调用，处理文件上传事件
     */
    @JavascriptInterface
    fun onFileUploaded(path: String?, size: Long) {
        Log.d(TAG, "onFileUploaded called from JavaScript: path=$path, size=$size")
        listener.onFileUploaded(path, size)
    }
}

/**
 * Server State JavaScript Bridge
 * Receives server state updates from Tauri/Rust and forwards to foreground service
 */
class ServerStateBridge(activity: MainActivity) : BaseJsBridge(activity, "ServerStateBridge") {

    /**
     * Called from JavaScript when server state changes
     * @param isRunning Whether FTP server is running
     * @param statsJson JSON string with stats (files_transferred, bytes_transferred)
     * @param connectedClients Number of connected clients
     */
    @JavascriptInterface
    fun onServerStateChanged(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        LogWriter.log("ServerStateBridge.onServerStateChanged() called")
        LogWriter.log("isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        log("onServerStateChanged: running=$isRunning, clients=$connectedClients, stats=$statsJson")
        activity.updateServiceState(isRunning, statsJson, connectedClients)
    }
}

/**
 * SAF (Storage Access Framework) JavaScript Bridge
 * 仅保留所有文件访问权限设置功能
 */
class SAFPickerBridge(activity: MainActivity) : BaseJsBridge(activity, "SAFPickerBridge") {

    /**
     * 打开所有文件访问权限设置页面
     * 直接跳转到系统设置中的权限开关页面
     */
    @JavascriptInterface
    fun openAllFilesAccessSettings(): Boolean {
        log("openAllFilesAccessSettings called from JavaScript")

        runOnUi {
            StorageHelper.openManageStorageSettings(activity)
        }

        return true
    }
}

class MainActivity : TauriActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        private const val REQUEST_POST_NOTIFICATIONS = 1001
        @JvmStatic
        var currentActivity: MainActivity? = null
    }
    
    private var safBridge: SAFPickerBridge? = null
    private var webViewRef: WebView? = null
    private var fileUploadListener: FileUploadListener? = null
    private var fileUploadBridge: FileUploadBridge? = null
    private var serverStateBridge: ServerStateBridge? = null
    private var permissionBridge: PermissionBridge? = null
    private var ftpService: FtpForegroundService? = null

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        currentActivity = this
        
        LogWriter.init()
        LogWriter.log("MainActivity.onCreate() called")
        Log.d(TAG, "MainActivity created")
        
        // 初始化Bridge
        safBridge = SAFPickerBridge(this)
        
        // 初始化权限Bridge
        permissionBridge = PermissionBridge(this)
        
        // 初始化文件上传监听器
        fileUploadListener = FileUploadListener(this)
        fileUploadBridge = FileUploadBridge(fileUploadListener!!)
        
        // 初始化服务器状态Bridge
        serverStateBridge = ServerStateBridge(this)
        
        // Note: Foreground service is started when user clicks "启动服务器"
        // This avoids showing notification before server is actually running
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
        
        // 添加文件上传Bridge
        fileUploadBridge?.let { bridge ->
            webView.addJavascriptInterface(bridge, "FileUploadAndroid")
            Log.d(TAG, "JavaScript Bridge 'FileUploadAndroid' added to WebView")
        }
        
        // 添加服务器状态Bridge
        serverStateBridge?.let { bridge ->
            webView.addJavascriptInterface(bridge, "ServerStateAndroid")
            Log.d(TAG, "JavaScript Bridge 'ServerStateAndroid' added to WebView")
        }
        
        // 添加权限Bridge
        permissionBridge?.let { bridge ->
            webView.addJavascriptInterface(bridge, "PermissionAndroid")
            Log.d(TAG, "JavaScript Bridge 'PermissionAndroid' added to WebView")
        }
        
        // 注册Tauri事件监听 - 监听file-uploaded事件
        registerFileUploadEventListener()
    }
    
    /**
     * 注册Tauri事件监听
     * 通过JavaScript桥接监听Tauri后端事件
     */
    @SuppressLint("SetJavaScriptEnabled")
    private fun registerFileUploadEventListener() {
        webViewRef?.let { webView ->
            // 延迟注入确保Tauri环境已就绪（减少延迟从500ms到100ms）
            webView.postDelayed({
                val jsCode = """
                    (function() {
                        if (window.__tauriEventListenerRegistered) return;
                        window.__tauriEventListenerRegistered = true;
                        
                        if (window.__TAURI__ && window.__TAURI__.event) {
                            // 监听Tauri的file-uploaded事件
                            window.__TAURI__.event.listen('file-uploaded', function(event) {
                                console.log('[Android] file-uploaded event received:', event.payload);
                                // 调用原生方法
                                if (window.FileUploadAndroid) {
                                    window.FileUploadAndroid.onFileUploaded(
                                        event.payload.path || '',
                                        event.payload.size || 0
                                    );
                                }
                            });
                            console.log('[Android] file-uploaded event listener registered');
                            
                            // 监听服务器状态更新事件
                            window.__TAURI__.event.listen('android-service-state-update', function(event) {
                                console.log('[Android] android-service-state-update event received:', event.payload);
                                if (window.ServerStateAndroid) {
                                    window.ServerStateAndroid.onServerStateChanged(
                                        event.payload.is_running || false,
                                        event.payload.stats ? JSON.stringify(event.payload.stats) : null,
                                        event.payload.connected_clients || 0
                                    );
                                }
                            });
                            console.log('[Android] android-service-state-update event listener registered');
                        } else {
                            console.warn('[Android] Tauri event API not available');
                        }
                    })();
                """
                webView.evaluateJavascript(jsCode) { result ->
                    Log.d(TAG, "Tauri event listeners registration result: $result")
                }
            }, 100)
        } ?: run {
            Log.e(TAG, "WebView is null, cannot register event listeners")
        }
    }

    override fun onResume() {
        super.onResume()
        Log.d(TAG, "onResume: App resumed")
    }
    
    override fun onPause() {
        super.onPause()
        Log.d(TAG, "onPause: App paused")
    }

    override fun onDestroy() {
        super.onDestroy()
        webViewRef = null
        if (currentActivity == this) {
            currentActivity = null
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
    
    /**
     * Start the FTP foreground service
     */
    private fun startFtpForegroundService() {
        val serviceIntent = Intent(this, FtpForegroundService::class.java).apply {
            action = FtpForegroundService.ACTION_START
        }
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }
        Log.d(TAG, "Foreground service started")
    }
    
    /**
     * Handle permission request results
     */
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        Log.d(TAG, "onRequestPermissionsResult: requestCode=$requestCode, results=${grantResults.joinToString()}")
        
        when (requestCode) {
            REQUEST_POST_NOTIFICATIONS -> {
                val granted = grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED
                Log.d(TAG, "Notification permission result: granted=$granted")
                
                if (granted) {
                    // Start foreground service now that permission is granted
                    startFtpForegroundService()
                    Log.d(TAG, "Foreground service started after permission granted")
                }
            }
        }
    }
    
    /**
     * Update service state (called from JS bridge)
     * This also handles starting/stopping the foreground service based on server state
     */
    fun updateServiceState(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        LogWriter.log("MainActivity.updateServiceState() called")
        LogWriter.log("isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        Log.d(TAG, "updateServiceState: running=$isRunning, clients=$connectedClients, stats=$statsJson")
        
        val service = FtpForegroundService.getInstance()
        
        if (isRunning) {
            // Server is running - ensure foreground service is started
            if (service == null) {
                LogWriter.log("Foreground service not running, starting it now")
                Log.d(TAG, "Starting foreground service before updating state")
                startFtpForegroundService()
            }
            
            // Now update the state
            FtpForegroundService.getInstance()?.updateServerState(isRunning, statsJson, connectedClients)
        } else {
            // Server is stopped - stop foreground service
            if (service != null) {
                LogWriter.log("Server stopped, stopping foreground service")
                Log.d(TAG, "Stopping foreground service")
                stopFtpForegroundService()
            }
        }
    }
    
    /**
     * Stop the foreground service
     */
    private fun stopFtpForegroundService() {
        val intent = Intent(this, FtpForegroundService::class.java)
        intent.action = "com.gjk.cameraftpcompanion.STOP_SERVICE"
        stopService(intent)
        Log.d(TAG, "Foreground service stop requested")
    }
}
