/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.annotation.SuppressLint
import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.ExifInterface
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File
import java.text.SimpleDateFormat
import java.util.Locale

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
                    // Use file system lastModified time (consistent with Rust side)
                    val dateModified = File(path).lastModified()

                    // Get thumbnail using MediaStore
                    val thumbnail = getThumbnail(id)

                    // Prefer EXIF capture time over file modification time
                    val exifTime = getExifDateTime(path)
                    val sortTime = if (exifTime > 0) exifTime else dateModified

                    val imageJson = JSONObject().apply {
                        put("id", id)
                        put("path", path)
                        put("filename", name)
                        put("thumbnail", thumbnail)
                        put("dateModified", dateModified)
                        put("sortTime", sortTime)
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

    @android.webkit.JavascriptInterface
    fun deleteImages(idsJson: String): Boolean {
        Log.d(TAG, "deleteImages: idsJson=$idsJson")
        
        return try {
            val ids = JSONArray(idsJson).let { json ->
                (0 until json.length()).map { json.getInt(it) }
            }
            
            if (ids.isEmpty()) {
                Log.w(TAG, "deleteImages: no IDs provided")
                return false
            }
            
            val uri = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
            var deletedCount = 0
            
            ids.forEach { id ->
                val contentUri = ContentUris.withAppendedId(uri, id.toLong())
                val deleted = context.contentResolver.delete(contentUri, null, null)
                if (deleted > 0) {
                    deletedCount++
                    Log.d(TAG, "Deleted image id=$id")
                }
            }
            
            Log.d(TAG, "deleteImages: deleted $deletedCount/${ids.size} images")
            activity.runOnUiThread {
                Toast.makeText(context, "已删除 $deletedCount 张图片", Toast.LENGTH_SHORT).show()
            }
            
            deletedCount > 0
        } catch (e: Exception) {
            Log.e(TAG, "deleteImages error", e)
            activity.runOnUiThread {
                Toast.makeText(context, "删除失败: ${e.message}", Toast.LENGTH_SHORT).show()
            }
            false
        }
    }

    @android.webkit.JavascriptInterface
    fun shareImages(idsJson: String): Boolean {
        Log.d(TAG, "shareImages: idsJson=$idsJson")
        
        return try {
            val ids = JSONArray(idsJson).let { json ->
                (0 until json.length()).map { json.getInt(it) }
            }
            
            if (ids.isEmpty()) {
                Log.w(TAG, "shareImages: no IDs provided")
                return false
            }
            
            val uris = ids.map { id ->
                ContentUris.withAppendedId(
                    MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                    id.toLong()
                )
            }
            
            val intent = if (uris.size == 1) {
                Intent(Intent.ACTION_SEND).apply {
                    type = "image/*"
                    putExtra(Intent.EXTRA_STREAM, uris[0])
                }
            } else {
                Intent(Intent.ACTION_SEND_MULTIPLE).apply {
                    type = "image/*"
                    putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
                }
            }
            
            val chooser = Intent.createChooser(intent, "分享图片").apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            context.startActivity(chooser)
            
            Log.d(TAG, "shareImages: shared ${uris.size} images")
            true
        } catch (e: Exception) {
            Log.e(TAG, "shareImages error", e)
            activity.runOnUiThread {
                Toast.makeText(context, "分享失败: ${e.message}", Toast.LENGTH_SHORT).show()
            }
            false
        }
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

    private fun getExifDateTime(path: String): Long {
        return try {
            val exif = ExifInterface(path)
            val dateTimeStr = exif.getAttribute(ExifInterface.TAG_DATETIME_ORIGINAL)
                ?: exif.getAttribute(ExifInterface.TAG_DATETIME)
            
            if (dateTimeStr != null) {
                val format = SimpleDateFormat("yyyy:MM:dd HH:mm:ss", Locale.US)
                format.parse(dateTimeStr)?.time ?: 0L
            } else {
                0L
            }
        } catch (e: Exception) {
            0L
        }
    }

    private fun createResult(images: JSONArray): String {
        return JSONObject().apply {
            put("images", images)
        }.toString()
    }
}
