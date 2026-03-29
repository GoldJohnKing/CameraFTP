/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.MediaStore
import android.util.Log
import com.gjk.cameraftpcompanion.MainActivity
import org.json.JSONObject
import java.io.FileNotFoundException

class MediaStoreBridge(activity: MainActivity) : BaseJsBridge(activity) {

    companion object {
        private const val TAG = "MediaStoreBridge"
        private const val DEFAULT_MIME_TYPE = "application/octet-stream"
        private const val INITIAL_DELAY_MS = 100L
        private const val MAX_DELAY_MS = 400L

        private const val COLLECTION_IMAGES = "images"
        private const val COLLECTION_VIDEOS = "videos"
        private const val COLLECTION_DOWNLOADS = "downloads"

        /**
         * Determine MIME type from FTP type hint or file extension
         */
        @JvmStatic
        fun determineMime(filename: String, ftpType: String?): String {
            // FTP type takes precedence
            if (!ftpType.isNullOrBlank()) {
                return ftpType
            }

            // Fall back to extension-based detection
            return when (filename.substringAfterLast('.', "").lowercase()) {
                "jpg", "jpeg" -> "image/jpeg"
                "png" -> "image/png"
                "gif" -> "image/gif"
                "bmp" -> "image/bmp"
                "webp" -> "image/webp"
                "mp4" -> "video/mp4"
                "mov" -> "video/quicktime"
                "avi" -> "video/x-msvideo"
                else -> DEFAULT_MIME_TYPE
            }
        }

        /**
         * Retry with exponential backoff
         */
        @JvmStatic
        fun <T> retryWithBackoff(
            attempts: Int,
            sleep: (Long) -> Unit = { Thread.sleep(it) },
            block: () -> T
        ): Result<T> {
            var lastException: Exception? = null
            var delay = INITIAL_DELAY_MS

            repeat(attempts) { attempt ->
                try {
                    return Result.success(block())
                } catch (e: Exception) {
                    lastException = e
                    if (attempt < attempts - 1) {
                        sleep(delay)
                        delay = minOf(delay * 2, MAX_DELAY_MS)
                    }
                }
            }

            return Result.failure(lastException ?: RuntimeException("Unknown error"))
        }

        /**
         * Parse entry result JSON into EntryResult
         * Returns null if required fields are missing or malformed
         */
        @JvmStatic
        fun parseEntryResult(json: String): EntryResult? {
            return try {
                val obj = JSONObject(json)
                if (!obj.has("fd") || !obj.has("uri")) {
                    Log.w(TAG, "parseEntryResult: missing required fields in JSON: $json")
                    return null
                }
                EntryResult(
                    fd = obj.getInt("fd"),
                    uri = obj.getString("uri")
                )
            } catch (e: Exception) {
                Log.e(TAG, "parseEntryResult: failed to parse JSON: $json", e)
                null
            }
        }

        /**
         * Resolve existing URI from candidate list
         */
        @JvmStatic
        fun resolveExistingUri(candidates: List<String>): String? {
            return candidates.firstOrNull()
        }

        /**
         * Check if error code is fatal
         */
        @JvmStatic
        fun isFatalWriteError(code: String): Boolean {
            return code == "ENOSPC" || code == "EIO"
        }

        /**
         * Build ready event payload JSON
         */
        @JvmStatic
        fun buildReadyPayload(
            uri: String,
            relativePath: String,
            displayName: String,
            size: Long,
            timestamp: Long
        ): String {
            return JSONObject().apply {
                put("uri", uri)
                put("relativePath", relativePath)
                put("displayName", displayName)
                put("size", size)
                put("timestamp", timestamp)
            }.toString()
        }

        /**
         * Build ContentValues for pending entry
         */
        @JvmStatic
        fun buildPendingValues(displayName: String, sizeHint: Long?): ContentValues {
            return ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, displayName)
                put(MediaStore.MediaColumns.IS_PENDING, 1)
                sizeHint?.let { put(MediaStore.MediaColumns.SIZE, it) }
            }
        }

        /**
         * Build ContentValues for finalize
         */
        @JvmStatic
        fun buildFinalizeValues(expectedSize: Long?): ContentValues {
            return ContentValues().apply {
                put(MediaStore.MediaColumns.IS_PENDING, 0)
                expectedSize?.let { put(MediaStore.MediaColumns.SIZE, it) }
            }
        }

        /**
         * Validate size match
         */
        @JvmStatic
        fun validateSize(expected: Long, actual: Long): Boolean {
            return actual == 0L || expected == actual
        }

        /**
         * Determine if should abort on size mismatch
         */
        @JvmStatic
        fun shouldAbortOnSizeMismatch(expected: Long, actual: Long): Boolean {
            return !validateSize(expected, actual)
        }

        /**
         * Determine if should emit after validation
         */
        @JvmStatic
        fun shouldEmitAfterValidation(expected: Long, actual: Long): Boolean {
            return validateSize(expected, actual)
        }

        /**
         * Build cleanup selection for stale pending entries
         */
        @JvmStatic
        fun buildCleanupSelection(cutoffMillis: Long): String {
            return "${MediaStore.MediaColumns.IS_PENDING} = 1 AND ${MediaStore.MediaColumns.DATE_ADDED} < ${cutoffMillis / 1000}"
        }

        /**
         * Cleanup stale pending entries
         */
        @JvmStatic
        fun cleanupStalePendingEntries(contentResolver: ContentResolver, cutoffMillis: Long) {
            try {
                val selection = buildCleanupSelection(cutoffMillis)
                val totalDeleted = listOf(
                    MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                    MediaStore.Video.Media.EXTERNAL_CONTENT_URI,
                    filesCollectionUri()
                ).sumOf { uri -> contentResolver.delete(uri, selection, null) }
                Log.d(TAG, "Cleaned up $totalDeleted stale pending entries")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to cleanup stale pending entries", e)
            }
        }

        @JvmStatic
        fun collectionUri(collection: String): Uri {
            return when (collection.lowercase()) {
                COLLECTION_IMAGES -> MediaStore.Images.Media.EXTERNAL_CONTENT_URI
                COLLECTION_VIDEOS -> MediaStore.Video.Media.EXTERNAL_CONTENT_URI
                COLLECTION_DOWNLOADS -> filesCollectionUri()
                else -> filesCollectionUri()
            }
        }

        private fun filesCollectionUri(): Uri {
            return MediaStore.Files.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
        }

        private fun isFilesPrimaryDirAllowed(relativePath: String): Boolean {
            val normalized = relativePath.trimStart('/').lowercase()
            return normalized.startsWith("download/") || normalized.startsWith("documents/")
        }

        private fun listCollections(): List<Uri> {
            return listOf(
                filesCollectionUri()
            )
        }

        @JvmStatic
        fun normalizeDirectoryPrefix(relativePath: String): String {
            val normalized = relativePath.trimStart('/').trim()
            if (normalized.isEmpty()) {
                return normalized
            }

            return if (normalized.endsWith('/')) normalized else "$normalized/"
        }

        @JvmStatic
        fun buildListSelection(relativePathColumn: String): String {
            return "$relativePathColumn = ? OR $relativePathColumn LIKE ?"
        }

        @JvmStatic
        fun shouldEmitMediaStoreReady(mimeType: String?): Boolean {
            val value = mimeType?.lowercase() ?: return false
            return value.startsWith("image/") || value.startsWith("video/")
        }

        /**
         * Create MediaStore entry (native implementation)
         */
        @JvmStatic
        fun createEntryNative(
            context: Context,
            displayName: String,
            mime: String,
            relativePath: String,
            collection: String,
            sizeHint: Long?
        ): String {
            val resolver = context.contentResolver
            var uri = collectionUri(collection)

            if (collection.lowercase() == COLLECTION_DOWNLOADS && !isFilesPrimaryDirAllowed(relativePath)) {
                Log.w(
                    TAG,
                    "Files collection rejects primary dir for '$relativePath'; fallback to Images collection for compatibility"
                )
                uri = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
            }

            // Check if entry already exists
            val existingUri = findEntryUriNative(context, relativePath, displayName)
            if (existingUri != null) {
                // Reuse existing entry by setting IS_PENDING=1
                val values = ContentValues().apply {
                    put(MediaStore.MediaColumns.IS_PENDING, 1)
                    sizeHint?.let { put(MediaStore.MediaColumns.SIZE, it) }
                }
                resolver.update(Uri.parse(existingUri), values, null, null)
                return createEntryResult(existingUri, context)
            }

            // Create new entry
            val values = ContentValues().apply {
                put(MediaStore.MediaColumns.DISPLAY_NAME, displayName)
                put(MediaStore.MediaColumns.MIME_TYPE, mime)
                put(MediaStore.MediaColumns.RELATIVE_PATH, relativePath)
                put(MediaStore.MediaColumns.IS_PENDING, 1)
                sizeHint?.let { put(MediaStore.MediaColumns.SIZE, it) }
            }

            val newUri = resolver.insert(uri, values)
                ?: throw RuntimeException("Failed to create MediaStore entry")

            return createEntryResult(newUri.toString(), context)
        }

        private fun createEntryResult(uri: String, context: Context): String {
            val pfd = context.contentResolver.openFileDescriptor(Uri.parse(uri), "w")
                ?: throw FileNotFoundException("Failed to open file descriptor for $uri")

            val fd = pfd.detachFd()
            return JSONObject().apply {
                put("fd", fd)
                put("uri", uri)
            }.toString()
        }

        /**
         * Finalize MediaStore entry (native implementation)
         */
        @JvmStatic
        fun finalizeEntryNative(context: Context, uri: String, expectedSize: Long?): Boolean {
            val resolver = context.contentResolver
            val uriObj = Uri.parse(uri)

            // Validate size if provided
            if (expectedSize != null) {
                val cursor = resolver.query(uriObj, arrayOf(MediaStore.MediaColumns.SIZE), null, null, null)
                cursor?.use {
                    if (it.moveToFirst()) {
                        val actualSize = it.getLong(0)
                        if (!validateSize(expectedSize, actualSize)) {
                            Log.e(TAG, "Size mismatch: expected=$expectedSize, actual=$actualSize")
                            return false
                        }
                    }
                }
            }

            // Finalize by setting IS_PENDING=0 and persisting expected size.
            val values = buildFinalizeValues(expectedSize)
            val updated = resolver.update(uriObj, values, null, null)

            return updated > 0
        }

        @JvmStatic
        fun finalizeEntryAndEmitReadyNative(context: Context, uri: String, expectedSize: Long?): Boolean {
            val finalized = finalizeEntryNative(context, uri, expectedSize)
            if (!finalized) {
                return false
            }

            emitMediaStoreReady(context, uri, expectedSize ?: 0)
            return true
        }

        /**
         * Abort MediaStore entry (native implementation)
         */
        @JvmStatic
        fun abortEntryNative(context: Context, uri: String): Boolean {
            return try {
                val resolver = context.contentResolver
                val uriObj = Uri.parse(uri)
                val deleted = resolver.delete(uriObj, null, null)
                deleted > 0
            } catch (e: Exception) {
                Log.e(TAG, "Failed to abort entry: $uri", e)
                false
            }
        }

        /**
         * List entries in relative path (native implementation)
         */
        @JvmStatic
        fun listEntriesNative(context: Context, relativePath: String): String {
            val resolver = context.contentResolver
            val directoryPrefix = normalizeDirectoryPrefix(relativePath)
            val projection = arrayOf(
                MediaStore.MediaColumns._ID,
                MediaStore.MediaColumns.DISPLAY_NAME,
                MediaStore.MediaColumns.SIZE,
                MediaStore.MediaColumns.DATE_MODIFIED,
                MediaStore.MediaColumns.MIME_TYPE,
                MediaStore.MediaColumns.RELATIVE_PATH
            )
            val selection = if (directoryPrefix.isEmpty()) {
                "${MediaStore.MediaColumns.IS_PENDING} = 0"
            } else {
                "(${buildListSelection(MediaStore.MediaColumns.RELATIVE_PATH)}) AND ${MediaStore.MediaColumns.IS_PENDING} = 0"
            }
            val selectionArgs = if (directoryPrefix.isEmpty()) {
                null
            } else {
                arrayOf(directoryPrefix, "$directoryPrefix%")
            }

            val results = mutableListOf<JSONObject>()

            listCollections().forEach { collectionUri ->
                val cursor = resolver.query(collectionUri, projection, selection, selectionArgs, null)
                cursor?.use {
                    val idColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns._ID)
                    val displayNameColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns.DISPLAY_NAME)
                    val sizeColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns.SIZE)
                    val dateModifiedColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns.DATE_MODIFIED)
                    val mimeTypeColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns.MIME_TYPE)
                    val relativePathColumn = it.getColumnIndexOrThrow(MediaStore.MediaColumns.RELATIVE_PATH)

                    while (it.moveToNext()) {
                        val id = it.getLong(idColumn)
                        val displayName = it.getString(displayNameColumn)
                        val size = it.getLong(sizeColumn)
                        val dateModified = it.getLong(dateModifiedColumn) * 1000L
                        val mimeType = it.getString(mimeTypeColumn) ?: DEFAULT_MIME_TYPE
                        val entryRelativePath = it.getString(relativePathColumn) ?: relativePath

                        results.add(JSONObject().apply {
                            put("uri", ContentUris.withAppendedId(collectionUri, id).toString())
                            put("displayName", displayName)
                            put("size", size)
                            put("dateModified", dateModified)
                            put("mimeType", mimeType)
                            put("relativePath", entryRelativePath)
                        })
                    }
                }
            }

            return JSONObject().apply {
                put("entries", org.json.JSONArray(results))
            }.toString()
        }

        /**
         * Find entry URI by path and name (native implementation)
         */
        @JvmStatic
        fun findEntryUriNative(context: Context, relativePath: String, displayName: String): String? {
            val resolver = context.contentResolver
            val projection = arrayOf(MediaStore.MediaColumns._ID)
            val selection = "${MediaStore.MediaColumns.RELATIVE_PATH} = ? AND ${MediaStore.MediaColumns.DISPLAY_NAME} = ?"
            val selectionArgs = arrayOf(relativePath, displayName)

            listCollections().forEach { collectionUri ->
                val cursor = resolver.query(collectionUri, projection, selection, selectionArgs, null)
                cursor?.use {
                    if (it.moveToFirst()) {
                        val id = it.getLong(it.getColumnIndexOrThrow(MediaStore.MediaColumns._ID))
                        return ContentUris.withAppendedId(collectionUri, id).toString()
                    }
                }
            }

            return null
        }

        /**
         * Open entry for reading (native implementation)
         */
        @JvmStatic
        fun openEntryForReadNative(context: Context, uri: String): Int {
            val pfd = context.contentResolver.openFileDescriptor(Uri.parse(uri), "r")
                ?: throw FileNotFoundException("Failed to open file descriptor for $uri")

            return pfd.detachFd()
        }

        /**
         * Delete entry (native implementation)
         */
        @JvmStatic
        fun deleteEntryNative(context: Context, uri: String): Boolean {
            return try {
                val resolver = context.contentResolver
                val deleted = resolver.delete(Uri.parse(uri), null, null)
                deleted > 0
            } catch (e: Exception) {
                Log.e(TAG, "Failed to delete entry: $uri", e)
                false
            }
        }

        private fun emitMediaStoreReady(context: Context, uri: String, size: Long) {
            try {
                val cursor = context.contentResolver.query(
                    Uri.parse(uri),
                    arrayOf(
                        MediaStore.MediaColumns._ID,
                        MediaStore.MediaColumns.RELATIVE_PATH,
                        MediaStore.MediaColumns.DISPLAY_NAME,
                        MediaStore.MediaColumns.DATE_MODIFIED,
                        MediaStore.MediaColumns.MIME_TYPE,
                        MediaStore.MediaColumns.WIDTH,
                        MediaStore.MediaColumns.HEIGHT,
                    ),
                    null,
                    null,
                    null,
                )

                cursor?.use {
                    if (it.moveToFirst()) {
                        val mediaId = it.getLong(0).toString()
                        val relativePath = it.getString(1) ?: ""
                        val displayName = it.getString(2) ?: ""
                        val timestamp = it.getLong(3) * 1000
                        val mimeType = it.getString(4)
                        val width = it.getInt(5).takeIf { it > 0 }
                        val height = it.getInt(6).takeIf { it > 0 }

                        if (shouldEmitMediaStoreReady(mimeType)) {
                            val payload = buildReadyPayload(uri, relativePath, displayName, size, timestamp)
                            (context as? MainActivity)?.emitTauriEvent("media-store-ready", payload)

                            // Emit incremental add event to WebView (preserves scroll position)
                            val itemPayload = JSONObject().apply {
                                put("items", org.json.JSONArray().apply {
                                    put(JSONObject().apply {
                                        put("mediaId", mediaId)
                                        put("uri", uri)
                                        put("dateModifiedMs", timestamp)
                                        put("width", width ?: JSONObject.NULL)
                                        put("height", height ?: JSONObject.NULL)
                                        put("mimeType", mimeType ?: JSONObject.NULL)
                                        put("displayName", displayName)
                                    })
                                })
                                put("timestamp", System.currentTimeMillis())
                            }.toString()
                            (context as? MainActivity)?.emitWindowEvent("gallery-items-added", itemPayload)
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to emit media-store-ready event", e)
            }
        }
    }

    data class EntryResult(val fd: Int, val uri: String)

    /**
     * Create a MediaStore entry for FTP upload
     */
    @android.webkit.JavascriptInterface
    fun createMediaStoreEntry(
        displayName: String,
        mime: String,
        relativePath: String,
        collection: String,
        sizeHint: Long?
    ): String {
        Log.d(TAG, "createMediaStoreEntry: displayName=$displayName, mime=$mime, relativePath=$relativePath, collection=$collection, sizeHint=$sizeHint")

        return try {
            retryWithBackoff(3) {
                createEntryNative(activity, displayName, mime, relativePath, collection, sizeHint)
            }.getOrElse { e ->
                Log.e(TAG, "Failed to create MediaStore entry after retries", e)
                JSONObject().apply {
                    put("error", e.message)
                    put("fd", -1)
                }.toString()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create MediaStore entry", e)
            JSONObject().apply {
                put("error", e.message)
                put("fd", -1)
            }.toString()
        }
    }

    /**
     * Finalize a MediaStore entry after upload completes
     */
    @android.webkit.JavascriptInterface
    fun finalizeMediaStoreEntry(uri: String, expectedSize: Long?): Boolean {
        Log.d(TAG, "finalizeMediaStoreEntry: uri=$uri, expectedSize=$expectedSize")

        return try {
            val result = retryWithBackoff(3) {
                finalizeEntryAndEmitReadyNative(activity, uri, expectedSize)
            }

            if (result.isSuccess && result.getOrThrow()) {
                true
            } else {
                Log.e(TAG, "Failed to finalize MediaStore entry")
                false
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to finalize MediaStore entry", e)
            false
        }
    }

    /**
     * Abort a MediaStore entry on error
     */
    @android.webkit.JavascriptInterface
    fun abortMediaStoreEntry(uri: String): Boolean {
        Log.d(TAG, "abortMediaStoreEntry: uri=$uri")

        return try {
            abortEntryNative(activity, uri)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to abort MediaStore entry", e)
            false
        }
    }

}
