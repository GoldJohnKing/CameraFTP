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
import org.json.JSONArray

class ImageViewerBridge(activity: android.app.Activity) : BaseJsBridge(activity) {

    companion object {
        private const val TAG = "ImageViewerBridge"
    }

    @android.webkit.JavascriptInterface
    fun openViewer(uri: String, allUrisJson: String): Boolean {
        Log.d(TAG, "openViewer: uri=$uri")
        return try {
            val allUris = JSONArray(allUrisJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }
            val targetIndex = allUris.indexOf(uri)
            if (targetIndex == -1) {
                Log.e(TAG, "openViewer: target URI not found in list")
                return false
            }
            ImageViewerActivity.start(activity, allUris, targetIndex)
            true
        } catch (e: Exception) {
            Log.e(TAG, "openViewer error", e)
            false
        }
    }

    @android.webkit.JavascriptInterface
    fun closeViewer(): Boolean {
        Log.d(TAG, "closeViewer")
        return try {
            ImageViewerActivity.instance?.finish() ?: run {
                Log.w(TAG, "closeViewer: no active ImageViewerActivity")
            }
            true
        } catch (e: Exception) {
            Log.e(TAG, "closeViewer error", e)
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
            val projection = arrayOf(MediaStore.Images.Media.DATA)
            activity.contentResolver.query(contentUri, projection, null, null, null)?.use { cursor ->
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
}
