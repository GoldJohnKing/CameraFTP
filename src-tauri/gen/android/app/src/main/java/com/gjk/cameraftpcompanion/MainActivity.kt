/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.content.IntentSender
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.webkit.WebView
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import com.gjk.cameraftpcompanion.bridges.FileUploadBridge
import com.gjk.cameraftpcompanion.bridges.ServerStateBridge
import com.gjk.cameraftpcompanion.bridges.GalleryBridge
import com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2
import com.gjk.cameraftpcompanion.bridges.MediaStoreBridge
import com.gjk.cameraftpcompanion.bridges.ImageViewerBridge
import com.gjk.cameraftpcompanion.cache.ThumbnailCacheProvider
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class MainActivity : TauriActivity() {

    companion object {
        private const val TAG = "MainActivity"
        // 注意：这些常量与 Rust 侧 constants.rs 中的定义保持一致
        // Rust 侧: TAURI_LISTENER_MAX_RETRIES = 50
        // Rust 侧: TAURI_LISTENER_RETRY_DELAY_MS = 50L
        private const val TAURI_LISTENER_MAX_RETRIES = 50
        private const val TAURI_LISTENER_RETRY_DELAY_MS = 50L

        /**
         * Static WebView reference for cross-Activity Tauri IPC access
         */
        var instance: MainActivity? = null
            private set
    }

    private var webViewRef: WebView? = null
    private var fileUploadBridge: FileUploadBridge? = null
    private var serverStateBridge: ServerStateBridge? = null
    private var permissionBridge: PermissionBridge? = null
    private var galleryBridge: GalleryBridge? = null
    private var galleryBridgeV2: GalleryBridgeV2? = null
    private var mediaStoreBridge: MediaStoreBridge? = null
    private var imageViewerBridge: ImageViewerBridge? = null
    private val pendingDeleteResult = AtomicReference<Pair<CountDownLatch, AtomicReference<Boolean>>?>(null)
    private val deleteRequestLauncher = registerForActivityResult(
        ActivityResultContracts.StartIntentSenderForResult()
    ) { result ->
        pendingDeleteResult.getAndSet(null)?.let { (latch, approvedRef) ->
            approvedRef.set(result.resultCode == Activity.RESULT_OK)
            latch.countDown()
        }
    }

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
        instance = this
        
        Log.d(TAG, "onCreate: initializing bridges")
        fileUploadBridge = FileUploadBridge(this)
        serverStateBridge = ServerStateBridge(this)
        permissionBridge = PermissionBridge(this)
        galleryBridge = GalleryBridge(this)
        galleryBridgeV2 = GalleryBridgeV2(this)
        mediaStoreBridge = MediaStoreBridge(this)
        imageViewerBridge = ImageViewerBridge(this)

        // Initialize thumbnail cache
        ThumbnailCacheProvider.initialize(this)
        
        // Cleanup stale pending entries (older than 24 hours)
        val cutoffMillis = System.currentTimeMillis() - 24 * 60 * 60 * 1000L
        MediaStoreBridge.cleanupStalePendingEntries(contentResolver, cutoffMillis)
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
        addJsBridge(webView, galleryBridge, "GalleryAndroid")
        addJsBridge(webView, galleryBridgeV2, "GalleryAndroidV2")
        addJsBridge(webView, mediaStoreBridge, "MediaStoreAndroid")
        addJsBridge(webView, imageViewerBridge, "ImageViewerAndroid")

        // 注册Tauri事件监听
        registerTauriEventListeners()
    }
    
    /**
     * 注册Tauri事件监听
     * 通过JavaScript桥接监听Tauri后端事件
     * 使用轮询重试机制确保Tauri环境就绪
     */
    @SuppressLint("SetJavaScriptEnabled")
    private fun registerTauriEventListeners() {
        webViewRef?.let { webView ->
            EventListenerRegistration(webView).start()
        } ?: Log.e(TAG, "WebView is null, cannot register event listeners")
    }

    /**
     * Tauri事件监听器注册器
     * 处理重试逻辑和事件注册
     */
    private inner class EventListenerRegistration(private val webView: WebView) {
        private var retryCount = 0

        fun start() {
            attemptRegister()
        }

        private fun attemptRegister() {
            if (retryCount >= TAURI_LISTENER_MAX_RETRIES) {
                Log.w(TAG, "Max retries reached, Tauri event listener registration failed")
                return
            }

            webView.evaluateJavascript(jsRegistrationCode) { result ->
                handleResult(result?.trim()?.removeSurrounding("\""))
            }
        }

        private fun handleResult(result: String?) {
            when (result) {
                "success" -> Log.d(TAG, "Tauri event listeners registered successfully")
                "already_registered" -> Log.d(TAG, "Event listeners already registered")
                else -> {
                    retryCount++
                    webView.postDelayed({ attemptRegister() }, TAURI_LISTENER_RETRY_DELAY_MS)
                }
            }
        }
    }

    /**
     * 用于注册Tauri事件监听器的JavaScript代码
     */
    private val jsRegistrationCode = """
        (function() {
            if (window.__tauriEventListenerRegistered) return 'already_registered';
            
            if (window.__TAURI__?.event) {
                window.__tauriEventListenerRegistered = true;
                
                window.__TAURI__.event.listen('android-service-state-update', (event) => {
                    const p = event.payload;
                    window.ServerStateAndroid?.onServerStateChanged(
                        p?.is_running || false,
                        p?.stats ? JSON.stringify(p.stats) : null,
                        p?.connected_clients || 0
                    );
                });
                
                return 'success';
            }
            return 'not_ready';
        })();
    """.trimIndent()

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up bridge references")
        super.onDestroy()
        instance = null
        // Clear all bridge references to prevent memory leaks
        webViewRef = null
        fileUploadBridge = null
        serverStateBridge = null
        permissionBridge = null
        galleryBridge = null
        galleryBridgeV2 = null
        mediaStoreBridge = null
        imageViewerBridge = null
    }

    /**
     * 获取 WebView 引用（供 Bridge 使用）
     */
    fun getWebView(): WebView? {
        return webViewRef
    }

    /**
     * Emit a Tauri event to the WebView
     * @param name Event name
     * @param payloadJson JSON payload as string
     */
    fun emitTauriEvent(name: String, payloadJson: String) {
        val webView = webViewRef ?: return
        val script = "window.__TAURI__?.event?.emit('$name', $payloadJson)"
        runOnUiThread {
            webView.evaluateJavascript(script, null)
        }
    }

    /**
     * Dispatch a browser CustomEvent to the main window WebView.
     * @param name Event name
     * @param detailJson JSON detail object as string
     */
    fun emitWindowEvent(name: String, detailJson: String) {
        val webView = webViewRef ?: return
        val script = "window.dispatchEvent(new CustomEvent('$name', { detail: $detailJson }))"
        runOnUiThread {
            webView.evaluateJavascript(script, null)
        }
    }

    fun requestDeleteConfirmation(intentSender: IntentSender): Boolean {
        val latch = CountDownLatch(1)
        val approvedRef = AtomicReference(false)
        val pendingResult = latch to approvedRef
        pendingDeleteResult.set(pendingResult)

        runOnUiThread {
            try {
                val request = IntentSenderRequest.Builder(intentSender).build()
                deleteRequestLauncher.launch(request)
            } catch (e: Exception) {
                Log.e(TAG, "requestDeleteConfirmation: failed to launch delete request", e)
                pendingDeleteResult.getAndSet(null)?.let { (pendingLatch, pendingApprovedRef) ->
                    pendingApprovedRef.set(false)
                    pendingLatch.countDown()
                }
            }
        }

        val completed = latch.await(30, TimeUnit.SECONDS)
        if (!completed) {
            pendingDeleteResult.compareAndSet(pendingResult, null)
            Log.w(TAG, "requestDeleteConfirmation: timed out waiting for system dialog result")
        }

        return completed && approvedRef.get()
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

        when {
            isRunning -> startOrUpdateService(statsJson, connectedClients)
            else -> stopServiceIfRunning()
        }
    }

    private fun startOrUpdateService(statsJson: String?, connectedClients: Int) {
        val service = getOrCreateService()
        service?.updateServerState(statsJson, connectedClients)
            ?: Log.w(TAG, "Failed to start foreground service")
    }

    private fun getOrCreateService(): FtpForegroundService? {
        return FtpForegroundService.getInstance() ?: run {
            startFtpForegroundService()
            FtpForegroundService.getInstance()
        }
    }

    private fun stopServiceIfRunning() {
        FtpForegroundService.getInstance()?.let {
            stopFtpForegroundService()
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

    /**
     * Flag to track if we're in selection mode (for back button handling)
     */
    private var isInSelectionMode = false

    /**
     * Register back press callback to intercept back button
     * Called from JS when entering selection mode
     */
    fun registerBackPressCallback(): Boolean {
        Log.d(TAG, "registerBackPressCallback: entering selection mode")
        isInSelectionMode = true
        return true
    }

    /**
     * Unregister back press callback
     * Called from JS when exiting selection mode
     */
    fun unregisterBackPressCallback(): Boolean {
        Log.d(TAG, "unregisterBackPressCallback: exiting selection mode")
        isInSelectionMode = false
        return true
    }

    /**
     * Handle back button press
     * Override to intercept back button when in selection mode
     */
    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (isInSelectionMode) {
            // Notify JS to cancel selection
            try {
                webViewRef?.evaluateJavascript(
                    "if (window.__galleryOnBackPressed) { window.__galleryOnBackPressed(); }",
                    null
                )
            } catch (e: Exception) {
                Log.e(TAG, "onBackPressed: error calling evaluateJavascript", e)
            }
            // Don't call super to prevent default back behavior
            return
        }

        // Normal back behavior
        @Suppress("DEPRECATION")
        super.onBackPressed()
    }
}
