/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

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
import com.gjk.cameraftpcompanion.bridges.ColorGradingJniBridge
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.net.URLDecoder

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    internal var webView: WebView? = null
    internal var previewFilePath: String? = null
    internal var isSessionActive = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val filePath = intent.getStringExtra("filePath")
        if (filePath == null) {
            Log.e(TAG, "No filePath provided")
            finish()
            return
        }

        WindowCompat.setDecorFitsSystemWindows(window, false)

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
    }

    override fun onDestroy() {
        if (isSessionActive) {
            endPreviewSession()
        }
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

    internal fun endPreviewSession() {
        isSessionActive = false
        previewFilePath = null
        Thread { ColorGradingJniBridge.endPreview() }.start()
    }

    internal fun extractFilePathFromUrl(url: String): String? {
        val prefix = "http://image-preview.localhost/"
        if (!url.startsWith(prefix)) {
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

private class NativeColorGradingPreviewBridge(
    activity: ColorGradingActivity,
    private val filePath: String,
) {
    private val activityRef: WeakReference<ColorGradingActivity> = WeakReference(activity)

    @JavascriptInterface
    fun beginPreview(filePath: String) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "beginPreview: $filePath (JNI)")
        Thread {
            val result = ColorGradingJniBridge.beginPreview(filePath)
            activity.runOnUiThread {
                if (result.isSuccess) {
                    activity.isSessionActive = true
                    activity.webView?.evaluateJavascript("window.onPreviewReady?.();", null)
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "解码失败"
                    activity.webView?.evaluateJavascript(
                        "window.onPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            }
        }.start()
    }

    @JavascriptInterface
    fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset (JNI)")
        Thread {
            val result = ColorGradingJniBridge.applyPreview(lutId, meteringMode, evOffset)
            activity.runOnUiThread {
                if (result.isSuccess) {
                    val url = result.getOrDefault("")
                    val extractedPath = activity.extractFilePathFromUrl(url)
                    if (extractedPath != null) {
                        activity.previewFilePath = extractedPath
                        activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
                    } else {
                        activity.webView?.evaluateJavascript(
                            "window.notifyPreviewError?.(${JSONObject.quote("Invalid preview URL: $url")});", null
                        )
                    }
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "应用失败"
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            }
        }.start()
    }

    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        activity.isSessionActive = false
        activity.previewFilePath = null

        Thread {
            ColorGradingJniBridge.endPreview()
            val mainActivity = MainActivity.instance
            if (mainActivity != null) {
                val args = buildColorGradingArgsJson(filePath, lutId, meteringMode, evOffset, false)
                mainActivity.runOnUiThread {
                    mainActivity.getWebView()?.evaluateJavascript(
                        "(async function(){ try { await window.__tauriTriggerColorGrading?.(...${args}); } catch(e) {} " +
                        "try { window.__tauriSaveColorGradingLastUsed?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset}); } catch(e) {} })();",
                        null
                    )
                }
            }
        }.start()

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
        Thread { ColorGradingJniBridge.endPreview() }.start()
        activity.runOnUiThread { activity.finish() }
    }

    @JavascriptInterface
    fun getConfig(): String {
        // Get presets directly from JNI (no WebView dependency)
        val presetsJson = ColorGradingJniBridge.getPresets()

        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            return JSONObject().apply {
                put("filePath", filePath)
                put("presets", JSONArray(presetsJson))
            }.toString()
        }

        val resultFuture = java.util.concurrent.CompletableFuture<String>()
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{var l=window.__tauriGetColorGradingLastUsed?.()??'null';return JSON.stringify({lastUsed:l})}catch(e){return JSON.stringify({lastUsed:'null'})}})();"
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
                put("presets", JSONArray(presetsJson))
            }.toString()
        }

        val trimmed = raw.trim()
        val outerStr = if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
            try { JSONArray("[$trimmed]").getString(0) } catch (_: Exception) { trimmed.removeSurrounding("\"") }
        } else {
            trimmed
        }

        val json = try { JSONObject(outerStr) } catch (e: Exception) { JSONObject() }

        val lastUsedStr = json.optString("lastUsed", "null")
        val lastUsedDecoded = if (lastUsedStr.startsWith("\"")) {
            try { lastUsedStr.removeSurrounding("\"").replace("\\\"", "\"") } catch (_: Exception) { lastUsedStr }
        } else {
            lastUsedStr
        }
        val lastUsed = if (lastUsedDecoded != "null" && lastUsedDecoded.isNotEmpty()) {
            try { JSONObject(lastUsedDecoded) } catch (e: Exception) { null }
        } else null

        return JSONObject().apply {
            put("filePath", filePath)
            put("lastUsed", lastUsed ?: JSONObject.NULL)
            put("presets", JSONArray(presetsJson))
        }.toString()
    }

    private fun buildColorGradingArgsJson(
        filePath: String, lutId: String, meteringMode: String, evOffset: Float, syncToAuto: Boolean
    ): String {
        return "[${JSONObject.quote(filePath)},${JSONObject.quote(lutId)},${JSONObject.quote(meteringMode)},$evOffset,$syncToAuto]"
    }

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"
    }
}
