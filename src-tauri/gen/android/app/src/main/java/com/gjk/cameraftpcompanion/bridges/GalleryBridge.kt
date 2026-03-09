/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.annotation.SuppressLint
import android.content.Context
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

class GalleryBridge(private val context: Context) : BaseJsBridge(context as android.app.Activity) {

    companion object {
        private const val TAG = "GalleryBridge"
        private const val THUMBNAIL_QUALITY = 85
    }

    @android.webkit.JavascriptInterface
    fun getGalleryImages(storagePath: String): String {
        Log.d(TAG, "getGalleryImages: storagePath=$storagePath")
        
        val images = JSONArray()
        
        try {
            val imagesDir = File(storagePath)
            if (!imagesDir.exists() || !imagesDir.isDirectory) {
                Log.w(TAG, "Directory does not exist: $storagePath")
                return createResult(images)
            }

            // Query MediaStore for images in the specified directory
            val projection = arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DISPLAY_NAME,
                MediaStore.Images.Media.DATA,
                MediaStore.Images.Media.DATE_MODIFIED
            )

            val selection = "${MediaStore.Images.Media.DATA} LIKE ?"
            val selectionArgs = arrayOf("$storagePath%")
            val sortOrder = "${MediaStore.Images.Media.DATE_MODIFIED} DESC"

            val cursor: Cursor? = context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                projection,
                selection,
                selectionArgs,
                sortOrder
            )

            cursor?.use {
                val idColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val nameColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME)
                val dataColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATA)
                val dateColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)

                while (it.moveToNext()) {
                    val id = it.getLong(idColumn)
                    val name = it.getString(nameColumn)
                    val path = it.getString(dataColumn)
                    val dateModified = it.getLong(dateColumn) * 1000 // Convert to milliseconds

                    // Get thumbnail using MediaStore
                    val thumbnail = getThumbnail(id)

                    val imageJson = JSONObject().apply {
                        put("id", id)
                        put("path", path)
                        put("filename", name)
                        put("thumbnail", thumbnail)
                        put("dateModified", dateModified)
                    }
                    images.put(imageJson)
                }
            }

            Log.d(TAG, "getGalleryImages: found ${images.length()} images")
        } catch (e: Exception) {
            Log.e(TAG, "getGalleryImages error", e)
        }

        return createResult(images)
    }

    @SuppressLint("Recycle")
    private fun getThumbnail(imageId: Long): String {
        return try {
            // Try to get cached thumbnail from MediaStore first
            val thumbnail = MediaStore.Images.Thumbnails.getThumbnail(
                context.contentResolver,
                imageId,
                MediaStore.Images.Thumbnails.MINI_KIND,
                null
            )

            if (thumbnail != null) {
                bitmapToBase64(thumbnail)
            } else {
                // Fallback: create thumbnail manually
                createThumbnailManually(imageId)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to get thumbnail for imageId=$imageId", e)
            ""
        }
    }

    private fun createThumbnailManually(imageId: Long): String {
        val projection = arrayOf(MediaStore.Images.Media.DATA)
        val selection = "${MediaStore.Images.Media._ID} = ?"
        val selectionArgs = arrayOf(imageId.toString())

        context.contentResolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            projection,
            selection,
            selectionArgs,
            null
        )?.use { cursor ->
            if (cursor.moveToFirst()) {
                val path = cursor.getString(cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATA))
                val file = File(path)
                if (file.exists()) {
                    val options = BitmapFactory.Options().apply {
                        inJustDecodeBounds = true
                    }
                    BitmapFactory.decodeFile(path, options)

                    // Calculate sample size for thumbnail (~512px)
                    val sampleSize = calculateSampleSize(options.outWidth, options.outHeight, 512, 384)
                    options.inJustDecodeBounds = false
                    options.inSampleSize = sampleSize

                    val bitmap = BitmapFactory.decodeFile(path, options)
                    return bitmap?.let { bitmapToBase64(it) } ?: ""
                }
            }
        }
        return ""
    }

    private fun calculateSampleSize(width: Int, height: Int, reqWidth: Int, reqHeight: Int): Int {
        var sampleSize = 1
        if (height > reqHeight || width > reqWidth) {
            val halfHeight = height / 2
            val halfWidth = width / 2
            while (halfHeight / sampleSize >= reqHeight && halfWidth / sampleSize >= reqWidth) {
                sampleSize *= 2
            }
        }
        return sampleSize
    }

    private fun bitmapToBase64(bitmap: Bitmap): String {
        val outputStream = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.JPEG, THUMBNAIL_QUALITY, outputStream)
        val byteArray = outputStream.toByteArray()
        val base64 = Base64.encodeToString(byteArray, Base64.NO_WRAP)
        return "data:image/jpeg;base64,$base64"
    }

    private fun createResult(images: JSONArray): String {
        return JSONObject().apply {
            put("images", images)
        }.toString()
    }
}
