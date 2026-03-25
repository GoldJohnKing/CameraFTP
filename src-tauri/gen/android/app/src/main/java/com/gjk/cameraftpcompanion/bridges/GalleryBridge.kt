/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.ClipData
import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.app.RecoverableSecurityException
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import android.widget.Toast
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.cache.ThumbnailCacheProvider
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

class GalleryBridge(private val context: Context) : BaseJsBridge(context as android.app.Activity) {

    /**
     * Media entry from MediaStore query
     */
    data class MediaEntry(
        val uri: String,
        val dateModified: Long,  // seconds
        val dateAdded: Long,     // seconds
        val dateTaken: Long,     // milliseconds (EXIF capture time)
        val id: Long,
        val size: Long
    )

    companion object {
        private const val TAG = "GalleryBridge"
        private const val THUMBNAIL_QUALITY = 92
        private const val THUMBNAIL_WIDTH = 720
        private const val THUMBNAIL_HEIGHT = 720
        private const val THUMBNAIL_SUBDIR = "thumbnails"
        private const val URI_WINDOW_SIZE = 25  // Number of URIs to include on each side of target

        /**
         * Pick the freshest/newest entry based on dateTaken (EXIF capture time), then dateAdded, then id
         * Using id as final tie-breaker (higher id = more recent in MediaStore)
         */
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

        @JvmStatic
        fun pick_newest(a: MediaEntry, b: MediaEntry): MediaEntry {
            // Compare by dateTaken first (actual photo capture time from EXIF)
            // dateTaken is in milliseconds, 0 means not available
            val aHasTaken = a.dateTaken > 0
            val bHasTaken = b.dateTaken > 0
            if (aHasTaken && bHasTaken) {
                if (a.dateTaken != b.dateTaken) {
                    return if (a.dateTaken > b.dateTaken) a else b
                }
            } else if (aHasTaken) {
                return a
            } else if (bHasTaken) {
                return b
            }

            // Fall back to dateAdded if dateTaken not available
            if (a.dateAdded != b.dateAdded) {
                return if (a.dateAdded > b.dateAdded) a else b
            }

            // If both are equal, prefer higher id (more recent in MediaStore)
            return if (a.id >= b.id) a else b
        }

        /**
         * Build a window of URIs around the target index for swipe browsing
         * Returns up to 51 URIs (target + 25 on each side)
         */
        @JvmStatic
        fun build_uri_window(all: List<String>, target_index: Int): List<String> {
            if (all.isEmpty()) return emptyList()
            
            val start = (target_index - URI_WINDOW_SIZE).coerceAtLeast(0)
            val end = (target_index + URI_WINDOW_SIZE).coerceAtMost(all.lastIndex)
            
            return all.subList(start, end + 1)
        }

        /**
         * Build MediaStore query selection for CameraFTP directory
         */
        @JvmStatic
        fun build_query_selection(): String {
            return "${MediaStore.Images.Media.RELATIVE_PATH} LIKE '%DCIM/CameraFTP/%'"
        }

        /**
         * Sort entries by dateTaken DESC (EXIF capture time, matches system gallery), then dateAdded DESC, then id DESC
         * Entries with dateTaken=0 (not available) are sorted after those with valid dateTaken
         * Using id as final tie-breaker ensures stable ordering (newer id = more recent in MediaStore)
         */
        @JvmStatic
        fun sort_entries(entries: List<MediaEntry>): List<MediaEntry> {
            return entries.sortedWith(
                compareByDescending<MediaEntry> { it.dateTaken }
                    .thenByDescending { it.dateAdded }
                    .thenByDescending { it.id }
            )
        }

        /**
         * Determine if toast should be shown when no handler is available
         */
        @JvmStatic
        fun should_show_no_handler_toast(has_handler: Boolean): Boolean = !has_handler

        /**
         * Should grant read permission when opening external gallery
         */
        @JvmStatic
        fun should_grant_read_permission(): Boolean = true

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

        /**
         * Build delete selection for MediaStore URI
         */
        @JvmStatic
        fun build_delete_selection(uri: String): String {
            if (!uri.startsWith("content://")) {
                return ""
            }
            return "${MediaStore.Images.Media._ID}=?"
        }

        @JvmStatic
        fun shouldRemoveCachedThumbnail(
            fileName: String,
            legacyKeys: Set<String>,
            activeKeys: Set<String>,
        ): Boolean {
            val key = fileName.removePrefix("thumb_").removeSuffix(".jpg")
            return key !in legacyKeys && key !in activeKeys
        }
    }

