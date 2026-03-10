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
        private const val THUMBNAIL_WIDTH = 400  // 增大尺寸以获得更好的显示效果
        private const val THUMBNAIL_HEIGHT = 400
        private const val MAX_CACHE_SIZE_MB = 100  // 最大缓存 100MB
        private const val THUMBNAIL_SUBDIR = "thumbnails"
    }

    /**
     * 获取缩略图缓存目录
     */
    private fun getThumbnailCacheDir(): File {
        return File(context.cacheDir, THUMBNAIL_SUBDIR).apply {
            if (!exists()) mkdirs()
        }
    }

    /**
     * 获取缩略图缓存文件路径
     */
    private fun getThumbnailCacheFile(imageId: Long): File {
        return File(getThumbnailCacheDir(), "thumb_$imageId.jpg")
    }

    /**
     * 清理旧的缓存文件（LRU策略）
     */
    private fun cleanupOldCache() {
        try {
            val cacheDir = getThumbnailCacheDir()
            val files = cacheDir.listFiles() ?: return
            
            // 计算总大小
            val totalSize = files.sumOf { it.length() }
            val maxSizeBytes = MAX_CACHE_SIZE_MB * 1024 * 1024
            
            if (totalSize > maxSizeBytes) {
                // 按最后修改时间排序，删除最旧的
                files.sortBy { it.lastModified() }
                var currentSize = totalSize
                
                for (file in files) {
                    if (currentSize <= maxSizeBytes * 0.7) break  // 清理到 70%
                    currentSize -= file.length()
                    file.delete()
                }
                
                Log.d(TAG, "Cleaned up thumbnail cache. Reduced from ${totalSize / 1024 / 1024}MB to ${currentSize / 1024 / 1024}MB")
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to cleanup cache", e)
        }
    }

    /**
     * Get image metadata only (fast, for initial load).
     * Thumbnails should be loaded separately via getThumbnail().
     */
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
                    // Use MediaStore's DATE_MODIFIED for fast sorting
                    // This avoids file I/O and EXIF reading for each image
                    val dateModified = it.getLong(dateColumn) * 1000 // Convert to milliseconds

                    val imageJson = JSONObject().apply {
                        put("id", id)
                        put("path", path)
                        put("filename", name)
                        put("dateModified", dateModified)
                        put("sortTime", dateModified) // Use file time for fast initial load
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

    /**
     * Get thumbnail for a single image (for lazy loading).
     * This is called on-demand when an image becomes visible.
     * Returns the file path to the cached thumbnail, which can be loaded via convertFileSrc().
     */
    @android.webkit.JavascriptInterface
    fun getThumbnail(imageId: Long): String {
        Log.d(TAG, "getThumbnail: imageId=$imageId")
        return try {
            getThumbnailWithCache(imageId)
        } catch (e: Exception) {
            Log.e(TAG, "getThumbnail error for imageId=$imageId", e)
            ""
        }
    }

    /**
     * 获取缩略图并缓存到文件系统
     * 返回缓存文件的绝对路径，前端通过 convertFileSrc() 转换为 asset:// URL 加载
     */
    private fun getThumbnailWithCache(imageId: Long): String {
        val cacheFile = getThumbnailCacheFile(imageId)
        
        // 检查缓存是否已存在且有效（24小时内）
        if (cacheFile.exists() && cacheFile.length() > 0) {
            val age = System.currentTimeMillis() - cacheFile.lastModified()
            if (age < 24 * 60 * 60 * 1000) {  // 24小时
                Log.d(TAG, "Using cached thumbnail: ${cacheFile.absolutePath}")
                return cacheFile.absolutePath
            }
        }
        
        // 生成缩略图
        val bitmap = getThumbnailBitmap(imageId) ?: return ""
        
        // 保存到缓存
        try {
            cacheFile.outputStream().use { out ->
                bitmap.compress(Bitmap.CompressFormat.JPEG, THUMBNAIL_QUALITY, out)
            }
            
            // 检查并清理旧缓存
            cleanupOldCache()
            
            Log.d(TAG, "Saved thumbnail to cache: ${cacheFile.absolutePath}")
            return cacheFile.absolutePath
        } catch (e: Exception) {
            Log.e(TAG, "Failed to save thumbnail to cache", e)
            // 失败时回退到 Base64（确保兼容性）
            return bitmapToBase64(bitmap)
        }
    }

    /**
     * 获取缩略图 Bitmap
     */
    private fun getThumbnailBitmap(imageId: Long): Bitmap? {
        // 首先尝试从 MediaStore 获取缓存的缩略图
        val thumbnail = MediaStore.Images.Thumbnails.getThumbnail(
            context.contentResolver,
            imageId,
            MediaStore.Images.Thumbnails.MINI_KIND,
            null
        )
        
        return if (thumbnail != null) {
            thumbnail
        } else {
            // 手动生成缩略图
            createThumbnailManually(imageId)
        }
    }

    /**
     * Get accurate EXIF-based sort time for an image.
     * Called separately to avoid blocking initial load.
     */
    @android.webkit.JavascriptInterface
    fun getImageSortTime(imageId: Long): Long {
        return try {
            val path = getImagePath(imageId)
            if (path != null) {
                val exifTime = getExifDateTime(path)
                if (exifTime > 0) exifTime else 0L
            } else {
                0L
            }
        } catch (e: Exception) {
            Log.e(TAG, "getImageSortTime error for imageId=$imageId", e)
            0L
        }
    }

    /**
     * Get the latest image from the specified directory using MediaStore.
     * Uses DATE_MODIFIED for sorting (fast, consistent with getGalleryImages).
     * This replaces Rust FileIndex for Android platform to avoid data inconsistency.
     *
     * @param storagePath The directory path to query
     * @return JSON string containing { id, path, filename, dateModified } or null if not found
     */
    @android.webkit.JavascriptInterface
    fun getLatestImage(storagePath: String): String {
        Log.d(TAG, "getLatestImage: storagePath=$storagePath")

        return try {
            val imagesDir = File(storagePath)
            if (!imagesDir.exists() || !imagesDir.isDirectory) {
                Log.w(TAG, "Directory does not exist: $storagePath")
                return "null"
            }

            // Normalize the path for comparison (remove trailing slash for consistent matching)
            val normalizedPath = storagePath.removeSuffix("/")
            Log.d(TAG, "getLatestImage: normalizedPath=$normalizedPath")

            val projection = arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DISPLAY_NAME,
                MediaStore.Images.Media.DATA,
                MediaStore.Images.Media.DATE_MODIFIED
            )

            // Query all images and filter by path prefix (more reliable than LIKE query)
            val sortOrder = "${MediaStore.Images.Media.DATE_MODIFIED} DESC"

            context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                projection,
                null,  // No selection - we'll filter in code
                null,
                sortOrder
            )?.use { cursor ->
                val idColumn = cursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val nameColumn = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME)
                val dataColumn = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATA)
                val dateColumn = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)

                while (cursor.moveToNext()) {
                    val id = cursor.getLong(idColumn)
                    val name = cursor.getString(nameColumn)
                    val path = cursor.getString(dataColumn)
                    val dateModified = cursor.getLong(dateColumn) * 1000

                    // Check if this image is in the target directory (path starts with normalizedPath)
                    if (path.startsWith(normalizedPath)) {
                        val result = JSONObject().apply {
                            put("id", id)
                            put("path", path)
                            put("filename", name)
                            put("dateModified", dateModified)
                        }.toString()

                        Log.d(TAG, "getLatestImage: found $name at $path")
                        return result
                    }
                }
            }

            Log.d(TAG, "getLatestImage: no images found in $normalizedPath")
            "null"
        } catch (e: Exception) {
            Log.e(TAG, "getLatestImage error", e)
            "null"
        }
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
            deletedCount > 0
        } catch (e: Exception) {
            Log.e(TAG, "deleteImages error", e)
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
            false
        }
    }

    private fun getImagePath(imageId: Long): String? {
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
                return cursor.getString(cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATA))
            }
        }
        return null
    }

    /**
     * 手动生成缩略图（当 MediaStore 缓存不存在时）
     * 返回 Bitmap，由调用者决定如何保存
     */
    private fun createThumbnailManually(imageId: Long): Bitmap? {
        val path = getImagePath(imageId) ?: return null
        val file = File(path)
        if (!file.exists()) return null

        val options = BitmapFactory.Options().apply {
            inJustDecodeBounds = true
        }
        BitmapFactory.decodeFile(path, options)

        // Calculate sample size for thumbnail (使用 THUMBNAIL_WIDTH/HEIGHT)
        val sampleSize = calculateSampleSize(options.outWidth, options.outHeight, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT)
        options.inJustDecodeBounds = false
        options.inSampleSize = sampleSize

        return BitmapFactory.decodeFile(path, options)
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
