/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.controllers

import android.content.Intent
import android.net.Uri
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import com.gjk.cameraftpcompanion.ImageViewerActivity
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.R
import org.json.JSONArray
import java.lang.ref.WeakReference

private class NativeColorGradingBridge(
    activity: ImageViewerActivity,
    private val filePath: String,
) {
    private val activityRef: WeakReference<ImageViewerActivity> = WeakReference(activity)

    @JavascriptInterface
    fun onConfirm(lutId: String, useAutoExposure: Boolean, meteringMode: String, manualEv: Float, syncToAuto: Boolean) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            activity.overlayController.dismissColorGrading()
            activity.dispatchColorGrading(
                filePath, lutId, useAutoExposure, meteringMode, manualEv, syncToAuto,
            )
        }
    }

    @JavascriptInterface
    fun onCancel() {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread { activity.overlayController.dismissColorGrading() }
    }
}

private class NativeAiEditBridge(
    activity: ImageViewerActivity,
    private val filePath: String,
    private val mainActivity: MainActivity,
) {
    private companion object {
        private const val TAG = "NativeAiEditBridge"
    }

    private val activityRef: WeakReference<ImageViewerActivity> = WeakReference(activity)

    @JavascriptInterface
    fun onConfirm(prompt: String, model: String, saveAsAutoEdit: Boolean, apiKey: String) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            activity.overlayController.dismissAiEditPrompt()
            activity.dispatchAiEdit(filePath, prompt, model, saveAsAutoEdit, apiKey, mainActivity)
        }
    }

    @JavascriptInterface
    fun onCancel() {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread { activity.overlayController.dismissAiEditPrompt() }
    }

    @JavascriptInterface
    fun openLink(url: String) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            try {
                activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
            } catch (e: Exception) {
                Log.w(TAG, "Failed to open external link: $url", e)
            }
        }
    }
}

class WebViewOverlayController(private val activity: ImageViewerActivity) {

    private companion object {
        private const val TAG = "WebViewOverlayController"
    }

    private var colorGradingWebView: WebView? = null
    private var promptWebView: WebView? = null
    private var savedOrientation: Int? = null

    private fun lockOrientation() {
        savedOrientation = activity.requestedOrientation
        activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_LOCKED
    }

    private fun restoreOrientation() {
        savedOrientation?.let { activity.requestedOrientation = it }
        savedOrientation = null
    }