    /**
     * 获取缩略图缓存目录
     * Creates the directory if needed and validates writability
     */
    private fun getThumbnailCacheDir(): File {
        return File(context.cacheDir, THUMBNAIL_SUBDIR).apply {
            if (!exists()) {
                if (!mkdirs()) {
                    Log.e(TAG, "Failed to create thumbnail cache directory: $absolutePath")
                }
            }
            if (!canWrite()) {
                Log.e(TAG, "Thumbnail cache directory is not writable: $absolutePath")
            }
        }
    }

    /**
     * MD5 哈希 (kept for backward compatibility with existing cleanup logic)
     */
    private fun ByteArray.md5(): String {
        val md = java.security.MessageDigest.getInstance("MD5")
        val digest = md.digest(this)
        return digest.joinToString("") { "%02x".format(it) }
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
     * Uses ThumbnailCache for LRU eviction and dateModified-based invalidation
     * Supports MediaStore URIs (content://...)
     */
    private fun getThumbnailWithCache(imagePath: String): String {
        // Handle MediaStore URI
        val uri = if (imagePath.startsWith("content://")) {
            Uri.parse(imagePath)
        } else {
            // Legacy file path support
            val file = File(imagePath)
            if (!file.exists()) return ""
            Uri.fromFile(file)
        }

        // Query MediaStore for dateModified
        val dateModified = queryDateModifiedFromMediaStore(uri) ?: System.currentTimeMillis()
        val cache = ThumbnailCacheProvider.instance

        // Check if cached with current dateModified
        if (cache.contains(uri, dateModified)) {
            val cacheFile = cache.getCacheFile(uri, dateModified)
            if (cacheFile != null && cacheFile.exists() && cacheFile.length() > 0) {
                Log.d(TAG, "Using cached thumbnail: ${cacheFile.absolutePath}")
                return cacheFile.absolutePath
            }
        }

        // 生成缩略图 using MediaStore
        val bitmap = load_thumbnail(uri) ?: return ""

        // 保存到缓存
        try {
            val cacheFile = File(getThumbnailCacheDir(), "thumb_${cache.keyFor(uri, dateModified)}.jpg")
            cacheFile.outputStream().use { out ->
                bitmap.compress(Bitmap.CompressFormat.JPEG, THUMBNAIL_QUALITY, out)
            }

            // Track in cache for LRU eviction
            val bytes = cacheFile.length().toInt()
            cache.put(uri, dateModified, bytes)

            Log.d(TAG, "Saved thumbnail to cache: ${cacheFile.absolutePath}")
            return cacheFile.absolutePath
        } catch (e: Exception) {
            Log.e(TAG, "Failed to save thumbnail to cache", e)
            // 失败时回退到 Base64（确保兼容性）
            return bitmapToBase64(bitmap)
        }
    }

    /**
     * Query date modified from MediaStore for a URI
     */
    private fun queryDateModifiedFromMediaStore(uri: Uri): Long? {
        return try {
            val projection = arrayOf(MediaStore.Images.Media.DATE_MODIFIED)
            context.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val seconds = cursor.getLong(cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED))
                    seconds * 1000 // Convert to milliseconds
                } else null
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to query dateModified for uri=$uri", e)
            null
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



    /**
     * Remove thumbnail cache files for deleted images
     * Called by frontend after delete animation completes
     * Uses ThumbnailCache.evictIfPresent for proper cache management
     */
    @android.webkit.JavascriptInterface
    fun removeThumbnails(pathsJson: String): Boolean {
        Log.d(TAG, "removeThumbnails: pathsJson=$pathsJson")

        return try {
            val paths = JSONArray(pathsJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            var removedCount = 0
            val cache = ThumbnailCacheProvider.instance
            paths.forEach { path ->
                val file = File(path)
                if (file.exists()) {
                    val uri = Uri.fromFile(file)
                    cache.evictIfPresent(uri)
                    removedCount++
                    Log.d(TAG, "Removed thumbnail cache for path=$path")
                } else {
                    // File doesn't exist, try to delete any orphaned cache files
                    val oldCacheFile = File(getThumbnailCacheDir(), "thumb_${path.toByteArray().md5()}.jpg")
                    if (oldCacheFile.exists() && oldCacheFile.delete()) {
                        removedCount++
                        Log.d(TAG, "Removed orphaned thumbnail cache for path=$path")
                    }
                }
            }

            Log.d(TAG, "removeThumbnails: removed $removedCount/${paths.size} thumbnails")
            removedCount > 0
        } catch (e: Exception) {
            Log.e(TAG, "removeThumbnails error", e)
            false
        }
    }

    /**
     * 清理不在给定路径列表中的缩略图缓存
     * @param existingPathsJson JSON 数组，包含所有存在的图片路径
     * @return 清理的缓存文件数量
     */
    @android.webkit.JavascriptInterface
    fun cleanupThumbnailsNotInList(existingPathsJson: String): Int {
        Log.d(TAG, "cleanupThumbnailsNotInList: starting cleanup")

        return try {
            val existingPaths = JSONArray(existingPathsJson).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            val cache = ThumbnailCacheProvider.instance

            // Legacy key set (md5(path)) for backward compatibility.
            val legacyKeys = existingPaths.map { path ->
                path.toByteArray().md5()
            }.toSet()

            // Current key set (keyFor(uri, dateModified)).
            val activeKeys = existingPaths.mapNotNull { path ->
                val uri = Uri.parse(path)
                val dateModified = queryDateModifiedFromMediaStore(uri) ?: return@mapNotNull null
                cache.keyFor(uri, dateModified)
            }.toSet()

            val cacheDir = getThumbnailCacheDir()
            val cacheFiles = cacheDir.listFiles() ?: return 0

            var removedCount = 0
            cacheFiles.forEach { cacheFile ->
                if (shouldRemoveCachedThumbnail(cacheFile.name, legacyKeys, activeKeys)) {
                    if (cacheFile.delete()) {
                        removedCount++
                        Log.d(TAG, "Removed orphaned thumbnail: ${cacheFile.name}")
                    }
                }
            }

            Log.d(TAG, "cleanupThumbnailsNotInList: removed $removedCount orphaned thumbnails")
            removedCount
        } catch (e: Exception) {
            Log.e(TAG, "cleanupThumbnailsNotInList error", e)
            0
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

    /**
     * List images from MediaStore in DCIM/CameraFTP/ directory
     * Returns JSON array of image metadata
     */
    @android.webkit.JavascriptInterface
    fun listMediaStoreImages(): String {
        Log.d(TAG, "listMediaStoreImages: querying MediaStore")
        
        return try {
            val projection = arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DISPLAY_NAME,
                MediaStore.Images.Media.DATE_MODIFIED,
                MediaStore.Images.Media.DATE_ADDED,
                MediaStore.Images.Media.DATE_TAKEN,
                MediaStore.Images.Media.SIZE,
                MediaStore.Images.Media.RELATIVE_PATH
            )

            val selection = build_query_selection()
            
            // Query with DATE_TAKEN DESC to match system gallery order (actual capture time)
            val cursor = context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                projection,
                selection,
                null,
                "${MediaStore.Images.Media.DATE_TAKEN} DESC"
            )

            // Use LinkedHashMap to preserve insertion order from MediaStore query
            val uriMap = LinkedHashMap<String, MediaEntry>()

            cursor?.use {
                val idColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val nameColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME)
                val modifiedColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)
                val addedColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_ADDED)
                val takenColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_TAKEN)
                val sizeColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.SIZE)

                while (it.moveToNext()) {
                    val id = it.getLong(idColumn)
                    val displayName = it.getString(nameColumn)
                    val dateModified = it.getLong(modifiedColumn)
                    val dateAdded = it.getLong(addedColumn)
                    val dateTaken = it.getLong(takenColumn)
                    val size = it.getLong(sizeColumn)

                    val contentUri = ContentUris.withAppendedId(
                        MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                        id
                    ).toString()

                    val entry = MediaEntry(contentUri, dateModified, dateAdded, dateTaken, id, size)
                    
                    // Deduplicate by display name, keeping the newest
                    val existing = uriMap[displayName]
                    uriMap[displayName] = if (existing != null) {
                        pick_newest(existing, entry)
                    } else {
                        entry
                    }
                }
            }

