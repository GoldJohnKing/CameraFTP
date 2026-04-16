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
    private lateinit var aiEditProgressFill: View
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
        set(value) {
            field = value
            runOnUiThread {
                if (!isFinishing && !isDestroyed) {
                    btnAiEdit.isEnabled = !value
                    btnAiEdit.alpha = if (value) 0.5f else 1f
                }
            }
        }
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
    }

    private fun setupButtons() {
        btnAiEdit.setOnClickListener {
            if (!isAiEditing && uris.isNotEmpty() && currentIndex in uris.indices) {
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

        // Fetch current prompt from WebView config, then show WebView overlay dialog
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{return window.__tauriGetAiEditPrompt?.()??''}catch(e){return ''}})();"
            ) { result ->
                val currentPrompt = result?.trim()?.removeSurrounding("\"")?.replace("\\n", "\n") ?: ""
                runOnUiThread { showPromptWebViewOverlay(filePath, currentPrompt, mainActivity) }
            }
        }
    }

    private fun showPromptWebViewOverlay(filePath: String, currentPrompt: String, mainActivity: MainActivity) {
        val rootView = findViewById<FrameLayout>(android.R.id.content)

        // Dismiss any existing overlay
        dismissPromptWebView()

        val escapedPrompt = currentPrompt
            .replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace("\"", "&quot;").replace("'", "&#39;")
            .replace("\n", "&#10;")

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
              textarea {
                width: 100%; padding: 8px 12px; border: 1px solid #e5e7eb;
                border-radius: 8px; font-size: 14px; color: #374151;
                background: #fff; resize: none; outline: none;
                font-family: inherit; line-height: 1.5;
              }
              textarea:focus { border-color: transparent; box-shadow: 0 0 0 2px #3b82f6; }
              .hint { font-size: 12px; color: #9ca3af; margin-top: 8px; }
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
              .actions { display: flex; gap: 8px; }
              .btn {
                padding: 8px 16px; border-radius: 8px; font-size: 14px;
                font-weight: 500; border: none; cursor: pointer;
              }
              .btn-cancel { background: #f3f4f6; color: #374151; }
              .btn-cancel:hover { background: #e5e7eb; }
              .btn-confirm { background: #2563eb; color: #fff; }
              .btn-confirm:hover { background: #1d4ed8; }
            </style>
            </head>
            <body>
            <div class="overlay" onclick="if(event.target===this)NativeBridge.onCancel()">
              <div class="card">
                <div class="header">
                  <div class="title-group">
                    <div class="title">AI修图提示词</div>
                    <div class="subtitle">编辑提示词后确认触发修图</div>
                  </div>
                  <button class="close-btn" onclick="NativeBridge.onCancel()">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </button>
                </div>
                <div class="content">
                  <textarea id="prompt" rows="4" placeholder="例如：提升画质，使照片更清晰">${escapedPrompt}</textarea>
                  <div class="hint">留空使用默认提示词</div>
                </div>
                <div class="footer">
                  <div class="save-toggle" onclick="toggleSave()">
                    <div class="toggle on" id="saveToggle"></div>
                    <span>保存提示词</span>
                  </div>
                  <div class="actions">
                    <button class="btn btn-cancel" onclick="NativeBridge.onCancel()">取消</button>
                    <button class="btn btn-confirm" onclick="onConfirm()">确认修图</button>
                  </div>
                </div>
              </div>
            </div>
            <script>
              var savePrompt = true;
              function toggleSave() {
                savePrompt = !savePrompt;
                document.getElementById('saveToggle').className = 'toggle' + (savePrompt ? ' on' : '');
              }
              function onConfirm() {
                var prompt = document.getElementById('prompt').value.trim();
                NativeBridge.onConfirm(prompt, savePrompt);
              }
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
                fun onConfirm(prompt: String, shouldSave: Boolean) {
                    runOnUiThread {
                        dismissPromptWebView()
                        dispatchAiEdit(filePath, prompt, shouldSave, mainActivity)
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

    private fun dispatchAiEdit(filePath: String, prompt: String, shouldSave: Boolean, mainActivity: MainActivity) {
        isAiEditing = true

        val escapedPath = filePath.replace("\\", "\\\\").replace("'", "\\'")
        val escapedPrompt = prompt.replace("\\", "\\\\").replace("'", "\\'")
            .replace("\n", "\\n").replace("\r", "\\r")

        val js = """
            (function() {
                if (window.__tauriTriggerAiEditWithPrompt) {
                    window.__tauriTriggerAiEditWithPrompt('$escapedPath', '$escapedPrompt', $shouldSave);
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

    /**
     * Called from JS bridge when AI edit completes (success or failure)
     */
    fun onAiEditComplete(success: Boolean, message: String?) {
        runOnUiThread {
            isAiEditing = false
            stopHighlightSweepAnimation()
            if (isFinishing || isDestroyed) return@runOnUiThread
            if (success) {
                aiEditProgressContainer.visibility = View.GONE
            } else {
                aiEditStatusText.text = "修图完成"
                aiEditStatusText.setTextColor(0xFFFFFFFF.toInt())
                aiEditProgressText.text = message ?: "修图失败"
                aiEditFailureText.visibility = View.GONE
                aiEditCancelBtn.text = "✕"
                aiEditCancelBtn.setOnClickListener {
                    aiEditProgressContainer.visibility = View.GONE
                }
                aiEditProgressContainer.postDelayed({
                    aiEditProgressContainer.visibility = View.GONE
                }, 3000)
            }
        }
    }

    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread

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
        val wasAiEditing = isAiEditing

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
        if (wasAiEditing) {
            btnAiEdit.isEnabled = false
            btnAiEdit.alpha = 0.5f
        }

        setupViewPager()
        setupButtons()
        updateUI()
        syncAiEditProgressFromWebView()
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
