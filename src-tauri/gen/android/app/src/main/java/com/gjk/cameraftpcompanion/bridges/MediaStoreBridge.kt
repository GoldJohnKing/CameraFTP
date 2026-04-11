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
import org.json.JSONArray
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
                "dng" -> "image/x-adobe-dng"
                "nef" -> "image/x-nikon-nef"
                "nrw" -> "image/x-nikon-nrw"
                "cr2" -> "image/x-canon-cr2"
                "cr3" -> "image/x-canon-cr3"
                "arw" -> "image/x-sony-arw"
                "sr2" -> "image/x-sony-sr2"
                "raf" -> "image/x-fuji-raf"
                "orf" -> "image/x-olympus-orf"
                "rw2" -> "image/x-panasonic-rw2"
                "pef" -> "image/x-pentax-pef"
                "x3f" -> "image/x-sigma-x3f"
                "mp4" -> "video/mp4"
                "mov" -> "video/quicktime"
                "avi" -> "video/x-msvideo"
                else -> DEFAULT_MIME_TYPE
            }
        }

        /**
         * Query Android's MimeTypeMap for a file extension.
         * Returns null if the extension is not recognized.
         */
        @JvmStatic
        fun mimeTypeFromExtension(extension: String): String? {
            return android.webkit.MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(extension.lowercase())
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
            val uri = collectionUri(collection)
            var effectiveRelativePath = relativePath

            if (collection.lowercase() == COLLECTION_DOWNLOADS) {
                // MediaStore.Files only allows Download/ and Documents/ as primary directories.
                // Remap DCIM/CameraFTP/ → Download/CameraFTP/ for non-media files.
                val normalized = relativePath.trimStart('/').lowercase()
                if (normalized.startsWith("dcim/")) {
                    val remapped = "Download/${relativePath.trimStart('/').substringAfter('/')}"
                    Log.d(TAG, "Remapping Downloads path: '$relativePath' → '$remapped'")
                    effectiveRelativePath = remapped
                }
            }

            // Check if entry already exists
            val existingUri = findEntryUriNative(context, effectiveRelativePath, displayName)
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
                put(MediaStore.MediaColumns.RELATIVE_PATH, effectiveRelativePath)
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
        fun finalizeEntryAndEmitGalleryItemsAddedNative(
            context: Context,
            uri: String,
            expectedSize: Long?
        ): Boolean {
            val finalized = finalizeEntryNative(context, uri, expectedSize)
            if (finalized) {
                emitGalleryItemsAdded(context, uri)
            }
            return finalized
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

        private fun emitGalleryItemsAdded(context: Context, uri: String) {
            try {
                val cursor = context.contentResolver.query(
                    Uri.parse(uri),
                    arrayOf(
                        MediaStore.MediaColumns._ID,
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
                        val displayName = it.getString(1) ?: ""
                        val timestamp = it.getLong(2) * 1000
                        val mimeType = it.getString(3)
                        val width = it.getInt(4).takeIf { value -> value > 0 }
                        val height = it.getInt(5).takeIf { value -> value > 0 }

                        if ((mimeType?.lowercase()?.startsWith("image/") == true) ||
                            (mimeType?.lowercase()?.startsWith("video/") == true)
                        ) {
                            val itemPayload = buildGalleryItemsAddedPayload(
                                uri = uri,
                                mediaId = mediaId,
                                timestamp = timestamp,
                                mimeType = mimeType,
                                displayName = displayName,
                                width = width,
                                height = height,
                                emittedAt = System.currentTimeMillis(),
                            )
                            (context as? MainActivity)?.emitWindowEvent("gallery-items-added", itemPayload)
                        }
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to emit gallery-items-added event", e)
            }
        }

        @JvmStatic
        fun buildGalleryItemsAddedPayload(
            uri: String,
            mediaId: String,
            timestamp: Long,
            mimeType: String?,
            displayName: String,
            width: Int?,
            height: Int?,
            emittedAt: Long,
        ): String {
            return JSONObject().apply {
                put("items", JSONArray().apply {
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
                put("timestamp", emittedAt)
            }.toString()
        }
    }

}
