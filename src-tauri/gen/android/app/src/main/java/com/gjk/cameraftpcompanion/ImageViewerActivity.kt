/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.IntentSender
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.database.Cursor
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.animation.Animation
import android.view.animation.LinearInterpolator
import android.view.animation.TranslateAnimation
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.exifinterface.media.ExifInterface
import androidx.viewpager2.widget.ViewPager2
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import org.json.JSONArray
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

class ImageViewerActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ImageViewerActivity"
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"
        const val EXTRA_AI_EDIT_ENABLED = "ai_edit_enabled"
        /** Active instance, set by onResume/cleared by onDestroy for bridge access */
        var instance: ImageViewerActivity? = null
            private set

        @Volatile
        var isViewerVisible: Boolean = false
            private set

        fun start(context: Context, uris: List<String>, targetIndex: Int, aiEditEnabled: Boolean = false) {
            val intent = Intent(context, ImageViewerActivity::class.java).apply {
                putExtra(EXTRA_URIS, JSONArray(uris).toString())
                putExtra(EXTRA_TARGET_INDEX, targetIndex)
                putExtra(EXTRA_AI_EDIT_ENABLED, aiEditEnabled)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
        }

        data class NavigationTarget(
            val uris: List<String>,
            val targetIndex: Int,
        )

        data class ReuseNavigationPlan(
            val shouldReuseExisting: Boolean,
            val uris: List<String>,
            val safeTargetIndex: Int,
        )

        @JvmStatic
        fun buildNavigationTarget(allUris: List<String>, targetUri: String): NavigationTarget? {
            val targetIndex = allUris.indexOf(targetUri)
            return if (targetIndex >= 0) {
                NavigationTarget(allUris, targetIndex)
            } else {
                null
            }
        }

        @JvmStatic
        fun buildReuseNavigationPlan(
            hasVisibleReusableViewer: Boolean,
            targetUris: List<String>,
            targetIndex: Int,
        ): ReuseNavigationPlan? {
            if (targetUris.isEmpty()) {
                return null
            }

            return ReuseNavigationPlan(
                shouldReuseExisting = hasVisibleReusableViewer,
                uris = targetUris,
                safeTargetIndex = targetIndex.coerceIn(0, targetUris.lastIndex),
            )
        }

        @JvmStatic
        fun navigateOrStart(context: Context, uris: List<String>, targetIndex: Int, aiEditEnabled: Boolean = false) {
            val active = instance
            val hasVisibleReusableViewer = active != null && isViewerVisible && !active.isFinishing && !active.isDestroyed
            val plan = buildReuseNavigationPlan(hasVisibleReusableViewer, uris, targetIndex) ?: return

            if (plan.shouldReuseExisting && active != null) {
                active.navigateTo(plan.uris, plan.safeTargetIndex)
                return
            }

            start(context, plan.uris, plan.safeTargetIndex, aiEditEnabled)
        }

        /**
         * Triggers a MediaStore scan for a newly created file from any context.
         */
        @JvmStatic
        fun scanNewFile(context: Context, filePath: String) {
            android.media.MediaScannerConnection.scanFile(context, arrayOf(filePath), null, null)
        }
    }

    private lateinit var viewPager: ViewPager2
    private lateinit var bottomBar: LinearLayout
    private lateinit var filenameView: TextView
    private lateinit var exifParams: TextView
    private lateinit var exifDatetime: TextView
    private lateinit var btnAiEdit: ImageButton
    private lateinit var btnRotate: ImageButton
    private lateinit var btnDelete: ImageButton
    private lateinit var aiEditProgressContainer: FrameLayout
    private lateinit var aiEditProgressFill: FrameLayout
    private lateinit var aiEditProgressHighlight: View
    private lateinit var aiEditProgressEdge: View
    private lateinit var aiEditStatusText: TextView
    private lateinit var aiEditProgressText: TextView
    private lateinit var aiEditFailureText: TextView
    private lateinit var aiEditCancelBtn: TextView
    private var aiEditHighlightAnimation: TranslateAnimation? = null
    private var uris: MutableList<String> = mutableListOf()
    private var currentIndex: Int = 0
    private var isLandscape = false
    private var isBottomBarVisible = true
    private var pendingDeleteUri: String? = null
    private var isAiEditing = false
    private var promptWebView: WebView? = null

    private val deleteRequestLauncher = registerForActivityResult(
        ActivityResultContracts.StartIntentSenderForResult(),
    ) { result ->
        val uriString = pendingDeleteUri ?: return@registerForActivityResult
        pendingDeleteUri = null

        if (result.resultCode == Activity.RESULT_OK) {
            finalizeDeleteAfterConfirmation(uriString)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        hideSystemBars()
        setContentView(R.layout.activity_image_viewer)

        SubsamplingScaleImageView.setPreferredBitmapConfig(Bitmap.Config.ARGB_8888)

        uris = parseUrisFromIntent().toMutableList()
        currentIndex = intent.getIntExtra(EXTRA_TARGET_INDEX, 0)
        val aiEditEnabled = intent.getBooleanExtra(EXTRA_AI_EDIT_ENABLED, false)

        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        filenameView = findViewById(R.id.filename)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnAiEdit = findViewById(R.id.btn_ai_edit)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        aiEditProgressContainer = findViewById(R.id.ai_edit_progress_container)
        aiEditProgressFill = findViewById(R.id.ai_edit_progress_fill)
        aiEditProgressHighlight = findViewById(R.id.ai_edit_progress_highlight)
        aiEditProgressEdge = findViewById(R.id.ai_edit_progress_edge)
        aiEditStatusText = findViewById(R.id.ai_edit_status_text)
        aiEditProgressText = findViewById(R.id.ai_edit_progress_text)
        aiEditFailureText = findViewById(R.id.ai_edit_failure_text)
        aiEditCancelBtn = findViewById(R.id.ai_edit_cancel_btn)

        btnAiEdit.visibility = if (aiEditEnabled) View.VISIBLE else View.GONE

        setupViewPager()
        setupButtons()
        updateUI()
    }

    private fun setupViewPager() {
        val adapter = ImageViewerAdapter(uris) { toggleBottomBar() }
        viewPager.adapter = adapter
        viewPager.setCurrentItem(currentIndex, false)
        // Prefetch 1 adjacent page on each side (2 images total: previous + next)
        viewPager.offscreenPageLimit = 1
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                // Update adapter's current position for smart recycle logic
                (viewPager.adapter as? ImageViewerAdapter)?.currentPosition = position
                updateUI()
            }
        })
    }

    fun navigateTo(newUris: List<String>, targetIndex: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) {
                return@runOnUiThread
            }

            uris.clear()
            uris.addAll(newUris)

            if (uris.isEmpty()) {
                finish()
                return@runOnUiThread
            }

            val safeTargetIndex = targetIndex.coerceIn(0, uris.lastIndex)
            currentIndex = safeTargetIndex

            (viewPager.adapter as? ImageViewerAdapter)?.replaceUris(uris)
                ?: run {
                    setupViewPager()
                }

            viewPager.setCurrentItem(safeTargetIndex, false)
            updateUI()
        }
    }

    private fun toggleBottomBar() {
        isBottomBarVisible = !isBottomBarVisible
        if (isBottomBarVisible) {
            bottomBar.alpha = 0f
            bottomBar.visibility = View.VISIBLE
            bottomBar.animate().alpha(1f).setDuration(100).start()
        } else {
            bottomBar.animate()
                .alpha(0f)
                .setDuration(100)
                .withEndAction { bottomBar.visibility = View.GONE }
                .start()
        }
        updateProgressBarPosition()
    }

    private fun updateProgressBarPosition() {
        val lp = aiEditProgressContainer.layoutParams as? FrameLayout.LayoutParams ?: return
        val screenWidth = resources.displayMetrics.widthPixels
        lp.width = screenWidth * 2 / 3
        lp.gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
        lp.marginStart = 0
        lp.marginEnd = 0
        lp.bottomMargin = if (isBottomBarVisible && bottomBar.visibility != View.GONE) {
            val barHeight = bottomBar.height
            if (barHeight > 0) {
                barHeight + (bottomBar.layoutParams as? FrameLayout.LayoutParams)?.bottomMargin?.let { if (it > 0) it else 8.dpToPx() }!! + 12.dpToPx()
            } else {
                // bottomBar not laid out yet, use default spacing
                92.dpToPx()
            }
        } else {
            16.dpToPx()
        }
        aiEditProgressContainer.layoutParams = lp
    }

    private fun Int.dpToPx(): Int {
        return (this * resources.displayMetrics.density).toInt()
    }

    private fun setupButtons() {
        btnAiEdit.setOnClickListener {
            if (uris.isNotEmpty() && currentIndex in uris.indices) {
                triggerAiEditForCurrentImage()
            }
        }

        btnRotate.setOnClickListener {
            isLandscape = !isLandscape
            requestedOrientation = if (isLandscape) {
                ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            } else {
                ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            }
        }

        btnDelete.setOnClickListener {
            if (uris.isNotEmpty()) {
                deleteCurrentImage()
            }
        }
    }

    private fun updateUI() {
        updateFilenameAndExif()
    }

    private fun updateFilenameAndExif() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) {
            filenameView.text = ""
            exifParams.visibility = View.GONE
            exifDatetime.visibility = View.GONE
            return
        }

        val uri = Uri.parse(uris[currentIndex])
        queryMediaStoreInfo(uri)
    }

    private fun queryMediaStoreInfo(uri: Uri) {
        val projection = arrayOf(
            MediaStore.Images.Media.DISPLAY_NAME,
            MediaStore.Images.Media.DATE_TAKEN
        )

        try {
            val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
            cursor?.use {
                if (it.moveToFirst()) {
                    val displayName = it.getString(it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME))
                    val dateTaken = it.getLong(it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_TAKEN))

                    // Filename
                    filenameView.text = displayName ?: uri.lastPathSegment ?: ""

                    // Date taken
                    if (dateTaken > 0) {
                        val sdf = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.getDefault())
                        exifDatetime.text = sdf.format(Date(dateTaken))
                        exifDatetime.visibility = View.VISIBLE
                    } else {
                        exifDatetime.visibility = View.GONE
                    }

                    // Read EXIF params natively
                    readExifParams(uri)

                    return
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to query MediaStore for ${uri}", e)
        }

        // Fallback
        filenameView.text = uri.lastPathSegment ?: ""
        exifParams.visibility = View.GONE
        exifDatetime.visibility = View.GONE
    }

    private fun readExifParams(uri: Uri) {
        try {
            contentResolver.openInputStream(uri)?.use { stream ->
                val exif = ExifInterface(stream)
                val parts = mutableListOf<String>()

                exif.getAttributeInt(ExifInterface.TAG_PHOTOGRAPHIC_SENSITIVITY, -1).takeIf { it >= 0 }?.let {
                    parts.add("ISO $it")
                }

                exif.getAttributeDouble(ExifInterface.TAG_F_NUMBER, 0.0).takeIf { it > 0 }?.let {
                    parts.add("f/${"%.1f".format(it)}")
                }

                exif.getAttributeDouble(ExifInterface.TAG_EXPOSURE_TIME, 0.0).takeIf { it > 0 }?.let {
                    if (it < 1.0) {
                        val denom = (1.0 / it).roundToInt()
                        parts.add("1/${denom}s")
                    } else {
                        parts.add("%.1fs".format(it))
                    }
                }

                // Prefer 35mm equivalent focal length; fall back to native focal length
                val focalLength = exif.getAttributeInt(ExifInterface.TAG_FOCAL_LENGTH_IN_35MM_FILM, 0).takeIf { it > 0 }
                    ?: exif.getAttributeDouble(ExifInterface.TAG_FOCAL_LENGTH, 0.0).takeIf { it > 0 }?.roundToInt()
                focalLength?.let {
                    parts.add("${it}mm")
                }

                if (parts.isNotEmpty()) {
                    exifParams.text = parts.joinToString(" • ")
                    exifParams.visibility = View.VISIBLE
                } else {
                    exifParams.visibility = View.GONE
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read EXIF for $uri", e)
            exifParams.visibility = View.GONE
        }
    }

    /**
     * Called from JS bridge when EXIF data is available (enriches existing info)
     */
    fun onExifResult(exifJson: String?) {
        runOnUiThread {
            if (exifJson == null || exifJson == "null") return@runOnUiThread

            try {
                val exif = org.json.JSONObject(exifJson)
                val parts = mutableListOf<String>()

                exif.optInt("iso", -1).takeIf { it >= 0 }?.let {
                    parts.add("ISO $it")
                }
                exif.optString("aperture").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }
                exif.optString("shutterSpeed").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }
                exif.optString("focalLength").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }

                if (parts.isNotEmpty()) {
                    exifParams.text = parts.joinToString(" • ")
                    exifParams.visibility = View.VISIBLE
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
            }
        }
    }

    private fun triggerAiEditForCurrentImage() {
        val uriString = uris.getOrNull(currentIndex) ?: return
        val filePath = resolveUriToFilePath(uriString)

        if (filePath == null) {
            Log.w(TAG, "Cannot resolve file path for URI: $uriString")
            return
        }

        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available for AI edit")
            return
        }

        // Fetch current prompt and model from WebView config, then show WebView overlay dialog
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{return window.__tauriGetAiEditPrompt?.()??''}catch(e){return ''}})();"
            ) { result ->
                // evaluateJavascript JSON-encodes the JS return value.
                // JS returns JSON.stringify({prompt, model}), so the callback
                // gives us a double-encoded string like: "{\"prompt\":\"...\",\"model\":\"...\"}"
                // We decode the outer JSON string literal via JSONArray, then parse the inner JSON.
                val jsonString = try {
                    val trimmed = result?.trim() ?: ""
                    if (trimmed.startsWith("\"")) {
                        org.json.JSONArray("[$trimmed]").getString(0)
                    } else {
                        trimmed
                    }
                } catch (_: Exception) {
                    result?.trim()?.removeSurrounding("\"") ?: ""
                }
                val (currentPrompt, currentModel) = try {
                    val json = org.json.JSONObject(jsonString)
                    json.optString("prompt", "").replace("\\n", "\n") to json.optString("model", "")
                } catch (e: Exception) {
                    jsonString.replace("\\n", "\n") to ""
                }
                val autoEdit = try {
                    val json = org.json.JSONObject(jsonString)
                    json.optBoolean("autoEdit", false)
                } catch (_: Exception) { false }
                runOnUiThread { showPromptWebViewOverlay(filePath, currentPrompt, currentModel, autoEdit, mainActivity) }
            }
        }
    }

    private fun showPromptWebViewOverlay(filePath: String, currentPrompt: String, currentModel: String, autoEditEnabled: Boolean, mainActivity: MainActivity) {
        val rootView = findViewById<FrameLayout>(android.R.id.content)

        // Dismiss any existing overlay
        dismissPromptWebView()

        val escapedPrompt = currentPrompt
            .replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("\"", "&quot;").replace("'", "&#39;")
            .replace("\n", "&#10;")

        // Determine which model option is selected
        val modelOptions = listOf(
            "doubao-seedream-5-0-260128" to "Doubao-Seedream-5.0-lite",
            "doubao-seedream-4-5-251128" to "Doubao-Seedream-4.5",
            "doubao-seedream-4-0-250828" to "Doubao-Seedream-4.0",
        )
        val selectedModel = currentModel.ifEmpty { modelOptions.first().first }
        val modelOptionHtml = modelOptions.joinToString("") { (value, label) ->
            val sel = if (value == selectedModel) " selected" else ""
            """<div class="dropdown-opt$sel" data-value="$value">$label</div>"""
        }
        val selectedLabel = modelOptions.find { it.first == selectedModel }?.second ?: selectedModel

        val saveToggleHtml = if (autoEditEnabled) {
            """<div class="save-toggle" onclick="toggleSave()">
                    <div class="toggle" id="saveToggle"></div>
                    <span>保存为自动修图设置</span>
                  </div>"""
        } else ""

        val html = """
            <!DOCTYPE html>
            <html>
            <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1">
            <style>
              * { margin: 0; padding: 0; box-sizing: border-box; }
              body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
              .overlay {
                position: fixed; inset: 0;
                background: rgba(0,0,0,0.5);
                display: flex; align-items: center; justify-content: center;
                padding: 16px; z-index: 50;
              }
              .card {
                background: #fff; border-radius: 12px; width: 100%; max-width: 448px;
                box-shadow: 0 25px 50px -12px rgba(0,0,0,0.25);
                display: flex; flex-direction: column; max-height: 90vh;
              }
              .header {
                display: flex; align-items: center; justify-content: space-between;
                padding: 16px; border-bottom: 1px solid #e5e7eb;
              }
              .title-group { display: flex; flex-direction: column; }
              .title { font-size: 18px; font-weight: 600; color: #111827; }
              .subtitle { font-size: 14px; color: #6b7280; margin-top: 2px; }
              .close-btn {
                padding: 8px; border: none; background: none; cursor: pointer;
                color: #9ca3af; border-radius: 8px;
              }
              .close-btn:hover { color: #4b5563; background: #f3f4f6; }
              .close-btn svg { width: 20px; height: 20px; }
              .content { padding: 16px; overflow-y: auto; }
              .field-group { margin-bottom: 12px; }
              .field-group:last-child { margin-bottom: 0; }
              .field-label { font-size: 12px; font-weight: 500; color: #6b7280; margin-bottom: 4px; }
              textarea {
                width: 100%; padding: 8px 12px; border: 1px solid #e5e7eb;
                border-radius: 8px; font-size: 14px; color: #374151;
                background: #fff; resize: none; outline: none;
                font-family: inherit; line-height: 1.5;
              }
              textarea:focus { border-color: transparent; box-shadow: 0 0 0 2px #3b82f6; }
              .dropdown { position: relative; }
              .dropdown-btn {
                width: 100%; padding: 8px 12px; border: 1px solid #e5e7eb;
                border-radius: 8px; font-size: 14px; color: #374151;
                background: #fff; outline: none; cursor: pointer;
                display: flex; align-items: center; justify-content: space-between;
                text-align: left; -webkit-user-select: none; user-select: none;
                -webkit-tap-highlight-color: transparent;
              }
              .dropdown-btn:hover { border-color: #d1d5db; }
              .dropdown-btn .chevron {
                width: 16px; height: 16px; color: #9ca3af;
                transition: transform 0.2s; flex-shrink: 0;
              }
              .dropdown-btn.open .chevron { transform: rotate(180deg); }
              .dropdown-panel {
                position: absolute; left: 0; right: 0;
                margin-top: 4px; background: #fff; border: 1px solid #e5e7eb;
                border-radius: 8px; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.1), 0 4px 6px -4px rgba(0,0,0,0.1);
                padding: 4px 0; z-index: 10; max-height: 240px; overflow-y: auto;
                opacity: 0; transform: scaleY(0.95) translateY(-4px);
                transform-origin: top; pointer-events: none;
                transition: opacity 0.15s ease, transform 0.15s ease;
              }
              .dropdown-panel.open {
                opacity: 1; transform: scaleY(1) translateY(0);
                pointer-events: auto;
              }
              .dropdown-opt {
                padding: 8px 12px; font-size: 14px;
                color: #374151; cursor: pointer;
                -webkit-tap-highlight-color: transparent;
              }
              .dropdown-opt:hover { background: #f9fafb; }
              .dropdown-opt.selected { background: #eff6ff; color: #1d4ed8; font-weight: 500; }
          .footer {
            display: flex; align-items: center; justify-content: space-between;
            padding: 16px; border-top: 1px solid #e5e7eb;
          }
          .save-toggle { display: flex; align-items: center; gap: 8px; cursor: pointer; -webkit-tap-highlight-color: transparent; }
          .save-toggle span { font-size: 14px; color: #374151; font-weight: 500; }
          .toggle {
            position: relative; width: 44px; height: 24px;
            background: #d1d5db; border-radius: 12px;
            transition: background 0.2s; cursor: pointer; flex-shrink: 0;
          }
          .toggle.on { background: #2563eb; }
          .toggle::after {
            content: ''; position: absolute;
            width: 16px; height: 16px; background: #fff;
            border-radius: 50%; top: 4px; left: 4px;
            transition: transform 0.2s;
          }
          .toggle.on::after { transform: translateX(20px); }
          .actions { display: flex; gap: 8px; }
              .btn {
                padding: 8px 16px; border-radius: 8px; font-size: 14px;
                font-weight: 500; border: none; cursor: pointer;
              }
              .btn-cancel { background: #f3f4f6; color: #374151; }
              .btn-cancel:hover { background: #e5e7eb; }
              .btn-confirm { background: #2563eb; color: #fff; }
              .btn-confirm:hover { background: #1d4ed8; }
              .btn-confirm:disabled { opacity: 0.5; cursor: not-allowed; }
              .header-icon { color: #d97706; flex-shrink: 0; }
            </style>
            </head>
            <body>
            <div class="overlay" onclick="if(event.target===this)NativeBridge.onCancel()">
              <div class="card">
                <div class="header">
                  <div style="display:flex;align-items:center;gap:12px">
                    <div style="width:40px;height:40px;background:#f3f4f6;border-radius:8px;display:flex;align-items:center;justify-content:center"><svg class="header-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/><path d="M20 3v4"/><path d="M22 5h-4"/></svg></div>
                    <div class="title-group">
                      <div class="title">AI修图</div>
                      <div class="subtitle">使用生成式 AI 调整照片</div>
                    </div>
                  </div>
                  <button class="close-btn" onclick="NativeBridge.onCancel()">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </button>
                </div>
                <div class="content">
                  <div class="field-group">
                    <div class="field-label">模型</div>
                    <div class="dropdown" id="modelDropdown">
                      <button class="dropdown-btn" type="button" onclick="toggleDropdown()">
                        <span id="modelLabel">$selectedLabel</span>
                        <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                      </button>
                      <div class="dropdown-panel" id="modelPanel">$modelOptionHtml</div>
                    </div>
                  </div>
                  <div class="field-group">
                    <div class="field-label">提示词</div>
                    <textarea id="prompt" rows="4" placeholder="请输入提示词">${escapedPrompt}</textarea>
                  </div>
                </div>
                <div class="footer">
                  $saveToggleHtml
                  <div class="actions" style="margin-left:auto">
                    <button class="btn btn-cancel" onclick="NativeBridge.onCancel()">取消</button>
                    <button class="btn btn-confirm" id="confirmBtn" onclick="onConfirm()" disabled>确认</button>
                  </div>
                </div>
              </div>
            </div>
            <script>
              var saveAsAutoEdit = false;
              var selectedModel = '$selectedModel';
              function toggleSave() {
                saveAsAutoEdit = !saveAsAutoEdit;
                document.getElementById('saveToggle').className = 'toggle' + (saveAsAutoEdit ? ' on' : '');
              }
              function toggleDropdown() {
                var panel = document.getElementById('modelPanel');
                var btn = panel.previousElementSibling;
                var isOpen = panel.classList.contains('open');
                if (isOpen) {
                  panel.classList.remove('open');
                  btn.classList.remove('open');
                } else {
                  panel.classList.add('open');
                  btn.classList.add('open');
                }
              }
              function closeDropdown() {
                var panel = document.getElementById('modelPanel');
                var btn = panel.previousElementSibling;
                panel.classList.remove('open');
                btn.classList.remove('open');
              }
              document.getElementById('modelPanel').addEventListener('click', function(e) {
                var opt = e.target.closest('.dropdown-opt');
                if (!opt) return;
                selectedModel = opt.getAttribute('data-value');
                document.getElementById('modelLabel').textContent = opt.textContent;
                var allOpts = this.querySelectorAll('.dropdown-opt');
                for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
                opt.classList.add('selected');
                closeDropdown();
              });
              document.addEventListener('click', function(e) {
                if (!document.getElementById('modelDropdown').contains(e.target)) {
                  closeDropdown();
                }
              });
              function onConfirm() {
                var prompt = document.getElementById('prompt').value.trim();
                if (!prompt) return;
                NativeBridge.onConfirm(prompt, selectedModel, saveAsAutoEdit);
              }
              function updateConfirmBtn() {
                var prompt = document.getElementById('prompt').value.trim();
                document.getElementById('confirmBtn').disabled = !prompt;
              }
              document.getElementById('prompt').addEventListener('input', updateConfirmBtn);
              updateConfirmBtn();
              document.getElementById('prompt').focus();
            </script>
            </body>
            </html>
        """.trimIndent()

        val webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            setBackgroundColor(0)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            addJavascriptInterface(object {
                @JavascriptInterface
                fun onConfirm(prompt: String, model: String, saveAsAutoEdit: Boolean) {
                    runOnUiThread {
                        dismissPromptWebView()
                        dispatchAiEdit(filePath, prompt, model, saveAsAutoEdit, mainActivity)
                    }
                }
                @JavascriptInterface
                fun onCancel() {
                    runOnUiThread { dismissPromptWebView() }
                }
            }, "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        promptWebView = webView
    }

    private fun dismissPromptWebView() {
        promptWebView?.let {
            (it.parent as? FrameLayout)?.removeView(it)
            it.destroy()
        }
        promptWebView = null
    }

    private fun dispatchAiEdit(filePath: String, prompt: String, model: String, saveAsAutoEdit: Boolean, mainActivity: MainActivity) {
        isAiEditing = true

        val escapedPath = filePath.replace("\\", "\\\\").replace("'", "\\'")
        val escapedPrompt = prompt.replace("\\", "\\\\").replace("'", "\\'")
            .replace("\n", "\\n").replace("\r", "\\r")
        val escapedModel = model.replace("\\", "\\\\").replace("'", "\\'")
        val saveFlag = if (saveAsAutoEdit) "true" else "false"

        val js = """
            (function() {
                if (window.__tauriTriggerAiEditWithPrompt) {
                    window.__tauriTriggerAiEditWithPrompt('$escapedPath', '$escapedPrompt', '$escapedModel', $saveFlag);
                    return 'ok';
                }
                return 'no_handler';
            })();
        """.trimIndent()

        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                if (result?.trim()?.removeSurrounding("\"") == "no_handler") {
                    runOnUiThread {
                        Log.w(TAG, "AI edit failed: frontend handler not available")
                        isAiEditing = false
                    }
                }
            }
        }
    }

    private fun resolveUriToFilePath(uriString: String): String? {
        return try {
            val uri = Uri.parse(uriString)
            if (uri.scheme == "file") {
                uri.path
            } else if (uri.scheme == "content") {
                contentResolver.query(uri, arrayOf(MediaStore.Images.Media.DATA), null, null, null)?.use { cursor ->
                    if (cursor.moveToFirst()) {
                        val idx = cursor.getColumnIndex(MediaStore.Images.Media.DATA)
                        if (idx >= 0) cursor.getString(idx) else null
                    } else null
                }
            } else {
                uriString
            }
        } catch (e: Exception) {
            Log.e(TAG, "resolveUriToFilePath failed for $uriString", e)
            null
        }
    }

    private fun resetProgressAppearance() {
        aiEditProgressContainer.background = ContextCompat.getDrawable(this, R.drawable.progress_bar_bg)
        aiEditProgressFill.setBackgroundColor(0)
        aiEditProgressFill.background = ContextCompat.getDrawable(this, R.drawable.progress_bar_fill)
        aiEditProgressEdge.setBackgroundColor(0)
        aiEditProgressEdge.background = ContextCompat.getDrawable(this, R.drawable.progress_bar_edge)
        aiEditStatusText.setTextColor(0xFF60A5FA.toInt())
    }

    /**
     * Called from JS bridge when AI edit completes (success or failure)
     */
    fun onAiEditComplete(success: Boolean, message: String?) {
        runOnUiThread {
            isAiEditing = false
            stopHighlightSweepAnimation()
            if (isFinishing || isDestroyed) return@runOnUiThread
            if (success) {
                stopHighlightSweepAnimation()
                // Green container background
                aiEditProgressContainer.background = android.graphics.drawable.GradientDrawable().apply {
                    shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                    setColor(0xBF1A3C1A.toInt())
                    cornerRadius = 12f * resources.displayMetrics.density
                    setStroke(1 * resources.displayMetrics.density.toInt(), 0x3322C55E.toInt())
                }
                // Green fill overlay — static gradient matching editing style
                aiEditProgressFill.layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                aiEditProgressFill.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(0x0022C55E.toInt(), 0x2622C55E.toInt(), 0x2622C55E.toInt(), 0x0022C55E.toInt())
                )
                aiEditProgressFill.visibility = View.VISIBLE
                aiEditProgressHighlight.visibility = View.GONE
                // Green full-width bottom edge — static gradient
                aiEditProgressEdge.layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    2,
                    Gravity.BOTTOM
                )
                aiEditProgressEdge.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT,
                    intArrayOf(0x004ADE80.toInt(), 0x994ADE80.toInt(), 0x994ADE80.toInt(), 0x004ADE80.toInt())
                )
                // Text
                aiEditStatusText.text = "修图完成"
                aiEditStatusText.setTextColor(0xFFFFFFFF.toInt())
                aiEditProgressText.text = message ?: "修图完成"
                aiEditFailureText.visibility = View.GONE
                // Dismiss button
                aiEditCancelBtn.visibility = View.VISIBLE
                aiEditCancelBtn.text = "✕"
                aiEditCancelBtn.setOnClickListener {
                    resetProgressAppearance()
                    aiEditProgressContainer.visibility = View.GONE
                }
            } else {
                // Red container background
                aiEditProgressContainer.background = android.graphics.drawable.GradientDrawable().apply {
                    shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                    setColor(0xB35C1A1A.toInt())
                    cornerRadius = 12f * resources.displayMetrics.density
                    setStroke(1 * resources.displayMetrics.density.toInt(), 0x33EF4444.toInt())
                }
                // Red fill overlay
                aiEditProgressFill.layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    FrameLayout.LayoutParams.MATCH_PARENT
                )
                aiEditProgressFill.setBackgroundColor(0x33EF4444.toInt())
                aiEditProgressFill.visibility = View.VISIBLE
                // Red full-width bottom edge
                aiEditProgressEdge.layoutParams = FrameLayout.LayoutParams(
                    FrameLayout.LayoutParams.MATCH_PARENT,
                    2,
                    Gravity.BOTTOM
                )
                aiEditProgressEdge.setBackgroundColor(0x99F87171.toInt())
                // Text
                aiEditStatusText.text = "修图完成"
                aiEditStatusText.setTextColor(0xFFFFFFFF.toInt())
                aiEditProgressText.text = message ?: "修图失败"
                aiEditFailureText.visibility = View.GONE
                // Dismiss button
                aiEditCancelBtn.text = "✕"
                aiEditCancelBtn.setOnClickListener {
                    resetProgressAppearance()
                    aiEditProgressContainer.visibility = View.GONE
                }
            }
        }
    }

    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread

            resetProgressAppearance()
            aiEditProgressContainer.visibility = View.VISIBLE
            aiEditStatusText.visibility = View.VISIBLE
            aiEditStatusText.text = "AI修图中..."
            aiEditCancelBtn.visibility = View.VISIBLE
            aiEditCancelBtn.text = "取消"
            aiEditCancelBtn.setOnClickListener {
                val mainActivity = MainActivity.instance
                mainActivity?.runOnUiThread {
                    mainActivity.getWebView()?.evaluateJavascript(
                        "(function(){try{window.__tauriCancelAiEdit?.()}catch(e){}})();", null
                    )
                }
            }

            // Position after layout so bottomBar height is available
            aiEditProgressContainer.post { updateProgressBarPosition() }

            val percent = if (total > 0) (current * 100) / total else 0

            val containerWidth = aiEditProgressContainer.width
            if (containerWidth > 0) {
                applyProgressLayout(containerWidth, percent)
            } else {
                // Container not laid out yet — post to run after the next layout pass
                aiEditProgressContainer.post {
                    if (isFinishing || isDestroyed) return@post
                    val w = aiEditProgressContainer.width
                    if (w > 0) applyProgressLayout(w, percent)
                }
            }

            aiEditProgressText.text = "第${current}张/共${total}张"

            if (failedCount > 0) {
                aiEditFailureText.visibility = View.VISIBLE
                aiEditFailureText.text = "失败${failedCount}张"
            } else {
                aiEditFailureText.visibility = View.GONE
            }
        }
    }

    private fun applyProgressLayout(containerWidth: Int, percent: Int) {
        val fillWidth = (containerWidth * percent) / 100

        aiEditProgressFill.layoutParams = FrameLayout.LayoutParams(
            fillWidth.coerceAtLeast(8),
            FrameLayout.LayoutParams.MATCH_PARENT
        )

        val highlightWidth = (fillWidth * 0.4).toInt().coerceIn(20, containerWidth / 2)
        aiEditProgressHighlight.layoutParams = FrameLayout.LayoutParams(
            highlightWidth,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        aiEditProgressHighlight.visibility = View.VISIBLE

        aiEditProgressEdge.layoutParams = FrameLayout.LayoutParams(
            fillWidth.coerceAtLeast(8),
            2,
            Gravity.BOTTOM
        )

        startHighlightSweepAnimation(fillWidth)
    }

    private fun startHighlightSweepAnimation(fillWidth: Int) {
        // Cancel previous animation if any (e.g. progress updated)
        aiEditHighlightAnimation?.let {
            aiEditProgressHighlight.clearAnimation()
        }
        aiEditHighlightAnimation = null

        val highlightWidth = aiEditProgressHighlight.width
        if (highlightWidth <= 0) {
            // Highlight not laid out yet — post after layout
            aiEditProgressHighlight.post {
                if (isFinishing || isDestroyed) return@post
                val hw = aiEditProgressHighlight.width
                if (hw > 0) {
                    startHighlightSweepAnimationWithDimensions(hw, fillWidth)
                }
            }
            return
        }

        startHighlightSweepAnimationWithDimensions(highlightWidth, fillWidth)
    }

    private fun startHighlightSweepAnimationWithDimensions(highlightWidth: Int, fillWidth: Int) {
        val anim = TranslateAnimation(
            Animation.ABSOLUTE, (-highlightWidth).toFloat(),
            Animation.ABSOLUTE, fillWidth.toFloat(),
            Animation.ABSOLUTE, 0f,
            Animation.ABSOLUTE, 0f
        ).apply {
            duration = 2000
            interpolator = LinearInterpolator()
            repeatCount = Animation.INFINITE
        }
        aiEditProgressHighlight.startAnimation(anim)
        aiEditHighlightAnimation = anim
    }

    private fun stopHighlightSweepAnimation() {
        aiEditHighlightAnimation?.let {
            aiEditProgressHighlight.clearAnimation()
            aiEditProgressHighlight.visibility = View.GONE
        }
        aiEditHighlightAnimation = null
    }

    private fun syncAiEditProgressFromWebView() {
        val progress = com.gjk.cameraftpcompanion.bridges.ImageViewerBridge.lastProgress
        val editing = com.gjk.cameraftpcompanion.bridges.ImageViewerBridge.isAiEditing
        if (editing && progress != null) {
            isAiEditing = true
            updateAiEditProgress(progress.current, progress.total, progress.failedCount)
        }
    }

    private fun deleteCurrentImage() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        deleteCurrentImage(uris[currentIndex], allowDeleteConfirmation = true)
    }

    private fun deleteCurrentImage(uriString: String, allowDeleteConfirmation: Boolean) {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(uri)
            if (rowsDeleted > 0 || !stillExists) {
                applyDeleteSuccess(uriString)
            } else {
                Toast.makeText(this, "删除失败：文件不存在", Toast.LENGTH_SHORT).show()
            }
        } catch (e: Exception) {
            if (allowDeleteConfirmation) {
                if (e is SecurityException) {
                    val pendingIntent = MediaStore.createDeleteRequest(contentResolver, listOf(uri))
                    requestDeleteConfirmation(uriString, pendingIntent.intentSender)
                    return
                }
            }

            if (e is SecurityException) {
                if (!uriStillExists(uri)) {
                    applyDeleteSuccess(uriString)
                    return
                }

                Log.e(TAG, "No permission to delete image", e)
                Toast.makeText(this, "删除失败：无权限", Toast.LENGTH_SHORT).show()
                return
            }

            Log.e(TAG, "Failed to delete image", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    private fun finalizeDeleteAfterConfirmation(uriString: String) {
        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(uri)
            if (rowsDeleted > 0 || !stillExists) {
                applyDeleteSuccess(uriString)
                return
            }
        } catch (e: SecurityException) {
            if (!uriStillExists(uri)) {
                applyDeleteSuccess(uriString)
                return
            }

            Log.e(TAG, "Delete still blocked after confirmation", e)
            Toast.makeText(this, "删除失败：无权限", Toast.LENGTH_SHORT).show()
            return
        } catch (e: Exception) {
            Log.e(TAG, "Failed to finalize delete after confirmation", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
            return
        }

        Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
    }

    private fun applyDeleteSuccess(uriString: String) {
        val removedIndex = uris.indexOf(uriString)

        // Extract mediaId from URI (last segment of content://media/.../id)
        val mediaId = uriString.substringAfterLast("/")

        if (removedIndex >= 0) {
            uris.removeAt(removedIndex)

            if (removedIndex < currentIndex) {
                currentIndex -= 1
            } else if (currentIndex >= uris.size && uris.isNotEmpty()) {
                currentIndex = uris.size - 1
            }
        }

        notifyMediaLibraryDeleted(listOf(mediaId))

        if (uris.isEmpty()) {
            Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
            finish()
            return
        }

        (viewPager.adapter as? ImageViewerAdapter)?.replaceUris(uris)
        viewPager.setCurrentItem(currentIndex, false)
        updateUI()
        Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
    }

    private fun uriStillExists(uri: Uri): Boolean {
        return try {
            val cursor = contentResolver.query(uri, arrayOf(MediaStore.Images.Media._ID), null, null, null)
            cursor?.use { it.moveToFirst() } ?: false
        } catch (_: Exception) {
            false
        }
    }

    private fun requestDeleteConfirmation(uriString: String, intentSender: IntentSender) {
        pendingDeleteUri = uriString

        try {
            val request = IntentSenderRequest.Builder(intentSender).build()
            deleteRequestLauncher.launch(request)
        } catch (e: Exception) {
            pendingDeleteUri = null
            Log.e(TAG, "Failed to launch delete confirmation", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    private fun notifyMediaLibraryDeleted(deletedMediaIds: List<String>) {
        val mainActivity = MainActivity.instance ?: return

        // Note: Full refresh events removed - handled incrementally via gallery-items-deleted
        // Send incremental delete event to WebView (no full refresh, preserves scroll position)
        val deletedIdsJson = JSONArray(deletedMediaIds).toString()
        val deletePayload = "{\"mediaIds\":$deletedIdsJson,\"timestamp\":${System.currentTimeMillis()}}"
        mainActivity.emitWindowEvent("gallery-items-deleted", deletePayload)

        // Also refresh latest photo
        val refreshPayload = "{\"reason\":\"delete\",\"timestamp\":${System.currentTimeMillis()}}"
        mainActivity.emitWindowEvent("latest-photo-refresh-requested", refreshPayload)
    }

    private fun parseUrisFromIntent(): List<String> {
        val urisJson = intent.getStringExtra(EXTRA_URIS) ?: return emptyList()
        return try {
            val jsonArray = JSONArray(urisJson)
            (0 until jsonArray.length()).map { jsonArray.getString(it) }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse URIs from intent", e)
            emptyList()
        }
    }

    private fun hideSystemBars() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        val wasAiEditVisible = btnAiEdit.visibility

        setContentView(R.layout.activity_image_viewer)

        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        filenameView = findViewById(R.id.filename)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnAiEdit = findViewById(R.id.btn_ai_edit)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        aiEditProgressContainer = findViewById(R.id.ai_edit_progress_container)
        aiEditProgressFill = findViewById(R.id.ai_edit_progress_fill)
        aiEditProgressHighlight = findViewById(R.id.ai_edit_progress_highlight)
        aiEditProgressEdge = findViewById(R.id.ai_edit_progress_edge)
        aiEditStatusText = findViewById(R.id.ai_edit_status_text)
        aiEditProgressText = findViewById(R.id.ai_edit_progress_text)
        aiEditFailureText = findViewById(R.id.ai_edit_failure_text)
        aiEditCancelBtn = findViewById(R.id.ai_edit_cancel_btn)

        btnAiEdit.visibility = wasAiEditVisible

        setupViewPager()
        setupButtons()
        updateUI()
        syncAiEditProgressFromWebView()
        if (aiEditProgressContainer.visibility == View.VISIBLE) {
            updateProgressBarPosition()
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        finish()
    }

    override fun onResume() {
        super.onResume()
        instance = this
        isViewerVisible = true
        syncAiEditProgressFromWebView()
    }

    override fun onPause() {
        isViewerVisible = false
        super.onPause()
    }

    override fun onStart() {
        super.onStart()
        MainActivity.markActivityVisible()
    }

    override fun onStop() {
        MainActivity.markActivityHidden()
        super.onStop()
    }

    override fun onDestroy() {
        dismissPromptWebView()
        if (instance == this) {
            isViewerVisible = false
            instance = null
        }
        super.onDestroy()
    }
}
