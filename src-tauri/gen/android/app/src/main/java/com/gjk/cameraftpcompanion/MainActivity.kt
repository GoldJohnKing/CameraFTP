/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

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
import com.gjk.cameraftpcompanion.bridges.FileWatcherBridge

class MainActivity : TauriActivity() {

    companion object {
        private const val TAG = "MainActivity"
        private const val TAURI_LISTENER_MAX_RETRIES = 50
        private const val TAURI_LISTENER_RETRY_DELAY_MS = 50L
    }

    private var webViewRef: WebView? = null
    private var fileUploadBridge: FileUploadBridge? = null
    private var serverStateBridge: ServerStateBridge? = null
    private var permissionBridge: PermissionBridge? = null
    private var fileWatcherBridge: FileWatcherBridge? = null

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
        fileWatcherBridge = FileWatcherBridge(this)
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
        addJsBridge(webView, fileWatcherBridge, "FileWatcherAndroid")

        // 注册Tauri事件监听 - 监听file-uploaded事件
        registerFileUploadEventListener()
    }
    
    /**
     * 注册Tauri事件监听
     * 通过JavaScript桥接监听Tauri后端事件
     * 使用轮询重试机制确保Tauri环境就绪
     */
    @SuppressLint("SetJavaScriptEnabled")
    private fun registerFileUploadEventListener() {
        webViewRef?.let { webView ->
            attemptRegisterEventListener(webView, 0)
        } ?: run {
            Log.e(TAG, "WebView is null, cannot register event listeners")
        }
    }
    
    /**
     * 尝试注册事件监听器，支持重试
     * @param webView WebView实例
     * @param retryCount 当前重试次数
     */
    private fun attemptRegisterEventListener(webView: android.webkit.WebView, retryCount: Int) {
        if (retryCount >= TAURI_LISTENER_MAX_RETRIES) {
            Log.w(TAG, "Max retries reached, Tauri event listener registration failed")
            return
        }
        
        val jsCode = """
            (function() {
                if (window.__tauriEventListenerRegistered) return 'already_registered';
                
                if (window.__TAURI__ && window.__TAURI__.event) {
                    window.__tauriEventListenerRegistered = true;
                    
                    // 监听Tauri的file-uploaded事件
                    window.__TAURI__.event.listen('file-uploaded', function(event) {
                        console.log('[Android] file-uploaded event received:', event.payload);
                        if (window.FileUploadAndroid) {
                            window.FileUploadAndroid.onFileUploaded(
                                event.payload.path || ''
                            );
                        }
                    });
                    
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
                    
                    return 'success';
                } else {
                    return 'not_ready';
                }
            })();
        """
        
        webView.evaluateJavascript(jsCode) { result ->
            when (result?.trim()?.removeSurrounding("\"")) {
                "success" -> Log.d(TAG, "Tauri event listeners registered successfully")
                "already_registered" -> Log.d(TAG, "Event listeners already registered")
                else -> {
                    // Tauri not ready, retry after delay
                    webView.postDelayed({
                        attemptRegisterEventListener(webView, retryCount + 1)
                    }, TAURI_LISTENER_RETRY_DELAY_MS)
                }
            }
        }
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up bridge references")
        super.onDestroy()
        // 停止文件监听
        fileWatcherBridge?.stopWatching()
        // Clear all bridge references to prevent memory leaks
        webViewRef = null
        fileUploadBridge = null
        serverStateBridge = null
        permissionBridge = null
        fileWatcherBridge = null
    }

    /**
     * 获取 WebView 引用（供 Bridge 使用）
     */
    fun getWebView(): WebView? {
        return webViewRef
    }

    /**
     * 启动文件系统监听（供外部调用）
     * @param path 要监听的目录路径
     * @return 是否成功启动
     */
    fun startFileWatching(path: String): Boolean {
        Log.d(TAG, "startFileWatching: path=$path")
        return fileWatcherBridge?.startWatching(path) ?: false
    }

    /**
     * 停止文件系统监听（供外部调用）
     */
    fun stopFileWatching() {
        Log.d(TAG, "stopFileWatching")
        fileWatcherBridge?.stopWatching()
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
                if (service == null) {
                    Log.w(TAG, "Failed to start foreground service - service is still null after start attempt")
                    return
                }
            }

            // Now update the state
            service.updateServerState(statsJson, connectedClients)
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
