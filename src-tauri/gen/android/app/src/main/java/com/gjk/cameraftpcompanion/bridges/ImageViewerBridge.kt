/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.net.Uri
import android.provider.MediaStore
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

        @Volatile
        var lastProgress: AiEditProgressState? = null
            private set

        @Volatile
        var isAiEditing: Boolean = false
            private set

        fun clearProgress() {
            lastProgress = null
            isAiEditing = false
        }
    }

    @android.webkit.JavascriptInterface
    fun isAppVisible(): Boolean {
        return MainActivity.isAppVisible
    }

    @android.webkit.JavascriptInterface
    fun openOrNavigateTo(uri: String, allUrisJson: String, aiEditEnabled: Boolean = false): Boolean {
        Log.d(TAG, "openOrNavigateTo: uri=$uri, aiEditEnabled=$aiEditEnabled")
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
                aiEditEnabled,
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
     * Resolve a content:// URI to a real file system path.
     * Returns null if the URI cannot be resolved.
     */
    @android.webkit.JavascriptInterface
    fun resolveFilePath(uri: String): String? {
        return try {
            val contentUri = Uri.parse(uri)
            if (contentUri.scheme != "content") return uri
            activity.contentResolver.query(contentUri, arrayOf(MediaStore.Images.Media.DATA), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(MediaStore.Images.Media.DATA)
                    if (idx >= 0) cursor.getString(idx) else null
                } else null
            }
        } catch (e: Exception) {
            Log.e(TAG, "resolveFilePath failed for $uri", e)
            null
        }
    }

    /**
     * Called from JS when an AI edit triggered from native completes.
     */
    @android.webkit.JavascriptInterface
    fun onAiEditComplete(success: Boolean, message: String?) {
        if (success) clearProgress()
        val viewer = ImageViewerActivity.instance ?: return
        viewer.onAiEditComplete(success, message)
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
}
