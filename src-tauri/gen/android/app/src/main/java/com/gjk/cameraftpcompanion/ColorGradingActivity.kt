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

    @Volatile
    internal var isSaving = false

    @Volatile
    internal var isDecoding = false

    @Volatile
    internal var cancelDecoding = false

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
        if (isSaving) {
            // save() has already called endPreview() + enqueueBatch().
            // Do nothing — the batch worker handles the rest.
        } else if (isDecoding) {
            cancelDecoding = true
        } else if (isSessionActive) {
            endPreviewSession()
        } else {
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
        val maxWidth = activity.resources.displayMetrics.widthPixels
        val maxHeight = activity.resources.displayMetrics.heightPixels
        Log.d(TAG, "beginPreview: $filePath halfSize=true ${maxWidth}x${maxHeight} (JNI)")
        activity.isDecoding = true
        Thread {
            val result = ColorGradingJniBridge.beginPreview(filePath, true, maxWidth, maxHeight)
            activity.runOnUiThread {
                activity.isDecoding = false
                if (result.isSuccess) {
                    if (activity.cancelDecoding) {
                        Thread { ColorGradingJniBridge.endPreview() }.start()
                    } else {
                        activity.isSessionActive = true
                        activity.webView?.evaluateJavascript("window.onPreviewReady?.();", null)
                    }
                } else {
                    if (!activity.cancelDecoding) {
                        val msg = result.exceptionOrNull()?.message ?: "解码失败"
                        activity.webView?.evaluateJavascript(
                            "window.onPreviewError?.(${JSONObject.quote(msg)});", null
                        )
                    }
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
            val result = ColorGradingJniBridge.applyPreview(lutId, meteringMode, evOffset, maxWidth, maxHeight)
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
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset — enqueuing batch job")

        // Mark as saving to prevent onDestroy from racing with endPreview
        activity.isSaving = true

        // Save last-used config via JNI
        ColorGradingJniBridge.saveLastUsed(lutId, meteringMode, evOffset)

        // End preview session in background (non-blocking)
        Thread { ColorGradingJniBridge.endPreview() }.start()
        activity.isSessionActive = false

        // Enqueue full-resolution processing via ColorGradingService worker.
        // enqueue() only sends to mpsc channel — returns in ~1ms.
        val result = ColorGradingJniBridge.enqueueBatch(filePath, lutId, meteringMode, evOffset)

        activity.runOnUiThread {
            if (result.isSuccess) {
                // Gallery refresh via WebView (best-effort)
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

                // Close immediately — batch processing continues in background.
                // Progress shown via TaskProgressPanel in ImageViewerActivity.
                // On completion: scanNewFile → insertImage + MediaStore scan.
                activity.finish()
            } else {
                activity.isSaving = false
                val msg = result.exceptionOrNull()?.message ?: "入队失败"
                Log.e(TAG, "save: enqueueBatch failed - $msg")
                android.widget.Toast.makeText(activity, msg, android.widget.Toast.LENGTH_LONG).show()
            }
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

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"
    }
}
