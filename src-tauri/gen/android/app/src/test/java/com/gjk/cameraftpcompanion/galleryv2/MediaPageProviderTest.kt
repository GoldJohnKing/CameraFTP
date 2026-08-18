/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.provider.MediaStore
import android.util.Base64
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowContentResolver

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class MediaPageProviderTest {

    @Test
    fun cursor_encode_decode_roundtrip() {
        val cursor = MediaPageCursor(dateModifiedMs = 1700000000000L, mediaId = 42)
        val encoded = MediaPageProvider.encodeCursor(cursor)
        val decoded = MediaPageProvider.decodeCursor(encoded)
        assertNotNull(decoded)
        assertEquals(cursor.dateModifiedMs, decoded!!.dateModifiedMs)
        assertEquals(cursor.mediaId, decoded.mediaId)
    }

    @Test
    fun cursor_decode_invalid_returns_null() {
        val result = MediaPageProvider.decodeCursor("not-valid-base64!!")
        assertNull(result)
    }

    @Test
    fun cursor_decode_empty_string_returns_null() {
        val result = MediaPageProvider.decodeCursor("")
        assertNull(result)
    }

    @Test
    fun cursor_decode_garbage_json_returns_null() {
        val garbage = Base64.encodeToString("not json".toByteArray(), Base64.NO_WRAP)
        val result = MediaPageProvider.decodeCursor(garbage)
        assertNull(result)
    }

    @Test
    fun cursor_decode_missing_fields_returns_null() {
        val partial = Base64.encodeToString("""{"dateModifiedMs":100}""".toByteArray(), Base64.NO_WRAP)
        val result = MediaPageProvider.decodeCursor(partial)
        assertNull(result)
    }

    @Test
    fun cursor_encode_produces_non_empty_base64() {
        val cursor = MediaPageCursor(dateModifiedMs = 1000L, mediaId = 1)
        val encoded = MediaPageProvider.encodeCursor(cursor)
        assertTrue(encoded.isNotEmpty())
        // Base64 should only contain valid characters
        assertTrue(encoded.matches(Regex("[A-Za-z0-9+/=]+")))
    }

    @Test
    fun sort_order_is_date_modified_desc_then_id_desc() {
        val sortOrder = MediaPageProvider.SORT_ORDER
        assertTrue(sortOrder.contains("date_modified DESC"))
        assertTrue(sortOrder.contains("_id DESC"))
    }

    @Test
    fun cursor_roundtrip_with_zero_values() {
        val cursor = MediaPageCursor(dateModifiedMs = 0L, mediaId = 0L)
        val encoded = MediaPageProvider.encodeCursor(cursor)
        val decoded = MediaPageProvider.decodeCursor(encoded)
        assertNotNull(decoded)
        assertEquals(0L, decoded!!.dateModifiedMs)
        assertEquals(0L, decoded.mediaId)
    }

    @Test
    fun cursor_roundtrip_with_large_values() {
        val cursor = MediaPageCursor(dateModifiedMs = Long.MAX_VALUE, mediaId = Long.MAX_VALUE)
        val encoded = MediaPageProvider.encodeCursor(cursor)
        val decoded = MediaPageProvider.decodeCursor(encoded)
        assertNotNull(decoded)
        assertEquals(Long.MAX_VALUE, decoded!!.dateModifiedMs)
        assertEquals(Long.MAX_VALUE, decoded.mediaId)
    }

    // ── listPage keyset-pagination selection construction ──────────────

    /**
     * Captures every query issued against the MediaStore images collection
     * and returns a cursor over the caller-provided rows. Registered under
     * the "media" authority so ShadowContentResolver routes
     * [MediaPageProvider] queries here.
     */
    private class QueryCaptureProvider : ContentProvider() {

        data class CapturedQuery(
            val uri: Uri,
            val selection: String?,
            val selectionArgs: Array<out String>?,
            val sortOrder: String?
        )

        val queries = mutableListOf<CapturedQuery>()

        /** Rows in desired (already sorted) order: id, dateModifiedSec, w, h, mime, name, data. */
        var rows: List<Array<Any>> = emptyList()

        override fun onCreate(): Boolean = true

        override fun query(
            uri: Uri,
            projection: Array<out String>?,
            selection: String?,
            selectionArgs: Array<out String>?,
            sortOrder: String?
        ): Cursor? {
            queries += CapturedQuery(uri, selection, selectionArgs, sortOrder)
            return MatrixCursor(arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DATE_MODIFIED,
                MediaStore.Images.Media.WIDTH,
                MediaStore.Images.Media.HEIGHT,
                MediaStore.Images.Media.MIME_TYPE,
                MediaStore.Images.Media.DISPLAY_NAME,
                MediaStore.Images.Media.DATA
            )).apply { rows.forEach { addRow(it) } }
        }

        override fun getType(uri: Uri): String? = "image/jpeg"
        override fun insert(uri: Uri, values: ContentValues?): Uri? = null
        override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
        override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0
    }

    private fun registeredProvider(rows: List<Array<Any>>): QueryCaptureProvider =
        QueryCaptureProvider().apply {
            this.rows = rows
            ShadowContentResolver.registerProviderInternal("media", this)
        }

    /** Three images already in SORT_ORDER order (newest first). */
    private fun sampleRows(): List<Array<Any>> = listOf(
        arrayOf(101L, 500L, 4000, 3000, "image/jpeg", "IMG_1.JPG", "/storage/1/IMG_1.JPG"),
        arrayOf(102L, 400L, 3000, 2000, "image/jpeg", "IMG_2.JPG", "/storage/1/IMG_2.JPG"),
        arrayOf(103L, 300L, 2000, 1000, "image/jpeg", "IMG_3.JPG", "/storage/1/IMG_3.JPG")
    )

    @Test
    fun list_page_without_cursor_uses_base_selection_without_args() {
        val provider = registeredProvider(sampleRows())

        val result = MediaPageProvider(RuntimeEnvironment.getApplication()).listPage(cursor = null, pageSize = 2)

        val pageQuery = provider.queries.single { it.sortOrder == MediaPageProvider.SORT_ORDER }
        assertEquals(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, pageQuery.uri)
        assertEquals(
            "relative_path LIKE '%DCIM/CameraFTP/%'",
            pageQuery.selection
        )
        assertNull("first page must not carry keyset predicate args", pageQuery.selectionArgs)

        // Two newest rows returned; next cursor points at the last emitted row.
        assertEquals(listOf("101", "102"), result.items.map { it.mediaId })
        assertEquals(500_000L, result.items[0].dateModifiedMs)
        assertEquals(400_000L, result.items[1].dateModifiedMs)
        val next = MediaPageProvider.decodeCursor(result.nextCursor!!)
        assertEquals(400_000L, next!!.dateModifiedMs)
        assertEquals(102L, next.mediaId)

        // The count/revision query backs totalCount + revisionToken.
        assertEquals(3, result.totalCount)
        assertEquals("count:3", result.revisionToken)
    }

    @Test
    fun list_page_with_cursor_builds_keyset_predicate_with_second_precision_args() {
        val provider = registeredProvider(sampleRows())
        val cursor = MediaPageProvider.encodeCursor(MediaPageCursor(dateModifiedMs = 450_000L, mediaId = 250L))

        MediaPageProvider(RuntimeEnvironment.getApplication()).listPage(cursor = cursor, pageSize = 2)

        val pageQuery = provider.queries.single { it.sortOrder == MediaPageProvider.SORT_ORDER }
        assertEquals(
            "relative_path LIKE '%DCIM/CameraFTP/%' AND " +
                "(date_modified < ? OR (date_modified = ? AND _id < ?))",
            pageQuery.selection
        )
        // Cursor ms are converted back to MediaStore seconds for both args,
        // and the mediaId arg breaks ties within one date_modified second.
        assertEquals(
            listOf("450", "450", "250"),
            pageQuery.selectionArgs!!.toList()
        )
    }

    @Test
    fun list_page_short_page_has_no_next_cursor() {
        val provider = registeredProvider(sampleRows())

        val result = MediaPageProvider(RuntimeEnvironment.getApplication()).listPage(cursor = null, pageSize = 5)

        assertEquals(3, result.items.size)
        assertNull(
            "a short page (fewer items than pageSize) must not produce a next cursor",
            result.nextCursor
        )
    }

    @Test
    fun list_page_queries_are_scoped_to_the_dcim_cameraftp_directory() {
        val provider = registeredProvider(emptyList())

        MediaPageProvider(RuntimeEnvironment.getApplication()).listPage(cursor = null, pageSize = 10)

        // Both the page query and the count/revision query use the same
        // DCIM/CameraFTP selection — nothing queries unscoped.
        assertTrue(provider.queries.isNotEmpty())
        assertEquals(2, provider.queries.size)
        provider.queries.forEach { q ->
            assertEquals("relative_path LIKE '%DCIM/CameraFTP/%'", q.selection)
        }
    }
}
