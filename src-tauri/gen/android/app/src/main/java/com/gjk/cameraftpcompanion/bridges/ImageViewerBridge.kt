/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.util.Log
import com.gjk.cameraftpcompanion.ImageViewerActivity
import com.gjk.cameraftpcompanion.MainActivity
import org.json.JSONArray

class ImageViewerBridge(activity: android.app.Activity) : BaseJsBridge(activity) {

    companion object {
        private const val TAG = "ImageViewerBridge"

        data class AiEditProgressState(
            val current: Int,
            val total: Int,
            val failedCount: Int,
        )

        data class ColorGradingProgressState(
            val current: Int,
            val total: Int,
            val failedCount: Int,
        )

        @Volatile
        var lastProgress: AiEditProgressState? = null
            private set

        @Volatile
        var isAiEditing: Boolean = false
            private set

        @Volatile
        var lastColorGradingProgress: ColorGradingProgressState? = null
            private set

        @Volatile
        var isColorGrading: Boolean = false
            private set

        @Volatile
        var isAiEditDone: Boolean = false
            private set

        @Volatile
        var isColorGradingDone: Boolean = false
            private set

        fun clearProgress() {
            lastProgress = null
            isAiEditing = false
            isAiEditDone = false
        }

        fun clearColorGradingProgress() {
            lastColorGradingProgress = null
            isColorGrading = false
            isColorGradingDone = false
        }
    }

    @android.webkit.JavascriptInterface
    fun isAppVisible(): Boolean {
        return MainActivity.isAppVisible
    }

    @android.webkit.JavascriptInterface
    fun openOrNavigateTo(uri: String, allUrisJson: String): Boolean {
        Log.d(TAG, "openOrNavigateTo: uri=$uri")
        return try {
            val allUris = JSONArray(allUrisJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }
            val navigationTarget = ImageViewerActivity.buildNavigationTarget(allUris, uri)
            if (navigationTarget == null) {
                Log.e(TAG, "openViewer: target URI not found in list")
                return false
            }
            ImageViewerActivity.navigateOrStart(
                activity,
                navigationTarget.uris,
                navigationTarget.targetIndex,
            )
            true
        } catch (e: Exception) {
            Log.e(TAG, "openOrNavigateTo error", e)
            false
        }
    }

    /**
     * Callback from JS when EXIF data is fetched via Tauri IPC
     */
    @android.webkit.JavascriptInterface
    fun onExifResult(exifJson: String?) {
        ImageViewerActivity.instance?.onExifResult(exifJson)
    }

    /**
     * Resolve a URI to a file system path.
     * Handles file://, content:// (via MediaStore), and fallback.
     */
    @android.webkit.JavascriptInterface
    fun resolveFilePath(uri: String): String? {
        return ImageViewerActivity.resolveUriToFilePath(activity, uri)
    }

    /**
     * Called from JS when an AI edit triggered from native completes.
     */
    @android.webkit.JavascriptInterface
    fun onAiEditComplete(success: Boolean, message: String?, cancelled: Boolean) {
        isAiEditing = false
        if (cancelled) {
            clearProgress()
        }
        isAiEditDone = !cancelled && success
        val viewer = ImageViewerActivity.instance ?: return
        viewer.onAiEditComplete(success, message, cancelled)
    }

    @android.webkit.JavascriptInterface
    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        isAiEditing = true
        lastProgress = AiEditProgressState(current, total, failedCount)
        val viewer = ImageViewerActivity.instance ?: return
        viewer.updateAiEditProgress(current, total, failedCount)
    }

    /**
     * Triggers a MediaStore scan for a newly created file so it appears in the system gallery.
     */
    @android.webkit.JavascriptInterface
    fun scanNewFile(filePath: String?) {
        if (filePath == null) return
        val viewer = ImageViewerActivity.instance
        val context = (viewer ?: activity) as? android.content.Context ?: return
        android.media.MediaScannerConnection.scanFile(context, arrayOf(filePath), null, null)
    }

    @android.webkit.JavascriptInterface
    fun updateColorGradingProgress(current: Int, total: Int, failedCount: Int) {
        isColorGrading = true
        lastColorGradingProgress = ColorGradingProgressState(current, total, failedCount)
        val viewer = ImageViewerActivity.instance ?: return
        viewer.updateColorGradingProgress(current, total, failedCount)
    }

    @android.webkit.JavascriptInterface
    fun onColorGradingComplete(success: Boolean, message: String?, cancelled: Boolean) {
        isColorGrading = false
        if (cancelled) {
            clearColorGradingProgress()
        }
        isColorGradingDone = !cancelled && success
        val viewer = ImageViewerActivity.instance ?: return
        viewer.onColorGradingComplete(success, message, cancelled)
    }

    @android.webkit.JavascriptInterface
    fun dismissAllTaskProgress() {
        clearProgress()
        clearColorGradingProgress()
        ImageViewerActivity.instance?.dismissAllTaskProgress()
    }

    /**
     * Request EXIF data for multiple image positions.
     * Called from JS to prefetch EXIF for offscreen pages.
     */
    @android.webkit.JavascriptInterface
    fun requestExifForPositions(requestJson: String?) {
        if (requestJson == null) return
        try {
            val items = org.json.JSONArray(requestJson)
            val jsItems = mutableListOf<String>()
            for (i in 0 until items.length()) {
                jsItems.add(items.getJSONObject(i).toString())
            }
            val viewer = ImageViewerActivity.instance ?: return
            viewer.requestExifPrefetch(jsItems)
        } catch (e: Exception) {
            Log.e(TAG, "requestExifForPositions error", e)
        }
    }

    /**
     * Callback from JS with EXIF data for a specific adapter position.
     */
    @android.webkit.JavascriptInterface
    fun onExifResultForPosition(position: Int, exifJson: String?) {
        ImageViewerActivity.instance?.onExifResultForPosition(position, exifJson)
    }
}
