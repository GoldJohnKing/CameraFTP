/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import com.gjk.cameraftpcompanion.MainActivity
import org.json.JSONArray
import java.io.ByteArrayOutputStream
import java.io.File

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
    private fun getThumbnailCacheFile(imagePath: String): File {
        val md5 = imagePath.toByteArray().md5()
        return File(getThumbnailCacheDir(), "thumb_$md5.jpg")
    }

    /**
     * MD5 哈希
     */
    private fun ByteArray.md5(): String {
        val md = java.security.MessageDigest.getInstance("MD5")
        val digest = md.digest(this)
        return digest.joinToString("") { "%02x".format(it) }
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
     * Get thumbnail for a single image (for lazy loading).
     * This is called on-demand when an image becomes visible.
     * Returns the file path to the cached thumbnail, which can be loaded via convertFileSrc().
     */
    @android.webkit.JavascriptInterface
    fun getThumbnail(imagePath: String): String {
        Log.d(TAG, "getThumbnail: imagePath=$imagePath")
        return try {
            getThumbnailWithCache(imagePath)
        } catch (e: Exception) {
            Log.e(TAG, "getThumbnail error for imagePath=$imagePath", e)
            ""
        }
    }

    /**
     * 获取缩略图并缓存到文件系统
     * 返回缓存文件的绝对路径，前端通过 convertFileSrc() 转换为 asset:// URL 加载
     */
    private fun getThumbnailWithCache(imagePath: String): String {
        val cacheFile = getThumbnailCacheFile(imagePath)

        // 检查缓存是否已存在且有效（24小时内）
        if (cacheFile.exists() && cacheFile.length() > 0) {
            val age = System.currentTimeMillis() - cacheFile.lastModified()
            if (age < 24 * 60 * 60 * 1000) {  // 24小时
                Log.d(TAG, "Using cached thumbnail: ${cacheFile.absolutePath}")
                return cacheFile.absolutePath
            }
        }

        // 生成缩略图
        val bitmap = getThumbnailBitmap(imagePath) ?: return ""

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
    private fun getThumbnailBitmap(imagePath: String): Bitmap? {
        val file = File(imagePath)
        if (!file.exists()) return null
        return createThumbnailFromFile(file)
    }

    /**
     * Deletion result for a single file
     */
    data class FileDeletionResult(
        val path: String,
        val success: Boolean,
        val existed: Boolean
    )

    @android.webkit.JavascriptInterface
    fun deleteImages(pathsJson: String): String {
        Log.d(TAG, "deleteImages: pathsJson=$pathsJson")

        return try {
            val paths = JSONArray(pathsJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            if (paths.isEmpty()) {
                Log.w(TAG, "deleteImages: no paths provided")
                return """{"deleted":[],"notFound":[],"failed":[]}"""
            }

            val deleted = mutableListOf<String>()
            val notFound = mutableListOf<String>()
            val failed = mutableListOf<String>()

            paths.forEach { path ->
                val file = File(path)
                val existed = file.exists()

                if (existed) {
                    // Try to delete via MediaStore first (for proper media index update)
                    val uri = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
                    val rowsDeleted = context.contentResolver.delete(
                        uri,
                        "${MediaStore.Images.Media.DATA}=?",
                        arrayOf(path)
                    )

                    // Also delete the actual file if it still exists
                    val fileDeleted = file.exists() && file.delete()

                    if (fileDeleted || rowsDeleted > 0) {
                        deleted.add(path)
                        Log.d(TAG, "Deleted image path=$path")
                    } else {
                        failed.add(path)
                        Log.w(TAG, "Failed to delete image path=$path")
                    }
                } else {
                    // File doesn't exist, treat as "deleted" for animation purposes
                    notFound.add(path)
                    Log.d(TAG, "File not found, will remove from UI path=$path")
                }
            }

            Log.d(TAG, "deleteImages: deleted=${deleted.size}, notFound=${notFound.size}, failed=${failed.size}")

            // Build JSON response
            val deletedJson = deleted.joinToString(",", "[", "]") { "\"${escapeJson(it)}\"" }
            val notFoundJson = notFound.joinToString(",", "[", "]") { "\"${escapeJson(it)}\"" }
            val failedJson = failed.joinToString(",", "[", "]") { "\"${escapeJson(it)}\"" }
            
            "{\"deleted\":$deletedJson,\"notFound\":$notFoundJson,\"failed\":$failedJson}"
        } catch (e: Exception) {
            Log.e(TAG, "deleteImages error", e)
            """{"deleted":[],"notFound":[],"failed":[]}"""
        }
    }

    /**
     * Escape special characters in JSON string
     */
    private fun escapeJson(str: String): String {
        return str
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")
    }

    /**
     * Remove thumbnail cache files for deleted images
     * Called by frontend after delete animation completes
     */
    @android.webkit.JavascriptInterface
    fun removeThumbnails(pathsJson: String): Boolean {
        Log.d(TAG, "removeThumbnails: pathsJson=$pathsJson")

        return try {
            val paths = JSONArray(pathsJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            var removedCount = 0
            paths.forEach { path ->
                val cacheFile = getThumbnailCacheFile(path)
                if (cacheFile.exists() && cacheFile.delete()) {
                    removedCount++
                    Log.d(TAG, "Removed thumbnail cache for path=$path")
                }
            }

            Log.d(TAG, "removeThumbnails: removed $removedCount/${paths.size} thumbnails")
            removedCount > 0
        } catch (e: Exception) {
            Log.e(TAG, "removeThumbnails error", e)
            false
        }
    }

    @android.webkit.JavascriptInterface
    fun shareImages(pathsJson: String): Boolean {
        Log.d(TAG, "shareImages: pathsJson=$pathsJson")

        return try {
            val paths = JSONArray(pathsJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            if (paths.isEmpty()) {
                Log.w(TAG, "shareImages: no paths provided")
                return false
            }

            // Convert file paths to content URIs via FileProvider
            val uris = paths.map { path ->
                val file = File(path)
                androidx.core.content.FileProvider.getUriForFile(
                    context,
                    "${context.packageName}.fileprovider",
                    file
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

    /**
     * 从文件创建缩略图
     * 返回 Bitmap，由调用者决定如何保存
     */
    private fun createThumbnailFromFile(file: File): Bitmap? {
        if (!file.exists()) return null

        val options = BitmapFactory.Options().apply {
            inJustDecodeBounds = true
        }
        BitmapFactory.decodeFile(file.absolutePath, options)

        val sampleSize = calculateSampleSize(
            options.outWidth,
            options.outHeight,
            THUMBNAIL_WIDTH,
            THUMBNAIL_HEIGHT
        )
        options.inJustDecodeBounds = false
        options.inSampleSize = sampleSize

        return BitmapFactory.decodeFile(file.absolutePath, options)
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
