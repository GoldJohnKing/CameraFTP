/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.app.RecoverableSecurityException
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import com.gjk.cameraftpcompanion.MainActivity
import org.json.JSONArray
import org.json.JSONObject

class GalleryBridge(private val context: Context) : BaseJsBridge(context as android.app.Activity) {

    companion object {
        private const val TAG = "GalleryBridge"

        @JvmStatic
        fun shouldRequestDeleteConfirmation(
            apiLevel: Int,
            isSecurityException: Boolean,
            isRecoverableSecurityException: Boolean,
        ): Boolean {
            if (!isSecurityException) {
                return false
            }

            return when {
                apiLevel >= Build.VERSION_CODES.R -> true
                apiLevel == Build.VERSION_CODES.Q -> isRecoverableSecurityException
                else -> false
            }
        }

        @JvmStatic
        fun shouldRequestDeleteConfirmation(apiLevel: Int, throwable: Throwable): Boolean {
            return shouldRequestDeleteConfirmation(
                apiLevel = apiLevel,
                isSecurityException = throwable is SecurityException,
                isRecoverableSecurityException = throwable is RecoverableSecurityException,
            )
        }

        /**
         * Build share intent using MediaStore URIs
         * Follows Android 10+ best practices:
         * - Sets ClipData for permission propagation
         * - Sets FLAG_GRANT_READ_URI_PERMISSION on the intent itself
         */
        @JvmStatic
        fun build_share_intent(uris: List<String>): Intent {
            val uriObjects = uris.map { Uri.parse(it) }

            return if (uris.size == 1) {
                Intent(Intent.ACTION_SEND).apply {
                    type = "image/*"
                    putExtra(Intent.EXTRA_STREAM, uriObjects[0])
                    clipData = ClipData.newRawUri(null, uriObjects[0])
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
            } else {
                Intent(Intent.ACTION_SEND_MULTIPLE).apply {
                    type = "image/*"
                    putParcelableArrayListExtra(
                        Intent.EXTRA_STREAM,
                        ArrayList(uriObjects)
                    )
                    val data = ClipData.newRawUri(null, uriObjects[0])
                    for (i in 1 until uriObjects.size) {
                        data.addItem(ClipData.Item(uriObjects[i]))
                    }
                    setClipData(data)
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
            }
        }

    }


    @android.webkit.JavascriptInterface
    fun deleteImages(urisJson: String): String {
        Log.d(TAG, "deleteImages: urisJson=$urisJson")

        return try {
            val uris = JSONArray(urisJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            if (uris.isEmpty()) {
                Log.w(TAG, "deleteImages: no URIs provided")
                return """{"deleted":[],"notFound":[],"failed":[]}"""
            }

            val deleted = mutableListOf<String>()
            val notFound = mutableListOf<String>()
            val failed = mutableListOf<String>()
            val pendingConfirmationUris = mutableListOf<Uri>()

            uris.forEach { uriString ->
                try {
                    val uri = Uri.parse(uriString)
                    
                    // Delete via MediaStore using URI
                    val rowsDeleted = context.contentResolver.delete(uri, null, null)
                    classifyDeleteResult(uriString, rowsDeleted, deleted, notFound, failed)
                } catch (e: Exception) {
                    if (Build.VERSION.SDK_INT == Build.VERSION_CODES.Q && e is RecoverableSecurityException) {
                        val approved = (activity as? MainActivity)
                            ?.requestDeleteConfirmation(e.userAction.actionIntent.intentSender)
                            ?: false
                        classifyDeleteResult(uriString, if (approved) 1 else 0, deleted, notFound, failed)
                    } else if (shouldRequestDeleteConfirmation(Build.VERSION.SDK_INT, e)) {
                        pendingConfirmationUris.add(Uri.parse(uriString))
                        Log.w(TAG, "deleteImages: delete confirmation required for uri=$uriString", e)
                    } else {
                        Log.e(TAG, "Error deleting URI: $uriString", e)
                        failed.add(uriString)
                    }
                }
            }

            if (pendingConfirmationUris.isNotEmpty()) {
                val deleteConfirmed = requestDeleteConfirmation(pendingConfirmationUris)
                pendingConfirmationUris.forEach { uri ->
                    classifyDeleteResult(uri.toString(), if (deleteConfirmed) 1 else 0, deleted, notFound, failed)
                }
            }

            Log.d(TAG, "deleteImages: deleted=${deleted.size}, notFound=${notFound.size}, failed=${failed.size}")

            // Build JSON response using JSONObject
            val response = JSONObject().apply {
                put("deleted", JSONArray(deleted))
                put("notFound", JSONArray(notFound))
                put("failed", JSONArray(failed))
            }
            response.toString()
        } catch (e: Exception) {
            Log.e(TAG, "deleteImages error", e)
            """{"deleted":[],"notFound":[],"failed":[]}"""
        }
    }

    private fun requestDeleteConfirmation(uris: List<Uri>): Boolean {
        val mainActivity = activity as? MainActivity ?: return false

        return try {
            when {
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> {
                    val pendingIntent = MediaStore.createDeleteRequest(context.contentResolver, uris)
                    mainActivity.requestDeleteConfirmation(pendingIntent.intentSender)
                }

                Build.VERSION.SDK_INT == Build.VERSION_CODES.Q -> {
                    false
                }

                else -> false
            }
        } catch (e: Exception) {
            Log.e(TAG, "requestDeleteConfirmation failed", e)
            false
        }
    }

    private fun classifyDeleteResult(
        uriString: String,
        rowsDeleted: Int,
        deleted: MutableList<String>,
        notFound: MutableList<String>,
        failed: MutableList<String>,
    ) {
        val stillExists = try {
            val cursor = context.contentResolver.query(Uri.parse(uriString), null, null, null, null)
            cursor?.use { it.count > 0 } ?: false
        } catch (_: Exception) {
            false
        }

        if (!stillExists) {
            if (rowsDeleted > 0) {
                deleted.add(uriString)
                Log.d(TAG, "Deleted image via MediaStore: uri=$uriString")
            } else {
                notFound.add(uriString)
                Log.d(TAG, "Image not found in MediaStore: uri=$uriString")
            }
        } else {
            failed.add(uriString)
            Log.w(TAG, "Failed to delete image (still exists): uri=$uriString")
        }
    }



    @android.webkit.JavascriptInterface
    fun shareImages(urisJson: String): Boolean {
        Log.d(TAG, "shareImages: urisJson=$urisJson")

        return try {
            val uris = JSONArray(urisJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            if (uris.isEmpty()) {
                Log.w(TAG, "shareImages: no URIs provided")
                return false
            }

            val intent = build_share_intent(uris)
            
            val chooser = Intent.createChooser(intent, "分享图片").apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(chooser)

            Log.d(TAG, "shareImages: shared ${uris.size} images")
            true
        } catch (e: Exception) {
            Log.e(TAG, "shareImages error", e)
            false
        }
    }

    /**
     * Register back press callback to intercept back button
     * Called from JS when entering selection mode
     */
    @android.webkit.JavascriptInterface
    fun registerBackPressCallback(): Boolean {
        return try {
            (activity as? MainActivity)?.registerBackPressCallback() ?: false
        } catch (e: Exception) {
            Log.e(TAG, "registerBackPressCallback: exception", e)
            false
        }
    }

    /**
     * Unregister back press callback
     * Called from JS when exiting selection mode
     */
    @android.webkit.JavascriptInterface
    fun unregisterBackPressCallback(): Boolean {
        return try {
            (activity as? MainActivity)?.unregisterBackPressCallback() ?: false
        } catch (e: Exception) {
            Log.e(TAG, "unregisterBackPressCallback: exception", e)
            false
        }
    }

}
