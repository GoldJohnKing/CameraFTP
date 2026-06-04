/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.net.URLDecoder
import java.util.concurrent.atomic.AtomicLong

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    internal var webView: WebView? = null
    internal var previewFilePath: String? = null
    internal var isSessionActive = false
    internal var previewBridge: CGPreviewResultBridge? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val filePath = intent.getStringExtra("filePath")
        if (filePath == null) {
            Log.e(TAG, "No filePath provided")
            finish()
            return
        }

        WindowCompat.setDecorFitsSystemWindows(window, false)

        previewBridge = CGPreviewResultBridge(this)

        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            settings.allowFileAccess = false

            webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(
                    view: WebView, request: WebResourceRequest
                ): WebResourceResponse? {
                    if (request.url.scheme == "preview" && request.url.host == "latest") {
                        val path = previewFilePath
                        if (path != null) {
                            val file = File(path)
                            if (file.exists()) {
                                return WebResourceResponse(
                                    "image/jpeg", null, 200, "OK",
                                    mapOf("Content-Length" to file.length().toString()),
                                    FileInputStream(file)
                                )
                            }
                        }
                        return WebResourceResponse(
                            "image/jpeg", null, 404, "Not Found",
                            emptyMap(), null
                        )
                    }
                    return super.shouldInterceptRequest(view, request)
                }
            }

            addJavascriptInterface(
                NativeColorGradingPreviewBridge(this@ColorGradingActivity, filePath),
                "NativeBridge"
            )
            loadUrl("file:///android_asset/color_grading_preview.html")
        }

        val container = FrameLayout(this).apply {
            fitsSystemWindows = true
            addView(webView, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
        }
        setContentView(container)

        // Register callback bridge on main WebView for async results
        registerPreviewBridge()
    }

    private fun registerPreviewBridge() {
        val mainActivity = MainActivity.instance ?: return
        mainActivity.runOnUiThread {
            try {
                mainActivity.getWebView()?.addJavascriptInterface(previewBridge!!, "CGPreviewBridge")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to register CGPreviewBridge", e)
            }
        }
    }

    private fun unregisterPreviewBridge() {
        val mainActivity = MainActivity.instance ?: return
        mainActivity.runOnUiThread {
            try {
                mainActivity.getWebView()?.removeJavascriptInterface("CGPreviewBridge")
            } catch (_: Exception) {}
        }
    }

    override fun onDestroy() {
        if (isSessionActive) {
            endPreviewSession()
        }
        unregisterPreviewBridge()
        webView?.let {
            (it.parent as? android.view.ViewGroup)?.removeView(it)
            it.destroy()
        }
        webView = null
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        if (isSessionActive) {
            endPreviewSession()
        }
        super.onBackPressed()
    }

    internal fun fireAndForget(js: String) {
        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available")
            runOnUiThread {
                Toast.makeText(this, "无法连接后端", Toast.LENGTH_SHORT).show()
                finish()
            }
            return
        }
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js, null)
        }
    }

    internal fun endPreviewSession() {
        isSessionActive = false
        previewFilePath = null
        fireAndForget(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} })();"
        )
    }

    internal fun extractFilePathFromUrl(url: String): String? {
        // Rust returns "http://image-preview.localhost/<percent-encoded-path>"
        // Extract the path portion and URL-decode it
        val prefix = "http://image-preview.localhost/"
        if (!url.startsWith(prefix)) {
            // Maybe it's already a raw path (shouldn't happen but handle gracefully)
            if (File(url).exists()) return url
            return null
        }
        val encoded = url.substring(prefix.length)
        return try {
            URLDecoder.decode(encoded, "UTF-8")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to decode preview URL: $url", e)
            null
        }
    }
}

class CGPreviewResultBridge(activity: ColorGradingActivity) {
    private val activityRef: WeakReference<ColorGradingActivity> = WeakReference(activity)
    private val pendingRequestId = AtomicLong(0)

    fun nextRequestId(): Long = pendingRequestId.incrementAndGet()
    fun currentRequestId(): Long = pendingRequestId.get()

    @JavascriptInterface
    fun onBeginResult(requestId: Long, success: Boolean, errorMessage: String?) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "onBeginResult: success=$success id=$requestId")
        if (requestId != currentRequestId()) return
        activity.runOnUiThread {
            if (success) {
                activity.isSessionActive = true
                activity.webView?.evaluateJavascript("window.onPreviewReady?.();", null)
            } else {
                activity.webView?.evaluateJavascript(
                    "window.onPreviewError?.(${JSONObject.quote(errorMessage ?: "解码失败")});", null
                )
            }
        }
    }

    @JavascriptInterface
    fun onApplyResult(requestId: Long, resultUrl: String?) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "onApplyResult: url=$resultUrl id=$requestId (current=${currentRequestId()})")
        if (requestId != currentRequestId()) return
        if (resultUrl == null || resultUrl.isEmpty()) {
            activity.runOnUiThread {
                activity.webView?.evaluateJavascript(
                    "window.notifyPreviewError?.('Empty result');", null
                )
            }
            return
        }
        val filePath = activity.extractFilePathFromUrl(resultUrl)
        if (filePath != null) {
            activity.previewFilePath = filePath
            activity.runOnUiThread {
                activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
            }
        } else {
            activity.runOnUiThread {
                activity.webView?.evaluateJavascript(
                    "window.notifyPreviewError?.(${JSONObject.quote("Invalid preview URL: $resultUrl")});", null
                )
            }
        }
    }

    @JavascriptInterface
    fun onApplyError(requestId: Long, errorMessage: String?) {
        val activity = activityRef.get() ?: return
        Log.e(TAG, "onApplyError: $errorMessage id=$requestId")
        if (requestId != currentRequestId()) return
        activity.runOnUiThread {
            activity.webView?.evaluateJavascript(
                "window.notifyPreviewError?.(${JSONObject.quote(errorMessage ?: "应用失败")});", null
            )
        }
    }

    companion object {
        private const val TAG = "CGPreviewResultBridge"
    }
}