    fun showColorGrading(
        filePath: String,
        autoColorGradingEnabled: Boolean,
        presets: List<Pair<String, String>>,
        lastUsedPresetId: String? = null,
        lastUsedAutoExposure: Boolean? = null,
        lastUsedMeteringMode: String? = null,
        lastUsedManualEv: Float? = null,
    ) {
        lockOrientation()
        val rootView = activity.findViewById<FrameLayout>(android.R.id.content)

        dismissColorGrading()

        val initialPresetId = lastUsedPresetId?.takeIf { id -> presets.any { it.first == id } } ?: presets.firstOrNull()?.first
            ?: run {
                Log.w(TAG, "No color grading presets available")
                android.widget.Toast.makeText(activity, "调色预设尚未加载", android.widget.Toast.LENGTH_SHORT).show()
                return
            }
        val initialPresetLabel = presets.find { it.first == initialPresetId }?.second ?: presets.first().second
        val presetOptionsHtml = presets.joinToString("") { (value, label) ->
            """<div class="dropdown-opt${if (value == initialPresetId) " selected" else ""}" data-value="$value">$label</div>"""
        }

        val autoExposureChecked = lastUsedAutoExposure ?: true
        val evValue = lastUsedManualEv ?: 0.0f
        val evDisplay = if (evValue > 0) "+${"%.1f".format(evValue)} EV" else "${"%.1f".format(evValue)} EV"
        val initialMetering = lastUsedMeteringMode ?: "highlight-safe"

        val saveToggleHtml = if (autoColorGradingEnabled) {
            """<div class="save-toggle" onclick="toggleSync()">
                    <div class="toggle" id="syncToggle"></div>
                    <span>同步到自动调色</span>
                  </div>"""
        } else ""

        val html = activity.assets.open("color_grading_dialog.html").bufferedReader().use { it.readText() }
            .replace("{{FIRST_ID}}", initialPresetId)
            .replace("{{FIRST_LABEL}}", initialPresetLabel)
            .replace("{{PRESET_OPTIONS}}", presetOptionsHtml)
            .replace("{{SAVE_TOGGLE}}", saveToggleHtml)
            .replace("{{AUTO_EXPOSURE_CHECKED}}", if (autoExposureChecked) "checked" else "")
            .replace("{{EV_VALUE}}", evValue.toString())
            .replace("{{EV_DISPLAY}}", evDisplay)
            .replace("{{SELECTED_METERING}}", initialMetering)

        val webView = WebView(activity).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            setBackgroundColor(0)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            addJavascriptInterface(NativeColorGradingBridge(activity, filePath), "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        colorGradingWebView = webView
    }

    fun dismissColorGrading() {
        colorGradingWebView?.let {
            (it.parent as? FrameLayout)?.removeView(it)
            it.destroy()
        }
        colorGradingWebView = null
        restoreOrientation()
    }

    fun showAiEditPrompt(
        filePath: String,
        currentPrompt: String,
        currentModel: String,
        autoEditEnabled: Boolean,
        hasApiKey: Boolean,
        mainActivity: MainActivity,
    ) {
        lockOrientation()
        val rootView = activity.findViewById<FrameLayout>(android.R.id.content)

        dismissAiEditPrompt()

        val escapedPrompt = android.text.TextUtils.htmlEncode(currentPrompt)
            .replace("\n", "&#10;")

        val modelOptions = listOf(
            "doubao-seedream-5-0-260128" to "Doubao-Seedream-5.0-lite",
            "doubao-seedream-4-5-251128" to "Doubao-Seedream-4.5",
            "doubao-seedream-4-0-250828" to "Doubao-Seedream-4.0",
        )
        val selectedModel = modelOptions.map { it.first }
            .firstOrNull { it == currentModel }
            ?: modelOptions.first().first
        val modelOptionHtml = modelOptions.joinToString("") { (value, label) ->
            val sel = if (value == selectedModel) " selected" else ""
            """<div class="dropdown-opt$sel" data-value="$value">$label</div>"""
        }
        val selectedLabel = modelOptions.first { it.first == selectedModel }.second

        val saveToggleHtml = if (autoEditEnabled) {
            """<div class="save-toggle" onclick="toggleSave()">
                    <div class="toggle" id="saveToggle"></div>
                    <span>保存为自动修图设置</span>
                  </div>"""
        } else ""

        val apiKeyHtml = if (!hasApiKey) {
            """
            <div class="field-group">
              <div class="field-label">火山引擎 API Key</div>
              <div style="position:relative">
                <input type="text" id="apiKey" autocomplete="off" placeholder="输入火山引擎 API Key" />
                <button type="button" class="eye-btn" onmousedown="event.preventDefault()" onclick="toggleApiKeyVisibility()">
                  <svg id="eyeIcon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>
                </button>
              </div>
              <a href="#" class="api-link" onclick="event.preventDefault();NativeBridge.openLink('https://www.volcengine.com/docs/82379/1399008')">开通火山引擎模型服务 <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg></a>
            </div>
            """
        } else ""

        val html = activity.assets.open("ai_edit_dialog.html").bufferedReader().use { it.readText() }
            .replace("{{ESCAPED_PROMPT}}", escapedPrompt)
            .replace("{{SELECTED_MODEL}}", selectedModel)
            .replace("{{SELECTED_LABEL}}", selectedLabel)
            .replace("{{MODEL_OPTIONS}}", modelOptionHtml)
            .replace("{{SAVE_TOGGLE}}", saveToggleHtml)
            .replace("{{API_KEY_HTML}}", apiKeyHtml)

        val webView = WebView(activity).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            setBackgroundColor(0)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            addJavascriptInterface(NativeAiEditBridge(activity, filePath, mainActivity), "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        promptWebView = webView
    }

    fun dismissAiEditPrompt() {
        promptWebView?.let {
            (it.parent as? FrameLayout)?.removeView(it)
            it.destroy()
        }
        promptWebView = null
        restoreOrientation()
    }

    fun dismissAll() {
        dismissColorGrading()
        dismissAiEditPrompt()
    }
}
