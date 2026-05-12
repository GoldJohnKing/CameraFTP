/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.controllers

import android.app.Activity
import android.content.IntentSender
import android.net.Uri
import android.provider.MediaStore
import android.util.Log
import android.widget.Toast
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.IntentSenderRequest
import com.gjk.cameraftpcompanion.ImageViewerActivity
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.R
import org.json.JSONArray
import java.lang.ref.WeakReference

class DeleteController(
    activity: ImageViewerActivity,
    private val deleteRequestLauncher: ActivityResultLauncher<IntentSenderRequest>,
) {
    private companion object {
        private const val TAG = "DeleteController"
    }

    private val activityRef: WeakReference<ImageViewerActivity> = WeakReference(activity)
    private var pendingDeleteUri: String? = null

    fun deleteCurrentImage(uriString: String, uris: MutableList<String>, currentIndex: Int) {
        val activity = activityRef.get() ?: return
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = activity.contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(activity, uri)
            if (rowsDeleted > 0 || !stillExists) {
                applyDeleteSuccess(activity, uriString, uris, currentIndex)
            } else {
                Toast.makeText(activity, "删除失败：文件不存在", Toast.LENGTH_SHORT).show()
            }
        } catch (e: Exception) {
            if (e is SecurityException) {
                tryDeleteWithConfirmation(activity, uriString, uri, uris, currentIndex)
                return
            }
            Log.e(TAG, "Failed to delete image", e)
            Toast.makeText(activity, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    fun finalizeDeleteAfterConfirmation(uriString: String, uris: MutableList<String>, currentIndex: Int) {
        val activity = activityRef.get() ?: return
        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = activity.contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(activity, uri)
            if (rowsDeleted > 0 || !stillExists) {
                applyDeleteSuccess(activity, uriString, uris, currentIndex)
                return
            }
        } catch (e: SecurityException) {
            if (!uriStillExists(activity, uri)) {
                applyDeleteSuccess(activity, uriString, uris, currentIndex)
                return
            }
            Log.e(TAG, "Delete still blocked after confirmation", e)
            Toast.makeText(activity, "删除失败：无权限", Toast.LENGTH_SHORT).show()
            return
        } catch (e: Exception) {
            Log.e(TAG, "Failed to finalize delete after confirmation", e)
            Toast.makeText(activity, "删除失败", Toast.LENGTH_SHORT).show()
            return
        }

        Toast.makeText(activity, "删除失败", Toast.LENGTH_SHORT).show()
    }

    fun getPendingDeleteUri(): String? = pendingDeleteUri

    fun clearPendingDeleteUri() {
        pendingDeleteUri = null
    }

    private fun tryDeleteWithConfirmation(
        activity: ImageViewerActivity,
        uriString: String,
        uri: Uri,
        uris: MutableList<String>,
        currentIndex: Int,
    ) {
        if (!uriStillExists(activity, uri)) {
            applyDeleteSuccess(activity, uriString, uris, currentIndex)
            return
        }

        try {
            pendingDeleteUri = uriString
            val pendingIntent = MediaStore.createDeleteRequest(activity.contentResolver, listOf(uri))
            val request = IntentSenderRequest.Builder(pendingIntent.intentSender).build()
            deleteRequestLauncher.launch(request)
        } catch (e: Exception) {
            pendingDeleteUri = null
            Log.e(TAG, "Failed to launch delete confirmation", e)
            Toast.makeText(activity, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    private fun applyDeleteSuccess(
        activity: ImageViewerActivity,
        uriString: String,
        uris: MutableList<String>,
        currentIndex: Int,
    ) {
        val removedIndex = uris.indexOf(uriString)
        val mediaId = uriString.substringAfterLast("/")
        var newCurrentIndex = currentIndex

        if (removedIndex >= 0) {
            uris.removeAt(removedIndex)
            if (removedIndex < newCurrentIndex) {
                newCurrentIndex -= 1
            } else if (newCurrentIndex >= uris.size && uris.isNotEmpty()) {
                newCurrentIndex = uris.size - 1
            }
        }

        notifyMediaLibraryDeleted(activity, listOf(mediaId))

        if (uris.isEmpty()) {
            Toast.makeText(activity, "图片已删除", Toast.LENGTH_SHORT).show()
            activity.finish()
            return
        }

        activity.onDeleteSuccess(uris, newCurrentIndex)
        Toast.makeText(activity, "图片已删除", Toast.LENGTH_SHORT).show()
    }

    private fun uriStillExists(activity: Activity, uri: Uri): Boolean {
        return try {
            val cursor = activity.contentResolver.query(uri, arrayOf(MediaStore.Images.Media._ID), null, null, null)
            cursor?.use { it.moveToFirst() } ?: false
        } catch (_: Exception) {
            false
        }
    }

    private fun notifyMediaLibraryDeleted(activity: Activity, deletedMediaIds: List<String>) {
        val mainActivity = MainActivity.instance ?: return
        val deletedIdsJson = JSONArray(deletedMediaIds).toString()
        val deletePayload = "{\"mediaIds\":$deletedIdsJson,\"timestamp\":${System.currentTimeMillis()}}"
        mainActivity.emitWindowEvent("gallery-items-deleted", deletePayload)

        val refreshPayload = "{\"reason\":\"delete\",\"timestamp\":${System.currentTimeMillis()}}"
        mainActivity.emitWindowEvent("latest-photo-refresh-requested", refreshPayload)
    }
}
