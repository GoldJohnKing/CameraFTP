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
import android.text.TextUtils
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
        private val RAW_EXTENSIONS = setOf(
            "nef", "nrw", "cr2", "cr3", "arw", "sr2",
            "raf", "orf", "rw2", "pef", "dng", "x3f", "raw", "srw"
        )
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"
        /** Active instance, set by onResume/cleared by onDestroy for bridge access */
        var instance: ImageViewerActivity? = null
            private set

        @Volatile
        var isViewerVisible: Boolean = false
            private set

        fun start(context: Context, uris: List<String>, targetIndex: Int) {
            val intent = Intent(context, ImageViewerActivity::class.java).apply {
                putExtra(EXTRA_URIS, JSONArray(uris).toString())
                putExtra(EXTRA_TARGET_INDEX, targetIndex)
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
        fun navigateOrStart(context: Context, uris: List<String>, targetIndex: Int) {
            val active = instance
            val hasVisibleReusableViewer = active != null && isViewerVisible && !active.isFinishing && !active.isDestroyed
            val plan = buildReuseNavigationPlan(hasVisibleReusableViewer, uris, targetIndex) ?: return

            if (plan.shouldReuseExisting && active != null) {
                active.navigateTo(plan.uris, plan.safeTargetIndex)
                return
            }

            start(context, plan.uris, plan.safeTargetIndex)
        }

        /**
         * Resolve a URI string to a file system path.
         * Handles file://, content:// (via MediaStore), and fallback.
         */
        @JvmStatic
        fun resolveUriToFilePath(context: android.content.Context, uriString: String): String? {
            return try {
                val uri = Uri.parse(uriString)
                when (uri.scheme) {
                    "file" -> uri.path
                    "content" -> context.contentResolver.query(uri, arrayOf(MediaStore.Images.Media.DATA), null, null, null)?.use { cursor ->
                        if (cursor.moveToFirst()) {
                            val idx = cursor.getColumnIndex(MediaStore.Images.Media.DATA)
                            if (idx >= 0) cursor.getString(idx) else null
                        } else null
                    }
                    else -> uriString
                }
            } catch (e: Exception) {
                Log.e(TAG, "resolveUriToFilePath failed for $uriString", e)
                null
            }
        }
    }

    private lateinit var viewPager: ViewPager2
    private lateinit var bottomBar: LinearLayout
    private lateinit var filenameView: TextView
    private lateinit var exifParams: TextView
    private lateinit var exifDatetime: TextView
    private lateinit var btnMenu: ImageButton
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
    private var currentDisplayName: String? = null
    private var currentIndex: Int = 0
    private var isLandscape = false
    private var isBottomBarVisible = true
    private var pendingDeleteUri: String? = null
    private var isAiEditing = false
    private var isColorGrading = false
    private var promptWebView: WebView? = null
    private var menuPopupWindow: android.widget.PopupWindow? = null
    private var colorGradingWebView: WebView? = null

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
        bindViews()

        setupViewPager()
        setupButtons()
        updateUI()
    }

    private fun bindViews() {
        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        filenameView = findViewById(R.id.filename)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnMenu = findViewById(R.id.btn_menu)
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
        btnMenu.setOnClickListener {
            showImageMenu()
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

    private fun isRawFileByExtension(displayName: String?): Boolean {
        val ext = displayName?.substringAfterLast('.', "")?.lowercase() ?: return false
        return ext in RAW_EXTENSIONS
    }

    private fun showImageMenu() {
        menuPopupWindow?.dismiss()

        val popupView = layoutInflater.inflate(R.layout.popup_image_menu, null)
        val menuItemAiEdit = popupView.findViewById<LinearLayout>(R.id.menu_item_ai_edit)
        val menuItemColorGrading = popupView.findViewById<LinearLayout>(R.id.menu_item_color_grading)
        val cgIcon = popupView.findViewById<android.widget.ImageView>(R.id.menu_item_color_grading_icon)
        val cgText = popupView.findViewById<TextView>(R.id.menu_item_color_grading_text)

        val isRaw = uris.isNotEmpty() && currentIndex in uris.indices && isRawFileByExtension(currentDisplayName)

        if (!isRaw) {
            menuItemColorGrading.isEnabled = false
            menuItemColorGrading.isClickable = false
            cgIcon.alpha = 0.35f
            cgText.alpha = 0.35f
        }

        menuItemAiEdit.setOnClickListener {
            menuPopupWindow?.dismiss()
            if (uris.isNotEmpty() && currentIndex in uris.indices) {
                triggerAiEditForCurrentImage()
            }
        }

        menuItemColorGrading.setOnClickListener {
            menuPopupWindow?.dismiss()
            if (uris.isNotEmpty() && currentIndex in uris.indices) {
                triggerColorGradingForCurrentImage()
            }
        }

        val popup = android.widget.PopupWindow(
            popupView,
            android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            true
        )
        popup.setBackgroundDrawable(null)
        popup.isOutsideTouchable = true
        popup.isFocusable = true
        popup.elevation = 16f

        popup.setOnDismissListener {
            menuPopupWindow = null
        }

        menuPopupWindow = popup
        popupView.measure(
            android.view.View.MeasureSpec.UNSPECIFIED,
            android.view.View.MeasureSpec.UNSPECIFIED
        )
        val yOffset = -(btnMenu.height + popupView.measuredHeight + 8.dpToPx())
        popup.showAsDropDown(btnMenu, 0, yOffset)
    }

    private fun triggerColorGradingForCurrentImage() {
        val uriString = uris.getOrNull(currentIndex) ?: return
        val filePath = resolveUriToFilePath(uriString)

        if (filePath == null) {
            Log.w(TAG, "Cannot resolve file path for URI: $uriString")
            return
        }

        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available for color grading config")
            showColorGradingOverlay(filePath, false)
            return
        }

        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{return window.__tauriGetAutoColorGradingEnabled?.()??'false'}catch(e){return 'false'}})();"
            ) { result ->
                val enabled = result?.trim()?.removeSurrounding("\"")?.toBoolean() ?: false
                showColorGradingOverlay(filePath, enabled)
            }
        }
    }

    private fun showColorGradingOverlay(filePath: String, autoColorGradingEnabled: Boolean) {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LOCKED
        val rootView = findViewById<FrameLayout>(android.R.id.content)

        dismissColorGradingWebView()

        val presets = listOf(
            "arri-alexa-classic-709" to "ARRI ALEXA Classic 709",
            "fujifilm-acros" to "Fujifilm ACROS",
            "fujifilm-astia" to "Fujifilm ASTIA",
            "fujifilm-classic-chrome" to "Fujifilm CLASSIC CHROME",
            "fujifilm-classic-neg" to "Fujifilm CLASSIC Neg",
            "fujifilm-eterna-3513di" to "Fujifilm ETERNA 3513DI",
            "fujifilm-eterna-bb" to "Fujifilm ETERNA BB",
            "fujifilm-eterna" to "Fujifilm ETERNA",
            "fujifilm-pro-neg-std" to "Fujifilm PRO Neg. Std",
            "fujifilm-provia" to "Fujifilm PROVIA",
            "fujifilm-reala-ace" to "Fujifilm REALA ACE",
            "fujifilm-velvia" to "Fujifilm Velvia",
            "kodak-vision-2383" to "Kodak VISION 2383",
            "leica-classic" to "Leica Classic",
            "leica-natural" to "Leica Natural",
            "red-achromic" to "RED Achromic",
            "red-filmbias-bb" to "RED FilmBias BB",
            "red-filmbias-offset" to "RED FilmBias Offset",
            "red-filmbias" to "RED FilmBias",
            "red-rec-709" to "RED Rec.709",
        )
        val firstId = presets.first().first
        val firstLabel = presets.first().second
        val presetOptionsHtml = presets.joinToString("") { (value, label) ->
            """<div class="dropdown-opt${if (value == firstId) " selected" else ""}" data-value="$value">$label</div>"""
        }

        val saveToggleHtml = if (autoColorGradingEnabled) {
            """<div class="save-toggle" onclick="toggleSync()">
                    <div class="toggle" id="syncToggle"></div>
                    <span>同步到自动调色</span>
                  </div>"""
        } else ""

        val html = """
            <!DOCTYPE html>
            <html>
            <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1">
            <style>
              * { margin: 0; padding: 0; box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
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
              .content { padding: 16px; overflow: visible; }
              .field-group { margin-bottom: 12px; }
              .field-group:last-child { margin-bottom: 0; }
              .field-label { font-size: 14px; font-weight: 500; color: #374151; margin-bottom: 4px; }
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
              .divider { border-top: 1px solid #f3f4f6; margin: 12px 0; }
              .toggle-row {
                display: flex; align-items: center; justify-content: space-between;
                padding: 4px 0;
              }
              .toggle-label-group { flex: 1; }
              .toggle-label { font-size: 14px; font-weight: 500; color: #374151; }
              .toggle-desc { font-size: 12px; color: #6b7280; margin-top: 2px; }
              .toggle-switch {
                position: relative; width: 44px; height: 24px; flex-shrink: 0; margin-left: 12px;
              }
              .toggle-switch input { opacity: 0; width: 0; height: 0; }
              .toggle-slider {
                position: absolute; inset: 0; background: #d1d5db; border-radius: 12px;
                transition: background 0.2s; cursor: pointer;
              }
              .toggle-slider:before {
                content: ''; position: absolute; width: 20px; height: 20px;
                left: 2px; bottom: 2px; background: #fff; border-radius: 50%;
                transition: transform 0.2s;
              }
              .toggle-switch input:checked + .toggle-slider { background: #2563eb; }
              .toggle-switch input:checked + .toggle-slider:before { transform: translateX(20px); }
              .slider-group { margin-top: 12px; }
              .slider-header {
                display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;
              }
              .slider-value { font-size: 13px; font-family: monospace; color: #6b7280; }
              input[type="range"] {
                -webkit-appearance: none; width: 100%; height: 6px;
                background: #e5e7eb; border-radius: 3px; outline: none;
              }
              input[type="range"]::-webkit-slider-thumb {
                -webkit-appearance: none; width: 20px; height: 20px;
                background: #2563eb; border-radius: 50%; cursor: pointer;
              }
              .slider-labels {
                display: flex; justify-content: space-between;
                font-size: 11px; color: #9ca3af; margin-top: 4px;
              }
              .footer {
                display: flex; align-items: center; justify-content: space-between;
                padding: 16px; border-top: 1px solid #e5e7eb;
              }
              .save-toggle { display: flex; align-items: center; gap: 8px; cursor: pointer; }
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
              .actions { display: flex; gap: 8px; margin-left: auto; }
              .btn {
                padding: 8px 16px; border-radius: 8px; font-size: 14px;
                font-weight: 500; border: none; cursor: pointer;
              }
              .btn-cancel { background: #f3f4f6; color: #374151; }
              .btn-cancel:hover { background: #e5e7eb; }
              .btn-confirm { background: #2563eb; color: #fff; }
              .btn-confirm:hover { background: #1d4ed8; }
              .header-icon { color: #7c3aed; flex-shrink: 0; }
            </style>
            </head>
            <body>
            <div class="overlay" onclick="if(event.target===this)NativeBridge.onCancel()">
              <div class="card">
                <div class="header">
                  <div style="display:flex;align-items:center;gap:12px">
                    <div style="width:40px;height:40px;background:#f3f4f6;border-radius:8px;display:flex;align-items:center;justify-content:center"><svg class="header-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="17.5" cy="10.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="8.5" cy="7.5" r="1.5" fill="currentColor" stroke="none"/><circle cx="6.5" cy="12.5" r="1.5" fill="currentColor" stroke="none"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/></svg></div>
                    <div class="title-group">
                      <div class="title">调色</div>
                      <div class="subtitle">使用胶片模拟调色处理 RAW 照片</div>
                    </div>
                  </div>
                  <button class="close-btn" onclick="NativeBridge.onCancel()">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </button>
                </div>
                <div class="content">
                  <div class="field-group">
                    <div class="field-label">调色预设</div>
                    <div class="dropdown" id="presetDropdown">
                      <button class="dropdown-btn" type="button" onclick="toggleDropdown()">
                        <span id="presetLabel">$firstLabel</span>
                        <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                      </button>
                      <div class="dropdown-panel" id="presetPanel">$presetOptionsHtml</div>
                    </div>
                  </div>
                  <div class="divider"></div>
                  <div class="toggle-row">
                    <div class="toggle-label-group">
                      <div class="toggle-label">自动曝光</div>
                      <div class="toggle-desc" id="exposureDesc">自动检测并调整曝光</div>
                    </div>
                    <label class="toggle-switch">
                      <input type="checkbox" id="autoExposureToggle" checked onchange="onExposureToggle()">
                      <span class="toggle-slider"></span>
                    </label>
                  </div>
                  <div class="slider-group" id="evSliderGroup" style="display:none">
                    <div class="slider-header">
                      <span class="field-label" style="margin-bottom:0">曝光补偿</span>
                      <span class="slider-value" id="evValue">0.0 EV</span>
                    </div>
                    <input type="range" id="evSlider" min="-5.0" max="5.0" step="0.1" value="0" oninput="onEvChange()">
                    <div class="slider-labels"><span>-5.0</span><span>0</span><span>+5.0</span></div>
                  </div>
                  <div id="meteringGroup" style="margin-top:12px">
                    <div class="field-group" style="margin-bottom:0">
                      <div class="field-label">测光模式</div>
                      <div class="dropdown" id="meteringDropdown">
                        <button class="dropdown-btn" type="button" onclick="toggleMeteringDropdown()">
                          <span id="meteringLabel">高光保护</span>
                          <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                        </button>
                        <div class="dropdown-panel" id="meteringPanel">
                          <div class="dropdown-opt selected" data-value="highlight-safe" onclick="selectMetering(this)">高光保护</div>
                          <div class="dropdown-opt" data-value="matrix" onclick="selectMetering(this)">矩阵测光</div>
                          <div class="dropdown-opt" data-value="center-weighted" onclick="selectMetering(this)">中央重点测光</div>
                          <div class="dropdown-opt" data-value="average" onclick="selectMetering(this)">平均测光</div>
                          <div class="dropdown-opt" data-value="hybrid" onclick="selectMetering(this)">混合测光</div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
                <div class="footer">
                  $saveToggleHtml
                  <div class="actions">
                    <button class="btn btn-cancel" onclick="NativeBridge.onCancel()">取消</button>
                    <button class="btn btn-confirm" onclick="onConfirm()">应用</button>
                  </div>
                </div>
              </div>
            </div>
            <script>
              var selectedPreset = '$firstId';
              function toggleDropdown() {
                var panel = document.getElementById('presetPanel');
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
                var panel = document.getElementById('presetPanel');
                var btn = panel.previousElementSibling;
                panel.classList.remove('open');
                btn.classList.remove('open');
              }
              document.getElementById('presetPanel').addEventListener('click', function(e) {
                var opt = e.target.closest('.dropdown-opt');
                if (!opt) return;
                selectedPreset = opt.getAttribute('data-value');
                document.getElementById('presetLabel').textContent = opt.textContent;
                var allOpts = this.querySelectorAll('.dropdown-opt');
                for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
                opt.classList.add('selected');
                closeDropdown();
              });
              document.addEventListener('click', function(e) {
                if (!document.getElementById('presetDropdown').contains(e.target)) {
                  closeDropdown();
                }
              });
              function onExposureToggle() {
                var checked = document.getElementById('autoExposureToggle').checked;
                document.getElementById('evSliderGroup').style.display = checked ? 'none' : 'block';
                document.getElementById('meteringGroup').style.display = checked ? 'block' : 'none';
                document.getElementById('exposureDesc').textContent = checked ? '自动检测并调整曝光' : '手动设置曝光补偿值';
              }
              function onEvChange() {
                var val = parseFloat(document.getElementById('evSlider').value);
                document.getElementById('evValue').textContent = (val > 0 ? '+' : '') + val.toFixed(1) + ' EV';
              }
              var selectedMetering = 'highlight-safe';
              function toggleMeteringDropdown() {
                var panel = document.getElementById('meteringPanel');
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
              function closeMeteringDropdown() {
                var panel = document.getElementById('meteringPanel');
                var btn = panel.previousElementSibling;
                panel.classList.remove('open');
                btn.classList.remove('open');
              }
              function selectMetering(opt) {
                selectedMetering = opt.getAttribute('data-value');
                document.getElementById('meteringLabel').textContent = opt.textContent;
                var allOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
                for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
                opt.classList.add('selected');
                closeMeteringDropdown();
              }
              document.addEventListener('click', function(e) {
                if (!document.getElementById('meteringDropdown').contains(e.target)) {
                  closeMeteringDropdown();
                }
              });
              var syncToAuto = false;
              function toggleSync() {
                syncToAuto = !syncToAuto;
                document.getElementById('syncToggle').className = 'toggle' + (syncToAuto ? ' on' : '');
              }
              function onConfirm() {
                var autoExp = document.getElementById('autoExposureToggle').checked;
                var ev = parseFloat(document.getElementById('evSlider').value);
                NativeBridge.onConfirm(selectedPreset, autoExp, selectedMetering, ev, syncToAuto);
              }
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
                fun onConfirm(lutId: String, useAutoExposure: Boolean, meteringMode: String, manualEv: Float, syncToAuto: Boolean) {
                    runOnUiThread {
                        dismissColorGradingWebView()
                        dispatchColorGrading(filePath, lutId, useAutoExposure, meteringMode, manualEv, syncToAuto)
                    }
                }
                @JavascriptInterface
                fun onCancel() {
                    runOnUiThread { dismissColorGradingWebView() }
                }
            }, "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        colorGradingWebView = webView
    }

    private fun dismissColorGradingWebView() {
        colorGradingWebView?.let {
            (it.parent as? FrameLayout)?.removeView(it)
            it.destroy()
        }
        colorGradingWebView = null
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    }

    private fun dispatchColorGrading(filePath: String, lutId: String, useAutoExposure: Boolean, meteringMode: String, manualEv: Float, syncToAuto: Boolean) {
        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available for color grading")
            return
        }

        val escapedFilePath = filePath.replace("\\", "\\\\").replace("'", "\\'")
        val escapedLutId = lutId.replace("\\", "\\\\").replace("'", "\\'")
        val escapedMeteringMode = meteringMode.replace("\\", "\\\\").replace("'", "\\'")
        val useAutoExpStr = useAutoExposure.toString()
        val manualEvStr = manualEv.toString()
        val syncToAutoStr = syncToAuto.toString()
        val js = """
            (function(){
                if(window.__tauriTriggerColorGrading){
                    window.__tauriTriggerColorGrading('$escapedFilePath','$escapedLutId','$useAutoExpStr','$escapedMeteringMode','$manualEvStr','$syncToAutoStr');
                    return 'ok';
                }
                return 'no_handler';
            })();
        """.trimIndent()

        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                if (result?.trim()?.removeSurrounding("\"") == "no_handler") {
                    runOnUiThread {
                        Log.w(TAG, "Color grading failed: frontend handler not available")
                    }
                }
            }
        }
    }

    private fun updateUI() {
        updateFilenameAndExif()
    }

    private fun updateFilenameAndExif() {
        currentDisplayName = null
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
                    currentDisplayName = displayName
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

                // Apply orientation from backend EXIF for RAW files where
                // Android's ExifInterface cannot read the orientation tag.
                applyOrientationFromExif(exif)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
            }
        }
    }

    /**
     * Apply EXIF orientation to the current SubsamplingScaleImageView.
     * This fixes RAW files (NEF, ARW, etc.) where Android's ExifInterface
     * fails to read the orientation, causing SubsamplingScaleImageView's
     * ORIENTATION_USE_EXIF to return no rotation.
     */
    private fun applyOrientationFromExif(exif: org.json.JSONObject) {
        val orientation = exif.optInt("orientation", 0)
        if (orientation <= 1) return // 0 = not present, 1 = normal (no rotation needed)

        // Map EXIF orientation (1-8) to clockwise rotation degrees
        val degrees = when (orientation) {
            3 -> 180
            6 -> 90
            8 -> 270
            else -> return // 2,4,5,7 involve flips — not supported by SubsamplingScaleImageView
        }

        // ViewPager2's first child is a RecyclerView; find the ViewHolder for current page
        val rv = viewPager.getChildAt(0) as? androidx.recyclerview.widget.RecyclerView ?: return
        val holder = rv.findViewHolderForAdapterPosition(currentIndex) as? ImageViewerAdapter.ViewHolder ?: return

        Log.d(TAG, "Applying backend orientation $orientation ($degrees°) to current image")
        holder.imageView.setOrientation(degrees)
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
                val jsonString = try {
                    val trimmed = result?.trim() ?: ""
                    if (trimmed.startsWith("\"")) {
                        org.json.JSONArray("[$trimmed]").getString(0)
                    } else {
                        trimmed
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to decode JSON from WebView: $result", e)
                    ""
                }
                val json = try { org.json.JSONObject(jsonString) } catch (e: Exception) {
                    Log.w(TAG, "Failed to parse prompt JSON: $jsonString", e)
                    null
                }
                val currentPrompt = json?.optString("prompt", "")?.replace("\\n", "\n") ?: ""
                val currentModel = json?.optString("model", "") ?: ""
                val autoEdit = json?.optBoolean("autoEdit", false) ?: false
                val hasApiKey = json?.optBoolean("hasApiKey", true) ?: true
                runOnUiThread { showPromptWebViewOverlay(filePath, currentPrompt, currentModel, autoEdit, hasApiKey, mainActivity) }
            }
        }
    }

    private fun showPromptWebViewOverlay(filePath: String, currentPrompt: String, currentModel: String, autoEditEnabled: Boolean, hasApiKey: Boolean, mainActivity: MainActivity) {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LOCKED
        val rootView = findViewById<FrameLayout>(android.R.id.content)

        // Dismiss any existing overlay
        dismissPromptWebView()

        val escapedPrompt = TextUtils.htmlEncode(currentPrompt)
            .replace("\n", "&#10;")

        // Determine which model option is selected — validate against whitelist
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

        val html = """
            <!DOCTYPE html>
            <html>
            <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,interactive-widget=resizes-content">
            <style>
              * { margin: 0; padding: 0; box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
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
              .content { padding: 16px; overflow: visible; }
              .field-group { margin-bottom: 12px; }
              .field-group:last-child { margin-bottom: 0; }
              .field-label { font-size: 14px; font-weight: 500; color: #374151; margin-bottom: 4px; }
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
              #apiKey {
                width: 100%; padding: 8px 40px 8px 12px; border: 1px solid #e5e7eb;
                border-radius: 8px; font-size: 14px; color: #374151;
                background: #fff; outline: none; font-family: inherit;
              }
              #apiKey:focus { border-color: transparent; box-shadow: 0 0 0 2px #3b82f6; }
              .eye-btn {
                position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
                background: none; border: none; cursor: pointer; padding: 4px; color: #9ca3af;
              }
              .eye-btn:hover { color: #4b5563; }
              .api-link {
                display: inline-flex; align-items: center; gap: 2px;
                font-size: 14px; color: #2563eb; text-decoration: none; margin-top: 4px;
              }
              .api-link:hover { color: #1d4ed8; }
              .api-link svg { flex-shrink: 0; }
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
                  $apiKeyHtml
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
              var realApiKey = '';
              var apiKeyVisible = false;
              function syncApiKeyDisplay() {
                var el = document.getElementById('apiKey');
                if (!el) return;
                el.value = apiKeyVisible ? realApiKey : '\u2022'.repeat(realApiKey.length);
              }
              function onConfirm() {
                var prompt = document.getElementById('prompt').value.trim();
                if (!prompt) return;
                var apiKeyOk = !document.getElementById('apiKey') || realApiKey.length > 0;
                if (!apiKeyOk) return;
                NativeBridge.onConfirm(prompt, selectedModel, saveAsAutoEdit, realApiKey);
              }
              function updateConfirmBtn() {
                var prompt = document.getElementById('prompt').value.trim();
                var apiKeyEl = document.getElementById('apiKey');
                var apiKeyOk = !apiKeyEl || realApiKey.length > 0;
                document.getElementById('confirmBtn').disabled = !(prompt && apiKeyOk);
              }
              function toggleApiKeyVisibility() {
                apiKeyVisible = !apiKeyVisible;
                syncApiKeyDisplay();
                var icon = document.getElementById('eyeIcon');
                if (apiKeyVisible) {
                  icon.innerHTML = '<path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/>';
                } else {
                  icon.innerHTML = '<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>';
                }
              }
              function onApiKeyInput(e) {
                var el = e.target;
                var raw = el.value;
                if (apiKeyVisible) {
                  realApiKey = raw;
                  updateConfirmBtn();
                  return;
                }
                var prevLen = realApiKey.length;
                var maskChar = '\u2022';
                var nonDots = raw.replace(/\u2022/g, '');
                var newValue;
                if (nonDots.length > 0) {
                  if (raw.length !== nonDots.length) {
                    var firstNew = raw.indexOf(nonDots[0]);
                    var dotsAfter = raw.length - firstNew - nonDots.length;
                    newValue = realApiKey.slice(0, firstNew) + nonDots + realApiKey.slice(prevLen - dotsAfter);
                  } else {
                    newValue = nonDots;
                  }
                } else if (raw.length < prevLen) {
                  var cursor = el.selectionStart != null ? el.selectionStart : raw.length;
                  var deleted = prevLen - raw.length;
                  newValue = realApiKey.slice(0, cursor) + realApiKey.slice(cursor + deleted);
                } else {
                  return;
                }
                realApiKey = newValue;
                updateConfirmBtn();
                var newDisplay = maskChar.repeat(newValue.length);
                var newCursor = el.selectionStart;
                el.value = newDisplay;
                if (newCursor != null) el.setSelectionRange(newCursor, newCursor);
              }
              document.getElementById('prompt').addEventListener('input', updateConfirmBtn);
              var apiKeyInput = document.getElementById('apiKey');
              if (apiKeyInput) apiKeyInput.addEventListener('input', onApiKeyInput);
              updateConfirmBtn();
              (document.getElementById('apiKey') || document.getElementById('prompt')).focus();
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
                fun onConfirm(prompt: String, model: String, saveAsAutoEdit: Boolean, apiKey: String) {
                    runOnUiThread {
                        dismissPromptWebView()
                        dispatchAiEdit(filePath, prompt, model, saveAsAutoEdit, apiKey, mainActivity)
                    }
                }
                @JavascriptInterface
                fun onCancel() {
                    runOnUiThread { dismissPromptWebView() }
                }
                @JavascriptInterface
                fun openLink(url: String) {
                    runOnUiThread {
                        try {
                            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                        } catch (e: Exception) {
                            Log.w(TAG, "Failed to open external link: $url", e)
                        }
                    }
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
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    }

    private fun dispatchAiEdit(filePath: String, prompt: String, model: String, saveAsAutoEdit: Boolean, apiKey: String, mainActivity: MainActivity) {
        isAiEditing = true

        // Use JSONArray for safe JSON encoding — avoids JS string injection vulnerabilities
        val args = JSONArray().put(filePath).put(prompt).put(model).put(saveAsAutoEdit).put(apiKey)
        val js = """
            (function() {
                var args = $args;
                if (window.__tauriTriggerAiEditWithPrompt) {
                    window.__tauriTriggerAiEditWithPrompt(args[0], args[1], args[2], args[3], args[4]);
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
        return resolveUriToFilePath(this, uriString)
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
    fun onAiEditComplete(success: Boolean, message: String?, cancelled: Boolean) {
        runOnUiThread {
            isAiEditing = false
            isColorGrading = false
            stopHighlightSweepAnimation()
            if (isFinishing || isDestroyed) return@runOnUiThread

            // On cancel, silently hide the progress bar without showing success/failure
            if (cancelled) {
                resetProgressAppearance()
                aiEditProgressContainer.visibility = View.GONE
                return@runOnUiThread
            }

            val containerColor: Int
            val strokeColor: Int
            val fillColors: IntArray?
            val fillColor: Int?
            val edgeColors: IntArray?
            val edgeColor: Int?

            if (success) {
                containerColor = 0xBF1A3C1A.toInt()
                strokeColor = 0x3322C55E.toInt()
                fillColors = intArrayOf(0x0022C55E.toInt(), 0x2622C55E.toInt(), 0x2622C55E.toInt(), 0x0022C55E.toInt())
                fillColor = null
                edgeColors = intArrayOf(0x004ADE80.toInt(), 0x994ADE80.toInt(), 0x994ADE80.toInt(), 0x004ADE80.toInt())
                edgeColor = null
            } else {
                containerColor = 0xB35C1A1A.toInt()
                strokeColor = 0x33EF4444.toInt()
                fillColors = null
                fillColor = 0x33EF4444.toInt()
                edgeColors = null
                edgeColor = 0x99F87171.toInt()
            }

            val dp = resources.displayMetrics.density

            aiEditProgressContainer.background = android.graphics.drawable.GradientDrawable().apply {
                shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                setColor(containerColor)
                cornerRadius = 12f * dp
                setStroke(dp.toInt(), strokeColor)
            }

            aiEditProgressFill.layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            if (fillColors != null) {
                aiEditProgressFill.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT, fillColors
                )
            } else {
                aiEditProgressFill.setBackgroundColor(fillColor!!)
            }
            aiEditProgressFill.visibility = View.VISIBLE
            aiEditProgressHighlight.visibility = View.GONE

            aiEditProgressEdge.layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                2,
                Gravity.BOTTOM
            )
            if (edgeColors != null) {
                aiEditProgressEdge.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT, edgeColors
                )
            } else {
                aiEditProgressEdge.setBackgroundColor(edgeColor!!)
            }

            aiEditStatusText.text = "修图完成"
            aiEditStatusText.setTextColor(0xFFFFFFFF.toInt())
            aiEditProgressText.text = message ?: ""
            aiEditFailureText.visibility = View.GONE
            aiEditCancelBtn.visibility = View.VISIBLE
            aiEditCancelBtn.text = "✕"
            aiEditCancelBtn.setOnClickListener {
                resetProgressAppearance()
                aiEditProgressContainer.visibility = View.GONE
            }
        }
    }

    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread

            isAiEditing = true
            isColorGrading = false

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

    fun updateColorGradingProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread

            isColorGrading = true
            isAiEditing = false
            resetProgressAppearance()
            aiEditProgressContainer.visibility = View.VISIBLE
            aiEditStatusText.visibility = View.VISIBLE
            aiEditStatusText.text = "调色中..."
            aiEditCancelBtn.visibility = View.VISIBLE
            aiEditCancelBtn.text = "取消"
            aiEditCancelBtn.setOnClickListener {
                val mainActivity = MainActivity.instance
                mainActivity?.runOnUiThread {
                    mainActivity.getWebView()?.evaluateJavascript(
                        "(function(){try{window.__tauriCancelColorGrading?.()}catch(e){}})();", null
                    )
                }
            }

            aiEditProgressContainer.post { updateProgressBarPosition() }

            val percent = if (total > 0) (current * 100) / total else 0

            val containerWidth = aiEditProgressContainer.width
            if (containerWidth > 0) {
                applyProgressLayout(containerWidth, percent)
            } else {
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

    fun onColorGradingComplete(success: Boolean, message: String?, cancelled: Boolean) {
        runOnUiThread {
            isColorGrading = false
            stopHighlightSweepAnimation()
            if (isFinishing || isDestroyed) return@runOnUiThread

            if (cancelled) {
                resetProgressAppearance()
                aiEditProgressContainer.visibility = View.GONE
                return@runOnUiThread
            }

            val containerColor: Int
            val strokeColor: Int
            val fillColors: IntArray?
            val fillColor: Int?
            val edgeColors: IntArray?
            val edgeColor: Int?

            if (success) {
                containerColor = 0xBF1A3C1A.toInt()
                strokeColor = 0x3322C55E.toInt()
                fillColors = intArrayOf(0x0022C55E.toInt(), 0x2622C55E.toInt(), 0x2622C55E.toInt(), 0x0022C55E.toInt())
                fillColor = null
                edgeColors = intArrayOf(0x004ADE80.toInt(), 0x994ADE80.toInt(), 0x994ADE80.toInt(), 0x004ADE80.toInt())
                edgeColor = null
            } else {
                containerColor = 0xB35C1A1A.toInt()
                strokeColor = 0x33EF4444.toInt()
                fillColors = null
                fillColor = 0x33EF4444.toInt()
                edgeColors = null
                edgeColor = 0x99F87171.toInt()
            }

            val dp = resources.displayMetrics.density

            aiEditProgressContainer.background = android.graphics.drawable.GradientDrawable().apply {
                shape = android.graphics.drawable.GradientDrawable.RECTANGLE
                setColor(containerColor)
                cornerRadius = 12f * dp
                setStroke(dp.toInt(), strokeColor)
            }

            aiEditProgressFill.layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
            if (fillColors != null) {
                aiEditProgressFill.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT, fillColors
                )
            } else {
                aiEditProgressFill.setBackgroundColor(fillColor!!)
            }
            aiEditProgressFill.visibility = View.VISIBLE
            aiEditProgressHighlight.visibility = View.GONE

            aiEditProgressEdge.layoutParams = FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                2,
                Gravity.BOTTOM
            )
            if (edgeColors != null) {
                aiEditProgressEdge.background = android.graphics.drawable.GradientDrawable(
                    android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT, edgeColors
                )
            } else {
                aiEditProgressEdge.setBackgroundColor(edgeColor!!)
            }

            aiEditStatusText.text = "调色完成"
            aiEditStatusText.setTextColor(0xFFFFFFFF.toInt())
            aiEditProgressText.text = message ?: ""
            aiEditFailureText.visibility = View.GONE
            aiEditCancelBtn.visibility = View.VISIBLE
            aiEditCancelBtn.text = "✕"
            aiEditCancelBtn.setOnClickListener {
                resetProgressAppearance()
                aiEditProgressContainer.visibility = View.GONE
            }
        }
    }

    private fun syncColorGradingProgressFromWebView() {
        val progress = com.gjk.cameraftpcompanion.bridges.ImageViewerBridge.lastColorGradingProgress
        val grading = com.gjk.cameraftpcompanion.bridges.ImageViewerBridge.isColorGrading
        if (grading && progress != null) {
            isColorGrading = true
            updateColorGradingProgress(progress.current, progress.total, progress.failedCount)
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

        dismissPromptWebView()
        dismissColorGradingWebView()
        setContentView(R.layout.activity_image_viewer)

        bindViews()

        setupViewPager()
        setupButtons()
        updateUI()
        syncAiEditProgressFromWebView()
        syncColorGradingProgressFromWebView()
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
        syncColorGradingProgressFromWebView()
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
        dismissColorGradingWebView()
        if (instance == this) {
            isViewerVisible = false
            instance = null
        }
        super.onDestroy()
    }
}
