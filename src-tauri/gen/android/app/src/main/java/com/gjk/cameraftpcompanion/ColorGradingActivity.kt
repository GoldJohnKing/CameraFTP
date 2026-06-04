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
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import com.gjk.cameraftpcompanion.bridges.ColorGradingJniBridge
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    internal var webView: WebView? = null

    @Volatile
    internal var previewJpegBytes: ByteArray? = null

    @Volatile
    internal var isSessionActive = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        onBackPressedDispatcher.addCallback(this, object : androidx.activity.OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (isSessionActive) {
                    endPreviewSession()
                }
                finish()
            }
        })

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
                        val bytes = previewJpegBytes
                        if (bytes != null && bytes.isNotEmpty()) {
                            return WebResourceResponse(
                                "image/jpeg", null, 200, "OK",
                                mapOf("Content-Length" to bytes.size.toString()),
                                java.io.ByteArrayInputStream(bytes)
                            )
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
        // Unconditionally end the preview session — covers both the case where
        // a session is active AND the case where beginPreview is still decoding
        // (isSessionActive is set to true only after successful decode).
        if (isSessionActive) {
            endPreviewSession()
        } else {
            // No active session yet, but a beginPreview thread might be running.
            // end() is a no-op when there's no session, so this is safe.
            Thread { ColorGradingJniBridge.endPreview() }.start()
        }
        webView?.let {
            (it.parent as? android.view.ViewGroup)?.removeView(it)
            it.destroy()
        }
        webView = null
        super.onDestroy()
    }

    internal fun endPreviewSession() {
        isSessionActive = false
        previewJpegBytes = null
        Thread { ColorGradingJniBridge.endPreview() }.start()
    }

}

internal class NativeColorGradingPreviewBridge(
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
        val maxWidth = activity.resources.displayMetrics.widthPixels
        val maxHeight = activity.resources.displayMetrics.heightPixels
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset size=${maxWidth}x${maxHeight} (JNI)")
        Thread {
            val result = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset, maxWidth, maxHeight)
            activity.runOnUiThread {
                if (result.isSuccess) {
                    activity.previewJpegBytes = result.getOrDefault(ByteArray(0))
                    activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
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

        activity.previewJpegBytes = null

        Thread {
            // Generate output filename matching batch processing path convention
            val outputPath = generateSavePath(lutId)
            if (outputPath == null) {
                Log.e(TAG, "save: failed to generate save path")
                activity.runOnUiThread {
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote("保存路径生成失败")});", null
                    )
                }
                return@Thread
            }

            Log.d(TAG, "save: outputPath=$outputPath")
            val result = ColorGradingJniBridge.commitPreview(lutId, true, meteringMode, evOffset, outputPath)

            activity.runOnUiThread {
                if (result.isSuccess) {
                    Log.d(TAG, "save: committed successfully to $outputPath")
                    // Notify Tauri frontend about the new graded file
                    val mainActivity = MainActivity.instance
                    if (mainActivity != null) {
                        mainActivity.getWebView()?.evaluateJavascript(
                            "(async function(){ try { await window.__TAURI__.invoke('notify_color_grading_done',{outputPaths:[${JSONObject.quote(outputPath)}]}); } catch(e) { console.warn('notify_color_grading_done error:',e); } })();",
                            null
                        )
                        // Save last-used settings
                        mainActivity.getWebView()?.evaluateJavascript(
                            "try { window.__tauriSaveColorGradingLastUsed?.(${JSONObject.quote(lutId)},${JSONObject.quote(meteringMode)},${evOffset}); } catch(e) {}",
                            null
                        )
                    } else {
                        Log.w(TAG, "save: MainActivity not available — notification skipped")
                    }
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "保存失败"
                    Log.e(TAG, "save: failed - $msg")
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            }
        }.start()

        // Delay finish to let the save thread start
        activity.runOnUiThread {
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                activity.finish()
            }, 200)
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
        val presetsJson = ColorGradingJniBridge.getPresets()

        // Read lastUsed directly from Rust config — no WebView IPC needed
        val lastUsedJson = ColorGradingJniBridge.getLastUsed()
        val lastUsed = if (lastUsedJson != null) {
            try { JSONObject(lastUsedJson) } catch (_: Exception) { null }
        } else null

        return JSONObject().apply {
            put("filePath", filePath)
            put("lastUsed", lastUsed ?: JSONObject.NULL)
            put("presets", JSONArray(presetsJson))
        }.toString()
    }

    /**
     * Generate a save path matching the batch processing convention:
     * <parent>/ColorGrading/<stem>_<presetId>_<timestamp>.jpg
     */
    private fun generateSavePath(lutId: String): String? {
        val activity = activityRef.get() ?: return null
        val original = java.io.File(filePath)
        if (!original.exists()) {
            Log.e(TAG, "generateSavePath: original file not found: $filePath")
            return null
        }
        val parent = original.parentFile ?: return null
        val stem = original.nameWithoutExtension
        val outputDir = java.io.File(parent, "ColorGrading")
        if (!outputDir.exists() && !outputDir.mkdirs()) {
            Log.e(TAG, "generateSavePath: failed to create ${outputDir.absolutePath}")
            return null
        }
        val timestamp = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.US).format(java.util.Date())
        val output = java.io.File(outputDir, "${stem}_${lutId}_${timestamp}.jpg")
        return output.absolutePath
    }

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"

        internal fun buildColorGradingArgsJson(
            filePath: String, lutId: String, meteringMode: String, evOffset: Float, syncToAuto: Boolean
        ): String {
            return "[${JSONObject.quote(filePath)},${JSONObject.quote(lutId)},${JSONObject.quote(meteringMode)},$evOffset,$syncToAuto]"
        }
    }
}