private class NativeColorGradingPreviewBridge(
    activity: ColorGradingActivity,
    private val filePath: String,
) {
    private val activityRef: WeakReference<ColorGradingActivity> = WeakReference(activity)
    private val applyRequestId = AtomicLong(0)

    @JavascriptInterface
    fun beginPreview(filePath: String) {
        val activity = activityRef.get() ?: return
        val bridge = activity.previewBridge ?: return
        val reqId = bridge.nextRequestId()
        Log.d(TAG, "beginPreview: $filePath reqId=$reqId")
        activity.fireAndForget(
            "(async function(){ var reqId=$reqId; try { await window.__tauriBeginColorGradingPreview?.('${filePath.replace("'", "\\'")}'); window.CGPreviewBridge?.onBeginResult(reqId, true, null); } catch(e) { window.CGPreviewBridge?.onBeginResult(reqId, false, e.message); } })();"
        )
    }

    @JavascriptInterface
    fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        val bridge = activity.previewBridge ?: return
        val myId = applyRequestId.incrementAndGet()
        // Sync the apply request ID with the bridge's request ID so stale results are discarded
        while (bridge.currentRequestId() < myId) {
            bridge.nextRequestId()
        }
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset id=$myId")
        activity.fireAndForget(
            "(async function(){ var reqId=$myId; try { var r = await window.__tauriApplyColorGradingPreview?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset}); window.CGPreviewBridge?.onApplyResult(reqId, r || ''); } catch(e) { window.CGPreviewBridge?.onApplyError(reqId, e.message); } })();"
        )
    }

    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        activity.isSessionActive = false
        activity.previewFilePath = null

        // Fire all three operations sequentially via chained promises
        activity.fireAndForget(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} try { await window.__tauriTriggerColorGrading?.('${filePath.replace("'", "\\'")}','${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset},false); } catch(e) {} try { window.__tauriSaveColorGradingLastUsed?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset}); } catch(e) {} })();"
        )

        // Close after a short delay to allow the JS to start executing
        activity.runOnUiThread {
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                activity.finish()
            }, 100)
        }
    }

    @JavascriptInterface
    fun cancelPreview() {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "cancelPreview")
        activity.endPreviewSession()
        activity.runOnUiThread { activity.finish() }
    }

    @JavascriptInterface
    fun getConfig(): String {
        val activity = activityRef.get() ?: return "{}"

        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            return JSONObject().apply {
                put("filePath", filePath)
                put("presets", JSONArray())
            }.toString()
        }

        // Synchronous blocking call to get config from main WebView
        val resultFuture = java.util.concurrent.CompletableFuture<String>()
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{var l=window.__tauriGetColorGradingLastUsed?.()??'null';var p=window.__tauriGetColorGradingPresets?.()??'[]';return JSON.stringify({lastUsed:l,presets:p})}catch(e){return JSON.stringify({lastUsed:'null',presets:'[]'})}})();"
            ) { result ->
                resultFuture.complete(result ?: "{}")
            }
        }

        val raw = try {
            resultFuture.get(5, java.util.concurrent.TimeUnit.SECONDS)
        } catch (e: Exception) {
            Log.w(TAG, "getConfig timed out or failed", e)
            return JSONObject().apply {
                put("filePath", filePath)
                put("presets", JSONArray())
            }.toString()
        }

        // Parse the nested JSON: evaluateJavascript returns a JSON-encoded string
        val trimmed = raw.trim()
        val outerStr = if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
            try { JSONArray("[$trimmed]").getString(0) } catch (_: Exception) { trimmed.removeSurrounding("\"") }
        } else {
            trimmed
        }

        val json = try { JSONObject(outerStr) } catch (e: Exception) { JSONObject() }

        val lastUsedStr = json.optString("lastUsed", "null")
        // lastUsedStr is itself a JSON string (possibly double-encoded by evaluateJavascript)
        val lastUsedDecoded = if (lastUsedStr.startsWith("\"")) {
            try { lastUsedStr.removeSurrounding("\"").replace("\\\"", "\"") } catch (_: Exception) { lastUsedStr }
        } else {
            lastUsedStr
        }
        val lastUsed = if (lastUsedDecoded != "null" && lastUsedDecoded.isNotEmpty()) {
            try { JSONObject(lastUsedDecoded) } catch (e: Exception) { null }
        } else null

        val presetsStr = json.optString("presets", "[]")
        // presetsStr is also potentially double-encoded
        val presetsDecoded = if (presetsStr.startsWith("\"")) {
            try { presetsStr.removeSurrounding("\"").replace("\\\"", "\"") } catch (_: Exception) { presetsStr }
        } else {
            presetsStr
        }
        val presetsArr = try { JSONArray(presetsDecoded) } catch (e: Exception) {
            Log.w(TAG, "Failed to parse presets: $presetsDecoded", e)
            JSONArray()
        }

        return JSONObject().apply {
            put("filePath", filePath)
            put("lastUsed", lastUsed ?: JSONObject.NULL)
            put("presets", presetsArr)
        }.toString()
    }

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"
    }
}
