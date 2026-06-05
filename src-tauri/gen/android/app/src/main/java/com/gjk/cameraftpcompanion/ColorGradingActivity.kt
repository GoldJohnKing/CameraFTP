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
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
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

    @Volatile
    internal var isApplyInFlight = false

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

        val insetsController = WindowCompat.getInsetsController(window, window.decorView)
        insetsController.hide(WindowInsetsCompat.Type.systemBars())
        insetsController.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE

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

    internal fun scanOutputFile(path: String) {
        android.media.MediaScannerConnection.scanFile(this, arrayOf(path), null, null)
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
        activity.isApplyInFlight = true
        Thread {
            val result = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset, maxWidth, maxHeight)
            activity.runOnUiThread {
                activity.isApplyInFlight = false
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
            Log.d(TAG, "save: calling commitPreview (JNI)")
            val result = ColorGradingJniBridge.commitPreview(lutId, true, meteringMode, evOffset)

            activity.runOnUiThread {
                if (result.isSuccess) {
                    val outputPath = result.getOrDefault("")
                    Log.d(TAG, "save: committed successfully to $outputPath")

                    // Save last-used config via JNI — no WebView dependency
                    ColorGradingJniBridge.saveLastUsed(lutId, meteringMode, evOffset)
                    Log.d(TAG, "save: saved last-used config via JNI")

                    // Insert into ImageViewerActivity directly — it may be paused (isViewerVisible=false)
                    // because ColorGradingActivity is in the foreground, but the instance is still alive.
                    // insertImage() posts to the viewer's UI thread via runOnUiThread, which will execute
                    // after ColorGradingActivity finishes and the viewer resumes.
                    val viewer = ImageViewerActivity.instance
                    if (viewer != null && !viewer.isFinishing && !viewer.isDestroyed) {
                        val file = java.io.File(outputPath)
                        if (file.exists()) {
                            val fileUri = android.net.Uri.fromFile(file).toString()
                            viewer.insertImage(fileUri, 0)
                            Log.d(TAG, "save: inserted $fileUri into ImageViewerActivity")
                        }
                    }

                    // MediaStore scan directly — no JS round-trip
                    activity.scanOutputFile(outputPath)

                    // Gallery refresh via WebView (best-effort — DOM events only)
                    val mainActivity = MainActivity.instance
                    mainActivity?.getWebView()?.evaluateJavascript(
                        """(function(){
                            setTimeout(function(){
                                window.dispatchEvent(new CustomEvent('gallery-refresh-requested',{detail:{reason:'color-grading'}}));
                                window.dispatchEvent(new CustomEvent('latest-photo-refresh-requested',{detail:{reason:'color-grading'}}));
                            },500);
                        })();""",
                        null
                    )

                    // Finish after all operations complete
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        activity.finish()
                    }, 500)
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "保存失败"
                    Log.e(TAG, "save: failed - $msg")
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                    // Re-apply preview so the user sees the image again
                    val maxWidth = activity.resources.displayMetrics.widthPixels
                    val maxHeight = activity.resources.displayMetrics.heightPixels
                    Thread {
                        val retryResult = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset, maxWidth, maxHeight)
                        activity.runOnUiThread {
                            if (retryResult.isSuccess) {
                                activity.previewJpegBytes = retryResult.getOrDefault(ByteArray(0))
                                activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
                            }
                        }
                    }.start()
                }
            }
        }.start()
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

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"
    }
}
