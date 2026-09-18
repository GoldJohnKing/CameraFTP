/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.ComponentCallbacks2
import android.content.IntentSender
import android.os.Bundle
import android.util.Log
import android.webkit.WebView
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import com.gjk.cameraftpcompanion.bridges.GalleryActionsBridge
import com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2
import com.gjk.cameraftpcompanion.bridges.MediaStoreBridge
import com.gjk.cameraftpcompanion.bridges.ImageViewerBridge
import com.gjk.cameraftpcompanion.bridges.NnCapabilityBridge
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference

class MainActivity : TauriActivity() {

    companion object {
        private const val TAG = "MainActivity"

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
    private var galleryActionsBridge: GalleryActionsBridge? = null
    private var galleryBridgeV2: GalleryBridgeV2? = null
    private var imageViewerBridge: ImageViewerBridge? = null
    @Volatile
    private var isWebViewActive = false

    /**
     * True while onTrimMemory(UI_HIDDEN+) has paused the main WebView's
     * timers. Cleared by [resumeWebViewTimersIfPaused], which MainActivity,
     * ImageViewerActivity and ColorGradingActivity call from their
     * onResume — whichever activity the user returns to.
     */
    @Volatile
    private var webViewTimersPausedByTrim = false
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
        bridge?.let { webView.addJavascriptInterface(it, name) }
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        instance = this
        
        Log.d(TAG, "onCreate: initializing bridges")

        // On Qualcomm Hexagon v73+ (SD 8 Gen 2+) devices, pre-load the QNN/ORT
        // native libraries BEFORE raw_alchemy_core so the latter can dlopen them
        // for the NN demosaic path. Non-whitelisted devices skip this entirely.
        if (NnCapabilityBridge.isNnCapable()) {
            loadNnNativeLibraries()
        }

        // Pre-load RawAlchemyCpp so Rust can dlopen it by name
        try {
            System.loadLibrary("raw_alchemy_core")
            Log.d(TAG, "RawAlchemyCpp loaded successfully")
        } catch (e: UnsatisfiedLinkError) {
            Log.w(TAG, "RawAlchemyCpp not found, LUT filter unavailable: ${e.message}")
        }
        permissionBridge = PermissionBridge(this)
        galleryActionsBridge = GalleryActionsBridge(this)
        galleryBridgeV2 = GalleryBridgeV2(this)
        imageViewerBridge = ImageViewerBridge(this)

        // Cleanup stale pending entries (older than 24 hours).
        // MediaStore deletes are binder calls that can block — run them off
        // the main thread. Nothing later in startup depends on this cleanup
        // (freshly created pending entries are younger than the cutoff, so
        // they can never be affected by it).
        val cutoffMillis = System.currentTimeMillis() - 24 * 60 * 60 * 1000L
        Thread({
            MediaStoreBridge.cleanupStalePendingEntries(contentResolver, cutoffMillis)
        }, "StalePendingCleanup").start()

        // Back-press callback for gallery selection mode.
        // Starts disabled; registerBackPressCallback() enables it when JS enters
        // selection mode. When disabled, OnBackPressedDispatcher naturally falls
        // through to the system default — no manual re-dispatch needed.
        selectionBackCallback = object : OnBackPressedCallback(false) {
            override fun handleOnBackPressed() {
                try {
                    getWebView()?.evaluateJavascript(
                        "if (window.__galleryOnBackPressed) { window.__galleryOnBackPressed(); }",
                        null
                    )
                } catch (e: Exception) {
                    Log.e(TAG, "onBackPressed: error calling evaluateJavascript", e)
                }
            }
        }
        onBackPressedDispatcher.addCallback(this, selectionBackCallback!!)
    }

