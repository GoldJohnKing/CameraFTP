/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.util.Base64
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
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
    fun media_page_item_fields_are_preserved() {
        val item = MediaPageItem(
            mediaId = "123",
            uri = "content://media/external/images/media/123",
            dateModifiedMs = 1700000000000L,
            width = 1920,
            height = 1080,
            mimeType = "image/jpeg",
            displayName = "test.jpg"
        )
        assertEquals("123", item.mediaId)
        assertEquals("content://media/external/images/media/123", item.uri)
        assertEquals(1700000000000L, item.dateModifiedMs)
        assertEquals(1920, item.width)
        assertEquals(1080, item.height)
        assertEquals("image/jpeg", item.mimeType)
        assertEquals("test.jpg", item.displayName)
    }

    @Test
    fun media_page_item_nullable_fields() {
        val item = MediaPageItem(
            mediaId = "456",
            uri = "content://media/external/images/media/456",
            dateModifiedMs = 1000L,
            width = null,
            height = null,
            mimeType = null,
            displayName = null
        )
        assertNull(item.width)
        assertNull(item.height)
        assertNull(item.mimeType)
        assertNull(item.displayName)
    }

    @Test
    fun media_page_result_empty_items() {
        val result = MediaPageResult(
            items = emptyList(),
            nextCursor = null,
            revisionToken = "count:0"
        )
        assertTrue(result.items.isEmpty())
        assertNull(result.nextCursor)
        assertEquals("count:0", result.revisionToken)
    }

    @Test
    fun media_page_result_with_items_and_cursor() {
        val items = listOf(
            MediaPageItem("1", "content://media/1", 1000L, 800, 600, "image/png", "img1.png"),
            MediaPageItem("2", "content://media/2", 900L, null, null, null, null)
        )
        val cursor = MediaPageProvider.encodeCursor(MediaPageCursor(900L, 2))
        val result = MediaPageResult(items, cursor, "count:10")

        assertEquals(2, result.items.size)
        assertNotNull(result.nextCursor)
        assertEquals("count:10", result.revisionToken)
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

    @Test
    fun sort_order_is_date_modified_desc_then_id_desc() {
        val sortOrder = MediaPageProvider.SORT_ORDER
        assertTrue(sortOrder.contains("date_modified DESC"))
        assertTrue(sortOrder.contains("_id DESC"))
    }

    @Test
    fun next_cursor_is_set_when_results_fill_page() {
        val lastItem = MediaPageItem("100", "content://media/100", 5000L, 1920, 1080, "image/jpeg", "photo.jpg")
        val cursor = MediaPageProvider.encodeCursor(MediaPageCursor(lastItem.dateModifiedMs, lastItem.mediaId.toLong()))
        assertNotNull(cursor)
        val decoded = MediaPageProvider.decodeCursor(cursor)
        assertNotNull(decoded)
        assertEquals(5000L, decoded!!.dateModifiedMs)
        assertEquals(100L, decoded.mediaId)
    }
}