            // Sort all entries
            val sortedEntries = sort_entries(uriMap.values.toList())
            
            // Build JSON response
            val jsonArray = JSONArray()
            sortedEntries.forEach { entry ->
                val json = JSONObject().apply {
                    put("uri", entry.uri)
                    put("displayName", uriMap.entries.find { it.value == entry }?.key ?: "")
                    put("dateModified", entry.dateModified)
                    put("size", entry.size)
                }
                jsonArray.put(json)
            }

            Log.d(TAG, "list_media_store_images: found ${sortedEntries.size} images")
            jsonArray.toString()
        } catch (e: Exception) {
            Log.e(TAG, "list_media_store_images error", e)
            "[]"
        }
    }

    /**
     * Open image with external gallery app, supporting swipe browsing
     * Uses MediaStore URIs for best compatibility
     */
    @android.webkit.JavascriptInterface
    fun open_external_gallery(target_uri: String, all_uris_json: String): Boolean {
        Log.d(TAG, "open_external_gallery: target_uri=$target_uri")
        
        return try {
            val allUris = JSONArray(all_uris_json).let { json ->
                (0 until json.length()).map { json.getString(it) }
            }

            val targetIndex = allUris.indexOf(target_uri)
            if (targetIndex == -1) {
                Log.e(TAG, "open_external_gallery: target URI not found in list")
                return false
            }

            // Build URI window for swipe browsing
            val windowUris = build_uri_window(allUris, targetIndex)
            
            val targetUriParsed = Uri.parse(target_uri)
            
            // Build ClipData with all URIs in the window
            val clipData = ClipData.newRawUri(null, targetUriParsed)
            windowUris.forEach { uri ->
                if (uri != target_uri) {
                    clipData.addItem(ClipData.Item(Uri.parse(uri)))
                }
            }

            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(targetUriParsed, "image/*")
                setClipData(clipData)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }

            // Check if there's a handler for this intent
            val resolveInfo = context.packageManager.resolveActivity(intent, 0)
            val hasHandler = resolveInfo != null

            if (should_show_no_handler_toast(hasHandler)) {
                runOnUiThread {
                    Toast.makeText(context, "No gallery app found", Toast.LENGTH_SHORT).show()
                }
                return false
            }

            if (should_grant_read_permission()) {
                // Grant read permission to all URIs in the clip
                windowUris.forEach { uri ->
                    context.grantUriPermission(
                        context.packageName,
                        Uri.parse(uri),
                        Intent.FLAG_GRANT_READ_URI_PERMISSION
                    )
                }
            }

            context.startActivity(intent)
            Log.d(TAG, "open_external_gallery: opened with ${windowUris.size} URIs")
            true
        } catch (e: Exception) {
            Log.e(TAG, "open_external_gallery error", e)
            false
        }
    }

    /**
     * Load thumbnail from MediaStore using ContentResolver
     */
    private fun load_thumbnail(uri: Uri): Bitmap? {
        return try {
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
                val source = ImageDecoder.createSource(context.contentResolver, uri)
                val bitmap = ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                    val sourceWidth = info.size.width
                    val sourceHeight = info.size.height
                    if (sourceWidth > 0 && sourceHeight > 0) {
                        val shortestEdge = minOf(sourceWidth, sourceHeight)
                        val scale = THUMBNAIL_WIDTH.toFloat() / shortestEdge.toFloat()
                        val targetWidth = maxOf(1, (sourceWidth * scale).toInt())
                        val targetHeight = maxOf(1, (sourceHeight * scale).toInt())
                        decoder.setTargetSize(targetWidth, targetHeight)
                    }
                    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                    decoder.isMutableRequired = false
                }
                centerCropThumbnail(bitmap)
            } else {
                // Fallback for older versions
                @Suppress("DEPRECATION")
                val bitmap = MediaStore.Images.Thumbnails.getThumbnail(
                    context.contentResolver,
                    ContentUris.parseId(uri),
                    MediaStore.Images.Thumbnails.MINI_KIND,
                    null
                )
                bitmap?.let(::centerCropThumbnail)
            }
        } catch (e: Exception) {
            Log.e(TAG, "load_thumbnail error for uri=$uri", e)
            null
        }
    }

    private fun centerCropThumbnail(bitmap: Bitmap): Bitmap {
        val cropEdge = minOf(bitmap.width, bitmap.height)
        val offsetX = (bitmap.width - cropEdge) / 2
        val offsetY = (bitmap.height - cropEdge) / 2
        val croppedBitmap = Bitmap.createBitmap(bitmap, offsetX, offsetY, cropEdge, cropEdge)
        return if (croppedBitmap.width == THUMBNAIL_WIDTH && croppedBitmap.height == THUMBNAIL_HEIGHT) {
            croppedBitmap
        } else {
            Bitmap.createScaledBitmap(croppedBitmap, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, true)
        }
    }
}
