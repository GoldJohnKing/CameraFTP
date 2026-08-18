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
import com.gjk.cameraftpcompanion.ImageViewerActivity
import com.gjk.cameraftpcompanion.MainActivity
import java.lang.ref.WeakReference

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

/**
 * Resolved model options + default selection for the native AI edit dialog.
 * Pure derivation over the caller-supplied model list — no hardcoded catalog.
 */
internal data class AiEditModelSelection(
    val selectedModel: String,
    val selectedLabel: String,
    val optionsHtml: String,
)

/**
 * Build the model dropdown options HTML and resolve the default selection.
 * Selection semantics: the entry matching [currentModel] wins; otherwise the
 * first entry is selected. An empty [models] list degrades to a single option
 * built from [currentModel] so the dialog stays functional.
 */
internal fun buildAiEditModelSelection(
    models: List<Pair<String, String>>,
    currentModel: String,
): AiEditModelSelection {
    val effectiveModels = if (models.isEmpty()) listOf(currentModel to currentModel) else models
    val selectedModel = effectiveModels.map { it.first }
        .firstOrNull { it == currentModel }
        ?: effectiveModels.first().first
    val optionsHtml = effectiveModels.joinToString("") { (value, label) ->
        val sel = if (value == selectedModel) " selected" else ""
        """<div class="dropdown-opt$sel" data-value="$value">$label</div>"""
    }
    val selectedLabel = effectiveModels.first { it.first == selectedModel }.second
    return AiEditModelSelection(selectedModel, selectedLabel, optionsHtml)
}

class WebViewOverlayController(private val activity: ImageViewerActivity) {

    private companion object {
        private const val TAG = "WebViewOverlayController"
    }

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

    fun showAiEditPrompt(
        filePath: String,
        currentPrompt: String,
        currentModel: String,
        autoEditEnabled: Boolean,
        hasApiKey: Boolean,
        /**
         * Model catalog for the dropdown, passed in from the frontend via the
         * __tauriGetAiEditPrompt callback (single source: Rust
         * SEEDREAM_MODELS in ai_edit/config.rs, exported to the frontend by
         * gen-types as bindings/SeedreamModels.ts).
         * Each pair is (modelId, displayLabel).
         */
        models: List<Pair<String, String>>,
        mainActivity: MainActivity,
    ) {
        lockOrientation()
        val rootView = activity.findViewById<FrameLayout>(android.R.id.content)

        dismissAiEditPrompt()

        val escapedPrompt = android.text.TextUtils.htmlEncode(currentPrompt)
            .replace("\n", "&#10;")

        val selection = buildAiEditModelSelection(models, currentModel)
        val selectedModel = selection.selectedModel
        val modelOptionHtml = selection.optionsHtml
        val selectedLabel = selection.selectedLabel

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
        dismissAiEditPrompt()
    }
}