    /**
     * Load QNN/ORT native libraries in dependency order before raw_alchemy_core.
     *
     * The three core libraries are required for the NN code path; if any is
     * absent the NN path is disabled and raw_alchemy_core falls back to its
     * bilinear demosaic. The Hexagon Skel variants are loaded best-effort —
     * only the one matching this SoC's Hexagon version is expected to resolve;
     * the others throw UnsatisfiedLinkError and are ignored.
     */
    private fun loadNnNativeLibraries() {
        val required = listOf("onnxruntime", "QnnSystem", "QnnHtp")
        for (lib in required) {
            try {
                System.loadLibrary(lib)
                Log.d(TAG, "NN lib loaded: $lib")
            } catch (e: UnsatisfiedLinkError) {
                Log.e(TAG, "NN path disabled — required lib missing: $lib", e)
                return
            }
        }
        // libQnnHtpV{73,75,79,81}Skel.so — whichever matches this Hexagon version.
        for (skel in listOf("QnnHtpV73Skel", "QnnHtpV75Skel", "QnnHtpV79Skel", "QnnHtpV81Skel")) {
            try {
                System.loadLibrary(skel)
                Log.d(TAG, "NN Skel loaded: $skel")
            } catch (e: UnsatisfiedLinkError) {
                // Skel variant for a different Hexagon version — expected, ignore.
            }
        }
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
        addJsBridge(webView, galleryActionsBridge, "GalleryAndroid")
        addJsBridge(webView, galleryBridgeV2, "GalleryAndroidV2")
        addJsBridge(webView, imageViewerBridge, "ImageViewerAndroid")
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up bridge references")
        isWebViewActive = false
        galleryBridgeV2?.destroy()
        super.onDestroy()
        instance = null
        // Clear all bridge references to prevent memory leaks
        webViewRef = null
        permissionBridge = null
        galleryActionsBridge = null
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

    override fun onResume() {
        super.onResume()
        // super (WryActivity.onResume) already calls webView.onResume();
        // this only re-enables the JS/layout timers that onTrimMemory may
        // have paused while the app was fully hidden.
        resumeWebViewTimersIfPaused()
        // Returning to the app may mean the user just granted the storage
        // permission in system settings; ask the web layer to re-check so a
        // false→true transition can trigger the gallery refresh hook.
        Log.d(TAG, "onResume: requesting permission re-check")
        emitWindowEvent("permission-recheck-requested", "{}")
    }

    /**
     * Release UI-side caches when the app's UI is no longer visible
     * (TRIM_MEMORY_UI_HIDDEN and above — e.g. MIUI backgrounding/lock screen).
     *
     * Scope is strictly UI-only: the FTP server state, the color-grading and
     * AI-edit processing workers, the file index and the configuration are
     * NOT touched. Everything released here is transparently rebuilt on next
     * use (WebView timers on resume, thumbnails from the L2 disk cache,
     * viewer tiles on rebind).
     */
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        if (level < ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN) {
            // Below UI_HIDDEN some app UI is still (about to be) visible —
            // dropping these caches would cause visible re-decode churn.
            // TODO(trim-running-low): at TRIM_MEMORY_RUNNING_LOW/CRITICAL also
            //   unload NN weights / decoder pools (future work; not needed for
            //   the UI_HIDDEN tier which MIUI sends on background/lock).
            return
        }

        Log.i(TAG, "onTrimMemory($level >= UI_HIDDEN): releasing UI-side caches")

        // 1) Freeze the main WebView's rendering and JS timers. onPause() is
        //    normally already applied by WryActivity.onPause() (UI_HIDDEN is
        //    only delivered after all activities are stopped) and is
        //    idempotent; pauseTimers() is the actual addition — WryActivity
        //    never pauses timers, so JS timers would keep firing while the
        //    app is hidden. The timer pause is reversed by
        //    resumeWebViewTimersIfPaused(); the onPause() half is undone by
        //    WryActivity's own onResume lifecycle.
        getWebView()?.let { wv ->
            try {
                wv.onPause()
                wv.pauseTimers()
                webViewTimersPausedByTrim = true
            } catch (e: Exception) {
                Log.e(TAG, "onTrimMemory: failed to pause WebView", e)
            }
        }

        // 2) L1 in-memory thumbnail LruCache (L2 disk cache untouched —
        //    thumbnails re-populate from disk on next access).
        galleryBridgeV2?.trimUiMemoryCaches()

        // 3) If the image viewer is alive, recycle its offscreen tile
        //    bitmaps (current page and ±1 neighbors are kept).
        ImageViewerActivity.instance?.let { viewer ->
            if (!viewer.isFinishing && !viewer.isDestroyed) {
                viewer.recycleOffscreenTiles()
            }
        }
    }

    /**
     * Undo the WebView timer pause applied by [onTrimMemory] (if any).
     * Idempotent and safe to call when timers were never paused.
     * Called from MainActivity.onResume, ImageViewerActivity.onResume and
     * ColorGradingActivity.onResume — whichever activity the user returns
     * to after the app was hidden.
     *
     * Only resumes the JS/layout timers: WebView.onResume() itself is
     * WryActivity's lifecycle responsibility — callers may be other
     * activities while MainActivity is still stopped, and resuming the
     * main WebView then would over-restore it (it stays correctly paused
     * until WryActivity.onResume runs).
     */
    fun resumeWebViewTimersIfPaused() {
        if (!webViewTimersPausedByTrim) return
        webViewTimersPausedByTrim = false
        webViewRef?.let { wv ->
            try {
                wv.resumeTimers()
                Log.d(TAG, "WebView timers resumed after trim pause")
            } catch (e: Exception) {
                Log.e(TAG, "resumeWebViewTimersIfPaused failed", e)
            }
        }
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
     * Back-press callback toggled by gallery selection mode.
     * Its `isEnabled` state IS the selection-mode state — no separate flag needed.
     */
    private var selectionBackCallback: OnBackPressedCallback? = null

    /**
     * Enable back-press interception for selection mode.
     * Called from JS when entering selection mode.
     */
    fun registerBackPressCallback(): Boolean {
        Log.d(TAG, "registerBackPressCallback: entering selection mode")
        selectionBackCallback?.isEnabled = true
        return true
    }

    /**
     * Disable back-press interception.
     * Called from JS when exiting selection mode.
     */
    fun unregisterBackPressCallback(): Boolean {
        Log.d(TAG, "unregisterBackPressCallback: exiting selection mode")
        selectionBackCallback?.isEnabled = false
        return true
    }
}
