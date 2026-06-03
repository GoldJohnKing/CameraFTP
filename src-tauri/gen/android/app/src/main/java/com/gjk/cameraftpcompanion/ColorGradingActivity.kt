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
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.util.concurrent.atomic.AtomicLong

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    private var webView: WebView? = null
    internal var previewFilePath: String? = null
    private var isSessionActive = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val filePath = intent.getStringExtra("filePath")
        if (filePath == null) {
            Log.e(TAG, "No filePath provided")
            finish()
            return
        }

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

        setContentView(webView)
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

    private fun callMainWebView(js: String, callback: ((String?) -> Unit)? = null) {
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
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                callback?.invoke(result)
            }
        }
    }

    internal fun parseJsString(result: String?): String? {
        if (result == null) return null
        val trimmed = result.trim()
        return if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
            try {
                JSONArray("[$trimmed]").getString(0)
            } catch (e: Exception) {
                trimmed.removeSurrounding("\"")
            }
        } else {
            trimmed
        }
    }

    internal fun endPreviewSession() {
        isSessionActive = false
        previewFilePath = null
        callMainWebView(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} })();"
        )
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
        Log.d(TAG, "beginPreview: $filePath")
        activity.callMainWebView(
            "(async function(){ try { await window.__tauriBeginColorGradingPreview?.('${filePath.replace("'", "\\'")}'); return 'ok'; } catch(e) { return 'error:' + e.message; } })();"
        ) { result ->
            val parsed = activity.parseJsString(result)
            if (parsed?.startsWith("error:") == true) {
                val msg = parsed.substring(6)
                Log.e(TAG, "beginPreview failed: $msg")
                activity.runOnUiThread {
                    activity.webView?.evaluateJavascript(
                        "window.onPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            } else {
                Log.d(TAG, "beginPreview success")
                activity.isSessionActive = true
                activity.runOnUiThread {
                    activity.webView?.evaluateJavascript("window.onPreviewReady?.();", null)
                }
            }
        }
    }

    @JavascriptInterface
    fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        val myId = applyRequestId.incrementAndGet()
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset id=$myId")
        activity.callMainWebView(
            "(async function(){ try { var r = await window.__tauriApplyColorGradingPreview?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset}); return r || ''; } catch(e) { return 'error:' + e.message; } })();"
        ) { result ->
            if (myId != applyRequestId.get()) {
                Log.d(TAG, "Discarding stale apply result (expected $myId, current ${applyRequestId.get()})")
                return@callMainWebView
            }
            val parsed = activity.parseJsString(result)
            if (parsed?.startsWith("error:") == true) {
                val msg = parsed.substring(6)
                Log.e(TAG, "applyPreview failed: $msg")
                activity.runOnUiThread {
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            } else if (parsed != null) {
                Log.d(TAG, "applyPreview success: $parsed")
                activity.previewFilePath = parsed
                activity.runOnUiThread {
                    activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
                }
            }
        }
    }

    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        activity.isSessionActive = false
        activity.previewFilePath = null

        activity.callMainWebView(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} })();"
        ) {
            activity.callMainWebView(
                "(async function(){ try { await window.__tauriTriggerColorGrading?.('${filePath.replace("'", "\\'")}','${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset},false); } catch(e) {} })();"
            ) {
                activity.callMainWebView(
                    "window.__tauriSaveColorGradingLastUsed?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset});"
                ) {
                    activity.runOnUiThread { activity.finish() }
                }
            }
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

        val parsed = activity.parseJsString(raw) ?: raw
        val json = try { JSONObject(parsed) } catch (e: Exception) { JSONObject() }

        val lastUsedStr = json.optString("lastUsed", "null")
        val lastUsed = if (lastUsedStr != "null" && lastUsedStr.isNotEmpty()) {
            try { JSONObject(lastUsedStr) } catch (e: Exception) { null }
        } else null

        val presetsStr = json.optString("presets", "[]")
        val presetsArr = try { JSONArray(presetsStr) } catch (e: Exception) { JSONArray() }

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
