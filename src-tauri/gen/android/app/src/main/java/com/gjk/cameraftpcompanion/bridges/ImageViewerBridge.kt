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

sealed class TaskProgressState {
    data object Idle : TaskProgressState()
    data class InProgress(val current: Int, val total: Int, val failedCount: Int) : TaskProgressState()
    data class Done(val success: Boolean, val total: Int, val failedCount: Int) : TaskProgressState()
}

class ImageViewerBridge(activity: android.app.Activity) : BaseJsBridge(activity) {

    companion object {
        private const val TAG = "ImageViewerBridge"

        @Volatile
        var aiEditState: TaskProgressState = TaskProgressState.Idle
            private set

        @Volatile
        var colorGradingState: TaskProgressState = TaskProgressState.Idle
            private set

        fun clearProgress() {
            aiEditState = TaskProgressState.Idle
        }

        fun clearColorGradingProgress() {
            colorGradingState = TaskProgressState.Idle
        }
    }

    @android.webkit.JavascriptInterface
    fun isAppVisible(): Boolean {
        return MainActivity.isAppVisible
    }

    @android.webkit.JavascriptInterface
    fun openOrNavigateTo(uri: String, allUrisJson: String): Boolean {
        Log.d(TAG, "openOrNavigateTo: uri=$uri")
        return jsCall("openOrNavigateTo error", false) {
            val allUris = JSONArray(allUrisJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }
            val navigationTarget = ImageViewerActivity.buildNavigationTarget(allUris, uri)
            if (navigationTarget == null) {
                Log.e(TAG, "openViewer: target URI not found in list")
                return@jsCall false
            }
            ImageViewerActivity.navigateOrStart(
                activity,
                navigationTarget.uris,
                navigationTarget.targetIndex,
            )
            true
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
        val prev = aiEditState as? TaskProgressState.InProgress
        aiEditState = if (cancelled) {
            TaskProgressState.Idle
        } else {
            TaskProgressState.Done(success, prev?.total ?: 0, prev?.failedCount ?: 0)
        }
        val viewer = ImageViewerActivity.instance ?: return
        viewer.onAiEditComplete(success, message, cancelled)
    }

    @android.webkit.JavascriptInterface
    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        aiEditState = TaskProgressState.InProgress(current, total, failedCount)
        val viewer = ImageViewerActivity.instance ?: return
        viewer.updateAiEditProgress(current, total, failedCount)
    }

    /**
     * Inserts a newly created file into the active viewer immediately via file:// URI,
     * then triggers an async MediaStore scan so the file appears in the system gallery.
     *
     * The synchronous file:// insertion avoids depending on the async scan callback,
     * which may not fire reliably when ImageViewerActivity is in the foreground
     * (MainActivity's WebView may be paused, blocking the gallery-items-added event chain).
     */
    @android.webkit.JavascriptInterface
    fun scanNewFile(filePath: String?) {
        if (filePath == null) return
        val viewer = ImageViewerActivity.instance
        val context = (viewer ?: activity) as? android.content.Context ?: return
        val mainActivity = MainActivity.instance ?: return

        // Insert into active viewer immediately using file:// URI.
        // This is synchronous and does not depend on MediaStore scan completion.
        if (viewer != null && ImageViewerActivity.isViewerVisible
            && !viewer.isFinishing && !viewer.isDestroyed) {
            val file = java.io.File(filePath)
            if (file.exists()) {
                val fileUri = android.net.Uri.fromFile(file).toString()
                // Mark as file-scheme so later content:// insertions from the WebView
                // event handler can detect and skip the duplicate.
                viewer.insertImage(fileUri, 0)
            }
        }

        // Also trigger async MediaStore scan for gallery visibility.
        android.media.MediaScannerConnection.scanFile(context, arrayOf(filePath), null,
            object : android.media.MediaScannerConnection.OnScanCompletedListener {
                override fun onScanCompleted(path: String?, uri: android.net.Uri?) {
                    if (uri == null) return
                    com.gjk.cameraftpcompanion.bridges.MediaStoreBridge.Companion
                        .emitGalleryItemsAdded(mainActivity, uri.toString())
                }
            }
        )
    }

    /**
     * Insert a new image into the currently visible viewer at a specific position.
     * No-op if the viewer is not visible or URI already exists in the list.
     * Also skips if a file:// URI for the same file is already present (avoids
     * inserting a content:// duplicate after scanNewFile's synchronous insertion).
     * @param uri Content URI of the new image
     * @param insertIndex Position to insert at (clamped to valid range by the activity)
     * @returns true if inserted into an active viewer
     */
    @android.webkit.JavascriptInterface
    fun insertImage(uri: String?, insertIndex: Int): Boolean {
        if (uri == null) return false
        val viewer = ImageViewerActivity.instance ?: return false
        if (!ImageViewerActivity.isViewerVisible) return false
        if (viewer.isFinishing || viewer.isDestroyed) return false
        // Check if a file:// URI for the same file already exists in the list
        val parsed = android.net.Uri.parse(uri)
        if (parsed.scheme == "content") {
            val filePath = ImageViewerActivity.resolveUriToFilePath(activity, uri)
            if (filePath != null) {
                val existingFileUri = android.net.Uri.fromFile(java.io.File(filePath)).toString()
                if (viewer.uris.contains(existingFileUri)) return false
            }
        }
        viewer.insertImage(uri, insertIndex)
        return true
    }

    /**
     * Navigate the currently visible viewer to an existing URI in its list.
     * No-op if the viewer is not visible or URI is not in the list.
     * @param uri Content URI to navigate to
     */
    @android.webkit.JavascriptInterface
    fun navigateToExistingUri(uri: String?) {
        if (uri == null) return
        val viewer = ImageViewerActivity.instance ?: return
        if (!ImageViewerActivity.isViewerVisible) return
        if (viewer.isFinishing || viewer.isDestroyed) return
        viewer.navigateToExistingUri(uri)
    }

    @android.webkit.JavascriptInterface
    fun updateColorGradingProgress(current: Int, total: Int, failedCount: Int) {
        colorGradingState = TaskProgressState.InProgress(current, total, failedCount)
        val viewer = ImageViewerActivity.instance ?: return
        viewer.updateColorGradingProgress(current, total, failedCount)
    }

    @android.webkit.JavascriptInterface
    fun onColorGradingComplete(success: Boolean, message: String?, cancelled: Boolean) {
        val prev = colorGradingState as? TaskProgressState.InProgress
        colorGradingState = if (cancelled) {
            TaskProgressState.Idle
        } else {
            TaskProgressState.Done(success, prev?.total ?: 0, prev?.failedCount ?: 0)
        }
        val viewer = ImageViewerActivity.instance ?: return
        viewer.onColorGradingComplete(success, message, cancelled)
    }

    /**
     * Request EXIF data for multiple image positions.
     * Called from JS to prefetch EXIF for offscreen pages.
     */
    @android.webkit.JavascriptInterface
    fun requestExifForPositions(requestJson: String?) {
        if (requestJson == null) return
        jsCall("requestExifForPositions error", Unit) {
            // Validate JSON; pass through to the activity for JS evaluation
            org.json.JSONArray(requestJson)
            val viewer = ImageViewerActivity.instance ?: return@jsCall
            viewer.requestExifPrefetch(requestJson)
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
