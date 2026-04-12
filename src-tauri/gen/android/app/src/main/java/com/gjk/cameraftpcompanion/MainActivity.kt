/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.IntentSender
import android.os.Bundle
import android.util.Log
import android.webkit.WebView
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import com.gjk.cameraftpcompanion.bridges.GalleryBridge
import com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2
import com.gjk.cameraftpcompanion.bridges.MediaStoreBridge
import com.gjk.cameraftpcompanion.bridges.ImageViewerBridge
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

class MainActivity : TauriActivity() {

    companion object {
        private const val TAG = "MainActivity"
        // 注意：这些常量与 Rust 侧 constants.rs 中的定义保持一致
        // Rust 侧: TAURI_LISTENER_MAX_RETRIES = 50
        // Rust 侧: TAURI_LISTENER_RETRY_DELAY_MS = 50L
        private const val TAURI_LISTENER_MAX_RETRIES = 50
        private const val TAURI_LISTENER_RETRY_DELAY_MS = 50L
        private val JS_REGISTRATION_CODE = """
            (function() {
                if (window.__tauriEventListenerRegistered) return 'already_registered';
                
                if (window.__TAURI__?.event) {
                    window.__tauriEventListenerRegistered = true;

                    return 'success';
                }
                return 'not_ready';
            })();
        """.trimIndent()

        /**
         * Static WebView reference for cross-Activity Tauri IPC access
         */
        var instance: MainActivity? = null
            private set

        @Volatile
        var isAppVisible: Boolean = false
            private set

        private val visibleActivityCount = AtomicInteger(0)

        @JvmStatic
        fun markActivityVisible() {
            val visibleCount = visibleActivityCount.incrementAndGet()
            isAppVisible = visibleCount > 0
        }

        @JvmStatic
        fun markActivityHidden() {
            val visibleCount = visibleActivityCount.updateAndGet { curr -> (curr - 1).coerceAtLeast(0) }
            isAppVisible = visibleCount > 0
        }
    }

    private var webViewRef: WebView? = null
    private var permissionBridge: PermissionBridge? = null
    private var galleryBridge: GalleryBridge? = null
    private var galleryBridgeV2: GalleryBridgeV2? = null
    private var imageViewerBridge: ImageViewerBridge? = null
    @Volatile
    private var isWebViewActive = false
    private var eventListenerRegistration: EventListenerRegistration? = null
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
        permissionBridge = PermissionBridge(this)
        galleryBridge = GalleryBridge(this)
        galleryBridgeV2 = GalleryBridgeV2(this)
        imageViewerBridge = ImageViewerBridge(this)

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
        isWebViewActive = true
        
        Log.d(TAG, "onWebViewCreate: adding JavaScript bridges")
        addJsBridge(webView, permissionBridge, "PermissionAndroid")
        addJsBridge(webView, galleryBridge, "GalleryAndroid")
        addJsBridge(webView, galleryBridgeV2, "GalleryAndroidV2")
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
            eventListenerRegistration?.cancel()
            eventListenerRegistration = EventListenerRegistration(webView).also { it.start() }
        } ?: Log.e(TAG, "WebView is null, cannot register event listeners")
    }

    /**
     * Tauri事件监听器注册器
     * 处理重试逻辑和事件注册
     */
    private inner class EventListenerRegistration(private val webView: WebView) {
        private var retryCount = 0
        private var cancelled = false
        private val retryRunnable = Runnable { attemptRegister() }

        fun start() {
            attemptRegister()
        }

        fun cancel() {
            cancelled = true
            webView.removeCallbacks(retryRunnable)
        }

        private fun attemptRegister() {
            if (cancelled || !isWebViewActive || webViewRef !== webView) {
                return
            }

            if (retryCount >= TAURI_LISTENER_MAX_RETRIES) {
                Log.w(TAG, "Max retries reached, Tauri event listener registration failed")
                return
            }

            webView.evaluateJavascript(JS_REGISTRATION_CODE) { result ->
                handleResult(result?.trim()?.removeSurrounding("\""))
            }
        }

        private fun handleResult(result: String?) {
            if (cancelled || !isWebViewActive || webViewRef !== webView) {
                return
            }

            when (result) {
                "success" -> Log.d(TAG, "Tauri event listeners registered successfully")
                "already_registered" -> Log.d(TAG, "Event listeners already registered")
                else -> {
                    retryCount++
                    webView.postDelayed(retryRunnable, TAURI_LISTENER_RETRY_DELAY_MS)
                }
            }
        }
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up bridge references")
        isWebViewActive = false
        eventListenerRegistration?.cancel()
        eventListenerRegistration = null
        galleryBridgeV2?.destroy()
        super.onDestroy()
        instance = null
        // Clear all bridge references to prevent memory leaks
        webViewRef = null
        permissionBridge = null
        galleryBridge = null
        galleryBridgeV2 = null
        imageViewerBridge = null
    }

    override fun onStart() {
        super.onStart()
        markActivityVisible()
    }

    override fun onStop() {
        markActivityHidden()
        super.onStop()
    }

    /**
     * 获取 WebView 引用（供 Bridge 使用）
     */
    fun getWebView(): WebView? {
        if (!isWebViewActive || isDestroyed) {
            return null
        }

        return webViewRef
    }

    /**
     * Dispatch a browser CustomEvent to the main window WebView.
     * @param name Event name
     * @param detailJson JSON detail object as string
     */
    fun emitWindowEvent(name: String, detailJson: String) {
        val script = "window.dispatchEvent(new CustomEvent('$name', { detail: $detailJson }))"
        runOnUiThread {
            getWebView()?.evaluateJavascript(script, null)
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
                getWebView()?.evaluateJavascript(
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
