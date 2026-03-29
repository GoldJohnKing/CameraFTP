/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.ContentUris
import android.content.Context
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import org.json.JSONObject

data class MediaPageCursor(val dateModifiedMs: Long, val mediaId: Long)

data class MediaPageItem(
    val mediaId: String,
    val uri: String,
    val dateModifiedMs: Long,
    val width: Int?,
    val height: Int?,
    val mimeType: String?,
    val displayName: String?
)

data class MediaPageResult(
    val items: List<MediaPageItem>,
    val nextCursor: String?,
    val revisionToken: String,
    val totalCount: Int
)

class MediaPageProvider(private val context: Context) {

    companion object {
        private const val TAG = "MediaPageProvider"

        private val PROJECTION = arrayOf(
            MediaStore.Images.Media._ID,
            MediaStore.Images.Media.DATE_MODIFIED,
            MediaStore.Images.Media.WIDTH,
            MediaStore.Images.Media.HEIGHT,
            MediaStore.Images.Media.MIME_TYPE,
            MediaStore.Images.Media.DISPLAY_NAME
        )

        private const val SELECTION = "${MediaStore.Images.Media.RELATIVE_PATH} LIKE '%DCIM/CameraFTP/%'"
        internal const val SORT_ORDER = "${MediaStore.Images.Media.DATE_MODIFIED} DESC, ${MediaStore.Images.Media._ID} DESC"

        @JvmStatic
        fun encodeCursor(cursor: MediaPageCursor): String {
            val json = JSONObject().apply {
                put("dateModifiedMs", cursor.dateModifiedMs)
                put("mediaId", cursor.mediaId)
            }
            return Base64.encodeToString(json.toString().toByteArray(Charsets.UTF_8), Base64.NO_WRAP)
        }

        @JvmStatic
        fun decodeCursor(cursorStr: String): MediaPageCursor? {
            return try {
                val json = JSONObject(String(Base64.decode(cursorStr, Base64.NO_WRAP), Charsets.UTF_8))
                MediaPageCursor(
                    dateModifiedMs = json.getLong("dateModifiedMs"),
                    mediaId = json.getLong("mediaId")
                )
            } catch (e: Exception) {
                Log.w(TAG, "Failed to decode cursor: $cursorStr", e)
                null
            }
        }
    }

    fun listPage(cursor: String?, pageSize: Int): MediaPageResult {
        require(pageSize > 0) { "pageSize must be positive" }
        Log.d(TAG, "listPage: cursor=$cursor, pageSize=$pageSize")

        val decodedCursor = cursor?.let { decodeCursor(it) }
        val selection: String
        val selectionArgs: Array<String>?

        if (decodedCursor != null) {
            // Keyset pagination: (dateModified, mediaId) < (cursor.dateModified, cursor.mediaId)
            // Equivalent to: dateModified < ? OR (dateModified = ? AND mediaId < ?)
            // MediaStore.DATE_MODIFIED is in seconds, so convert cursor's ms back to seconds
            val dateModifiedSec = decodedCursor.dateModifiedMs / 1000
            selection = "$SELECTION AND (${MediaStore.Images.Media.DATE_MODIFIED} < ? OR (${MediaStore.Images.Media.DATE_MODIFIED} = ? AND ${MediaStore.Images.Media._ID} < ?))"
            selectionArgs = arrayOf(
                dateModifiedSec.toString(),
                dateModifiedSec.toString(),
                decodedCursor.mediaId.toString()
            )
        } else {
            selection = SELECTION
            selectionArgs = null
        }

        val items = mutableListOf<MediaPageItem>()
        var lastDateModified: Long = 0
        var lastMediaId: Long = 0

        try {
            context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                PROJECTION,
                selection,
                selectionArgs,
                SORT_ORDER
            )?.use { mediaCursor ->
                val idColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val dateModifiedColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)
                val widthColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media.WIDTH)
                val heightColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media.HEIGHT)
                val mimeTypeColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media.MIME_TYPE)
                val displayNameColumn = mediaCursor.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME)

                var count = 0
                while (mediaCursor.moveToNext() && count < pageSize) {
                    val id = mediaCursor.getLong(idColumn)
                    val dateModifiedSec = mediaCursor.getLong(dateModifiedColumn)
                    val dateModifiedMs = dateModifiedSec * 1000
                    val width = mediaCursor.getInt(widthColumn).takeIf { it > 0 }
                    val height = mediaCursor.getInt(heightColumn).takeIf { it > 0 }
                    val mimeType = mediaCursor.getString(mimeTypeColumn)
                    val displayName = mediaCursor.getString(displayNameColumn)

                    val contentUri = ContentUris.withAppendedId(
                        MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                        id
                    ).toString()

                    items.add(
                        MediaPageItem(
                            mediaId = id.toString(),
                            uri = contentUri,
                            dateModifiedMs = dateModifiedMs,
                            width = width,
                            height = height,
                            mimeType = mimeType,
                            displayName = displayName
                        )
                    )

                    lastDateModified = dateModifiedMs
                    lastMediaId = id
                    count++
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "listPage query error", e)
        }

        // Determine if there's a next page by checking if we got a full page
        // and there might be more items
        val nextCursor = if (items.size == pageSize) {
            encodeCursor(MediaPageCursor(lastDateModified, lastMediaId))
        } else {
            null
        }

        // revisionToken: use a stable identifier based on the current MediaStore count
        // This changes when items are added/removed, allowing callers to detect staleness
        val revisionToken = computeRevisionToken()

        // Get total count for display
        val totalCount = getTotalCount()

        Log.d(TAG, "listPage: returned ${items.size} items, total=$totalCount, hasNext=${nextCursor != null}")
        return MediaPageResult(items, nextCursor, revisionToken, totalCount)
    }

    private fun getTotalCount(): Int {
        return try {
            context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                arrayOf(MediaStore.Images.Media._ID),
                SELECTION,
                null,
                null
            )?.use { cursor ->
                cursor.count
            } ?: 0
        } catch (e: Exception) {
            Log.w(TAG, "Failed to get total count", e)
            0
        }
    }

    private fun computeRevisionToken(): String {
        return try {
            context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                arrayOf(MediaStore.Images.Media._ID),
                SELECTION,
                null,
                null
            )?.use { cursor ->
                "count:${cursor.count}"
            } ?: "count:unknown"
        } catch (e: Exception) {
            Log.w(TAG, "Failed to compute revisionToken", e)
            "count:error"
        }
    }
}
