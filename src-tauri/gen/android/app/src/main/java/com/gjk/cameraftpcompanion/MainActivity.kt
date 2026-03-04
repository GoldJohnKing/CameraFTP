package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import com.gjk.cameraftpcompanion.bridges.FileUploadBridge
import com.gjk.cameraftpcompanion.bridges.ServerStateBridge

class MainActivity : TauriActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        private const val TAURI_EVENT_LISTENER_DELAY_MS = 100L
    }
    
    private var webViewRef: WebView? = null
    private var fileUploadBridge: FileUploadBridge? = null
    private var serverStateBridge: ServerStateBridge? = null
    private var permissionBridge: PermissionBridge? = null

    /**
     * Helper to add a JavaScript bridge to WebView with logging
     */
    private fun addJsBridge(webView: WebView, bridge: Any?, name: String) {
        bridge?.let {
            webView.addJavascriptInterface(it, name)
        }
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        
        Log.d(TAG, "onCreate: initializing bridges")
        fileUploadBridge = FileUploadBridge(this)
        serverStateBridge = ServerStateBridge(this)
        permissionBridge = PermissionBridge(this)
    }

    /**
     * WebView创建完成时调用（由WryActivity触发）
     * 这是添加JavaScript Bridge的正确时机
     */
    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        
        // 保存WebView引用
        webViewRef = webView
        
        Log.d(TAG, "onWebViewCreate: adding JavaScript bridges")
        addJsBridge(webView, fileUploadBridge, "FileUploadAndroid")
        addJsBridge(webView, serverStateBridge, "ServerStateAndroid")
        addJsBridge(webView, permissionBridge, "PermissionAndroid")
        
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
                                        event.payload.path || ''
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
                webView.evaluateJavascript(jsCode, null)
            }, TAURI_EVENT_LISTENER_DELAY_MS)
        } ?: run {
            Log.e(TAG, "WebView is null, cannot register event listeners")
        }
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up bridge references")
        super.onDestroy()
        // Clear all bridge references to prevent memory leaks
        webViewRef = null
        fileUploadBridge = null
        serverStateBridge = null
        permissionBridge = null
    }
    
    /**
     * Start the FTP foreground service
     */
    private fun startFtpForegroundService() {
        Log.d(TAG, "startFtpForegroundService: starting FTP service")
        val serviceIntent = Intent(this, FtpForegroundService::class.java).apply {
            action = FtpForegroundService.ACTION_START
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }
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
        // Service is started by updateServiceState() when server actually starts
    }
    
    /**
     * Update service state (called from JS bridge)
     * This also handles starting/stopping the foreground service based on server state
     */
    fun updateServiceState(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        Log.d(TAG, "updateServiceState: isRunning=$isRunning, connectedClients=$connectedClients")
        var service = FtpForegroundService.getInstance()

        if (isRunning) {
            // Server is running - ensure foreground service is started
            if (service == null) {
                startFtpForegroundService()
                service = FtpForegroundService.getInstance()
            }
            
            // Now update the state
            service?.updateServerState(statsJson, connectedClients)
        } else {
            // Server is stopped - stop foreground service
            if (service != null) {
                stopFtpForegroundService()
            }
        }
    }
    
    /**
     * Stop the foreground service
     */
    private fun stopFtpForegroundService() {
        Log.d(TAG, "stopFtpForegroundService: stopping FTP service")
        val intent = Intent(this, FtpForegroundService::class.java).apply {
            action = FtpForegroundService.ACTION_STOP
        }
        stopService(intent)
    }
}
