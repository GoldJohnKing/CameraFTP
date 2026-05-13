/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.viewpager2.widget.ViewPager2
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import com.gjk.cameraftpcompanion.controllers.DeleteController
import com.gjk.cameraftpcompanion.controllers.ExifController
import com.gjk.cameraftpcompanion.controllers.TaskProgressController
import com.gjk.cameraftpcompanion.controllers.WebViewOverlayController
import org.json.JSONArray
import java.lang.ref.WeakReference

class ImageViewerActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ImageViewerActivity"
        private val RAW_EXTENSIONS = setOf(
            "nef", "nrw", "cr2", "cr3", "arw", "sr2",
            "raf", "orf", "rw2", "pef", "dng", "x3f", "raw", "srw"
        )
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"
        private var _instance: WeakReference<ImageViewerActivity>? = null
        val instance: ImageViewerActivity?
            get() = _instance?.get()

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
            if (targetUris.isEmpty()) return null
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
            } else {
                start(context, plan.uris, plan.safeTargetIndex)
            }
        }

        @JvmStatic
        fun exifOrientationToDegrees(orientation: Int): Int {
            return when (orientation) {
                3 -> 180; 6 -> 90; 8 -> 270; else -> 0
            }
        }

        @JvmStatic
        fun resolveUriToFilePath(context: android.content.Context, uriString: String): String? {
            return try {
                val uri = Uri.parse(uriString)
                when (uri.scheme) {
                    "file" -> uri.path
                    "content" -> context.contentResolver.query(
                        uri, arrayOf(MediaStore.Images.Media.DATA), null, null, null
                    )?.use { cursor ->
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

    // Stable views (survive config changes)
    internal lateinit var viewPager: ViewPager2
    private lateinit var bottomBar: LinearLayout
    private lateinit var btnMenu: ImageButton
    private lateinit var btnRotate: ImageButton
    private lateinit var btnDelete: ImageButton

    // Dynamic views (recreated on config change via bindBottomBarInfo)
    internal lateinit var filenameView: TextView
    internal lateinit var exifParams: TextView
    internal lateinit var exifDatetime: TextView

    // State
    internal var uris: MutableList<String> = mutableListOf()
    internal var currentIndex: Int = 0
    internal var currentDisplayName: String? = null
    private var isLandscape = false
    private var isBottomBarVisible = true
    private var menuPopupWindow: android.widget.PopupWindow? = null

    // Controllers
    internal lateinit var overlayController: WebViewOverlayController
    internal lateinit var exifController: ExifController
    internal lateinit var taskController: TaskProgressController
    private lateinit var deleteController: DeleteController

    private val deleteRequestLauncher = registerForActivityResult(
        ActivityResultContracts.StartIntentSenderForResult(),
    ) { result ->
        val uriString = deleteController.getPendingDeleteUri() ?: return@registerForActivityResult
        deleteController.clearPendingDeleteUri()
        if (result.resultCode == Activity.RESULT_OK) {
            deleteController.finalizeDeleteAfterConfirmation(uriString, uris, currentIndex)
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

        // Initialize controllers
        exifController = ExifController(this)
        overlayController = WebViewOverlayController(this)
        taskController = TaskProgressController(this)
        deleteController = DeleteController(this, deleteRequestLauncher)

        bindViews()
        setupViewPager()
        setupButtons()
        updateUI()
    }

    private fun bindViews() {
        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        btnMenu = findViewById(R.id.btn_menu)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)
        taskController.bindViews(findViewById(android.R.id.content))
        bindBottomBarInfo()
    }

    private fun bindBottomBarInfo() {
        val infoContainer = findViewById<FrameLayout>(R.id.bottom_bar_info)
        infoContainer.removeAllViews()
        val infoLayout = if (resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE) {
            R.layout.bottom_bar_info_landscape
        } else {
            R.layout.bottom_bar_info_portrait
        }
        layoutInflater.inflate(infoLayout, infoContainer, true)
        filenameView = infoContainer.findViewById(R.id.filename)
        exifParams = infoContainer.findViewById(R.id.exif_params)
        exifDatetime = infoContainer.findViewById(R.id.exif_datetime)
    }

    private fun setupViewPager() {
        val adapter = ImageViewerAdapter(
            uris,
            onTap = { toggleBottomBar() },
            onExifNeeded = { position, uri -> exifController.requestSingleExif(position, uri) },
        )
        adapter.immediateLoadPosition = currentIndex
        exifController.attachAdapter(adapter)
        viewPager.adapter = adapter
        viewPager.setCurrentItem(currentIndex, false)
        viewPager.offscreenPageLimit = 1
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                val adapter = viewPager.adapter as? ImageViewerAdapter
                adapter?.currentPosition = position
                if (adapter?.immediateLoadPosition == position) {
                    adapter.immediateLoadPosition = -1
                }
                updateUI()
                exifController.prefetchOrientations(around = position)
            }
        })
    }

    fun navigateTo(newUris: List<String>, targetIndex: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread

            exifController.orientationCache.clear()
            uris.clear()
            uris.addAll(newUris)

            if (uris.isEmpty()) { finish(); return@runOnUiThread }

            val safeTargetIndex = targetIndex.coerceIn(0, uris.lastIndex)
            currentIndex = safeTargetIndex

            val existingAdapter = viewPager.adapter as? ImageViewerAdapter
            if (existingAdapter != null) {
                existingAdapter.replaceUris(uris)
                existingAdapter.immediateLoadPosition = safeTargetIndex
                exifController.attachAdapter(existingAdapter)
            } else {
                setupViewPager()
            }

            viewPager.setCurrentItem(safeTargetIndex, false)
            updateUI()
            exifController.prefetchOrientations(around = safeTargetIndex)
        }
    }

    fun insertImage(uri: String, insertIndex: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            if (uris.contains(uri)) return@runOnUiThread

            val adapter = viewPager.adapter as? ImageViewerAdapter ?: return@runOnUiThread
            val clampedIndex = insertIndex.coerceIn(0, uris.size)

            uris.add(clampedIndex, uri)

            // Shift orientation cache entries for positions >= clampedIndex
            val shiftedCache = java.util.concurrent.ConcurrentHashMap<Int, Int>()
            for ((pos, degrees) in exifController.orientationCache) {
                shiftedCache[if (pos >= clampedIndex) pos + 1 else pos] = degrees
            }
            exifController.orientationCache.clear()
            exifController.orientationCache.putAll(shiftedCache)

            if (!adapter.insertUri(clampedIndex, uri)) {
                // Adapter rejected (duplicate or other issue) — revert
                uris.removeAt(clampedIndex)
                exifController.orientationCache.clear()
                shiftedCache.let { old ->
                    for ((pos, degrees) in old) {
                        exifController.orientationCache[if (pos > clampedIndex) pos - 1 else pos] = degrees
                    }
                }
                return@runOnUiThread
            }

            // Adjust currentIndex if insert is before or at current position
            if (clampedIndex <= currentIndex) {
                currentIndex += 1
            }

            // ViewPager2 stays at current position; update bottom bar info
            viewPager.setCurrentItem(currentIndex, false)
            updateUI()
            exifController.prefetchOrientations(around = currentIndex)
        }
    }

    fun navigateToExistingUri(uri: String) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            val targetIndex = uris.indexOf(uri)
            if (targetIndex < 0) return@runOnUiThread
            if (targetIndex == currentIndex) return@runOnUiThread

            val adapter = viewPager.adapter as? ImageViewerAdapter ?: return@runOnUiThread
            currentIndex = targetIndex
            adapter.immediateLoadPosition = targetIndex
            viewPager.setCurrentItem(targetIndex, false)
            updateUI()
            exifController.prefetchOrientations(around = targetIndex)
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
        taskController.updatePosition(isBottomBarVisible, bottomBar)
    }

    private fun setupButtons() {
        btnMenu.setOnClickListener { showImageMenu() }
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
                val uriString = uris.getOrNull(currentIndex) ?: return@setOnClickListener
                deleteController.deleteCurrentImage(uriString, uris, currentIndex)
            }
        }
    }

    internal fun isRawFileByExtension(displayName: String?): Boolean {
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
            if (uris.isNotEmpty() && currentIndex in uris.indices) triggerAiEditForCurrentImage()
        }
        menuItemColorGrading.setOnClickListener {
            menuPopupWindow?.dismiss()
            if (uris.isNotEmpty() && currentIndex in uris.indices) triggerColorGradingForCurrentImage()
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
        popup.setOnDismissListener { menuPopupWindow = null }
        menuPopupWindow = popup
        popupView.measure(
            android.view.View.MeasureSpec.UNSPECIFIED,
            android.view.View.MeasureSpec.UNSPECIFIED
        )
        val yOffset = -(btnMenu.height + popupView.measuredHeight + 8.dpToPx())
        popup.showAsDropDown(btnMenu, 0, yOffset)
    }

    // --- Color grading trigger ---

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
            overlayController.showColorGrading(filePath, false)
            return
        }
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{return window.__tauriGetAutoColorGradingEnabled?.()??'false'}catch(e){return 'false'}})();"
            ) { result ->
                val enabled = result?.trim()?.removeSurrounding("\"")?.toBoolean() ?: false
                overlayController.showColorGrading(filePath, enabled)
            }
        }
    }

    internal fun dispatchColorGrading(
        filePath: String, lutId: String, useAutoExposure: Boolean,
        meteringMode: String, manualEv: Float, syncToAuto: Boolean,
    ) {
        val mainActivity = MainActivity.instance ?: run {
            Log.w(TAG, "MainActivity not available for color grading"); return
        }
        val args = JSONArray().apply {
            put(filePath); put(lutId); put(useAutoExposure)
            put(meteringMode); put(manualEv); put(syncToAuto)
        }
        val js = """
            (function(){
                if(window.__tauriTriggerColorGrading){
                    window.__tauriTriggerColorGrading(...${args.toString()});
                    return 'ok';
                }
                return 'no_handler';
            })();
        """.trimIndent()
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                if (result?.trim()?.removeSurrounding("\"") == "no_handler") {
                    runOnUiThread { Log.w(TAG, "Color grading failed: frontend handler not available") }
                }
            }
        }
    }

    // --- AI edit trigger ---

    private fun triggerAiEditForCurrentImage() {
        val uriString = uris.getOrNull(currentIndex) ?: return
        val filePath = resolveUriToFilePath(uriString)
        if (filePath == null) {
            Log.w(TAG, "Cannot resolve file path for URI: $uriString"); return
        }
        val mainActivity = MainActivity.instance ?: run {
            Log.w(TAG, "MainActivity not available for AI edit"); return
        }
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{return window.__tauriGetAiEditPrompt?.()??''}catch(e){return ''}})();"
            ) { result ->
                val jsonString = try {
                    val trimmed = result?.trim() ?: ""
                    if (trimmed.startsWith("\"")) JSONArray("[$trimmed]").getString(0) else trimmed
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to decode JSON from WebView: $result", e); ""
                }
                val json = try { org.json.JSONObject(jsonString) } catch (e: Exception) {
                    Log.w(TAG, "Failed to parse prompt JSON: $jsonString", e); null
                }
                val currentPrompt = json?.optString("prompt", "")?.replace("\\n", "\n") ?: ""
                val currentModel = json?.optString("model", "") ?: ""
                val autoEdit = json?.optBoolean("autoEdit", false) ?: false
                val hasApiKey = json?.optBoolean("hasApiKey", true) ?: true
                runOnUiThread {
                    overlayController.showAiEditPrompt(filePath, currentPrompt, currentModel, autoEdit, hasApiKey, mainActivity)
                }
            }
        }
    }

    internal fun dispatchAiEdit(
        filePath: String, prompt: String, model: String,
        saveAsAutoEdit: Boolean, apiKey: String, mainActivity: MainActivity,
    ) {
        taskController.isAiEditing = true
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
                        taskController.isAiEditing = false
                    }
                }
            }
        }
    }

    // --- EXIF prefetch (called by ExifController) ---

    internal fun requestExifPrefetch(jsonString: String) {
        val mainActivity = MainActivity.instance ?: return
        val webView = mainActivity.getWebView() ?: return
        val escaped = jsonString
            .replace("\\", "\\\\")
            .replace("'", "\\'")
            .replace("\n", "\\n")
        val js = "if(window.__requestExifForPositions)window.__requestExifForPositions('$escaped')"
        webView.post { webView.evaluateJavascript(js, null) }
    }

    // --- Bridge callbacks (called from ImageViewerBridge via static instance) ---

    fun onExifResult(exifJson: String?) {
        exifController.onExifResult(exifJson)
    }

    fun onExifResultForPosition(position: Int, exifJson: String?) {
        exifController.onExifResultForPosition(position, exifJson)
    }

    fun onAiEditComplete(success: Boolean, message: String?, cancelled: Boolean) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            taskController.onAiEditComplete(cancelled)
        }
    }

    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            taskController.updateAiEditProgress(current, total, failedCount)
        }
    }

    fun updateColorGradingProgress(current: Int, total: Int, failedCount: Int) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            taskController.updateColorGradingProgress(current, total, failedCount)
        }
    }

    fun onColorGradingComplete(success: Boolean, message: String?, cancelled: Boolean) {
        runOnUiThread {
            if (isFinishing || isDestroyed) return@runOnUiThread
            taskController.onColorGradingComplete(cancelled)
        }
    }

    fun dismissAllTaskProgress() {
        runOnUiThread { taskController.dismissAll() }
    }

    // --- Delete callback (called by DeleteController) ---

    internal fun onDeleteSuccess(updatedUris: MutableList<String>, newIndex: Int) {
        currentIndex = newIndex
        exifController.orientationCache.clear()
        (viewPager.adapter as? ImageViewerAdapter)?.replaceUris(uris)
        viewPager.setCurrentItem(currentIndex, false)
        updateUI()
        exifController.prefetchOrientations(around = currentIndex)
    }

    // --- UI update ---

    private fun updateUI() {
        exifController.updateFilenameAndExif()
    }

    private fun resolveUriToFilePath(uriString: String): String? {
        return resolveUriToFilePath(this, uriString)
    }

    // --- Lifecycle ---

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        isLandscape = newConfig.orientation == Configuration.ORIENTATION_LANDSCAPE
        taskController.cancelAutoDismiss()
        overlayController.dismissAll()

        // Only swap the bottom bar info section — ViewPager, adapter, task panel survive
        bindBottomBarInfo()

        // Re-fit the visible image to the new screen dimensions
        resetCurrentImageScale()

        updateUI()
        taskController.syncAiEditProgress()
        taskController.syncColorGradingProgress()
        if (taskController.isVisible) {
            taskController.updatePosition(isBottomBarVisible, bottomBar)
        }
    }

    private fun resetCurrentImageScale() {
        val rv = viewPager.getChildAt(0) as? androidx.recyclerview.widget.RecyclerView ?: return
        // Defer until after the layout pass so SubsamplingScaleImageView uses new dimensions
        rv.post {
            for (i in 0 until rv.childCount) {
                val holder = rv.getChildViewHolder(rv.getChildAt(i)) as? ImageViewerAdapter.ViewHolder ?: continue
                holder.imageView.resetScaleAndCenter()
            }
        }
    }

    override fun onResume() {
        super.onResume()
        _instance = WeakReference(this)
        isViewerVisible = true
        taskController.syncAiEditProgress()
        taskController.syncColorGradingProgress()
        if (taskController.isVisible) {
            taskController.updatePosition(isBottomBarVisible, bottomBar)
        }
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

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        finish()
    }

    override fun onDestroy() {
        exifController.destroy()
        taskController.destroy()
        overlayController.dismissAll()
        if (instance == this) {
            isViewerVisible = false
            _instance = null
        }
        super.onDestroy()
    }

    // --- Utilities ---

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

    private fun Int.dpToPx(): Int {
        return dpToPx(this@ImageViewerActivity)
    }
}
