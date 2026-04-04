/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import android.provider.MediaStore
import org.json.JSONObject

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class MediaStoreBridgeTest {

    @Test
    fun retry_with_backoff_uses_correct_delays() {
        val delays = mutableListOf<Long>()
        MediaStoreBridge.retryWithBackoff(3, sleep = { delays.add(it) }) { throw RuntimeException("fail") }
        assertEquals(listOf(100L, 200L), delays)
    }

    @Test
    fun retry_with_backoff_succeeds_on_second_attempt() {
        var attempt = 0
        val result = MediaStoreBridge.retryWithBackoff(3) {
            attempt++
            if (attempt == 1) throw RuntimeException("fail")
            "ok"
        }
        assertTrue(result.isSuccess)
    }

    @Test
    fun mime_detection_ftp_type_takes_precedence() {
        val mime = MediaStoreBridge.determineMime("IMG_1.JPG", "image/png")
        assertEquals("image/png", mime)
    }

    @Test
    fun mime_detection_falls_back_to_extension() {
        val mime = MediaStoreBridge.determineMime("IMG_1.JPG", null)
        assertEquals("image/jpeg", mime)
    }

    @Test
    fun mime_detection_defaults_to_octet_stream() {
        val mime = MediaStoreBridge.determineMime("FILE", null)
        assertEquals("application/octet-stream", mime)
    }

    @Test
    fun pending_values_preserves_display_name() {
        val values = MediaStoreBridge.buildPendingValues("IMG_1.JPG", null)
        assertEquals("IMG_1.JPG", values.getAsString(MediaStore.MediaColumns.DISPLAY_NAME))
    }

    @Test
    fun pending_values_sets_is_pending_and_size() {
        val values = MediaStoreBridge.buildPendingValues("IMG_1.JPG", 123)
        assertEquals(1, values.getAsInteger(MediaStore.MediaColumns.IS_PENDING))
        assertEquals(123L, values.getAsLong(MediaStore.MediaColumns.SIZE))
    }

    @Test
    fun finalize_values_clears_is_pending() {
        val values = MediaStoreBridge.buildFinalizeValues(123)
        assertEquals(0, values.getAsInteger(MediaStore.MediaColumns.IS_PENDING))
        assertEquals(123L, values.getAsLong(MediaStore.MediaColumns.SIZE))
    }

    @Test
    fun validate_size_handles_mismatch() {
        assertTrue(MediaStoreBridge.validateSize(1000, 0))
        assertFalse(MediaStoreBridge.validateSize(1000, 500))
        assertTrue(MediaStoreBridge.validateSize(1000, 1000))
    }

    @Test
    fun cleanup_selection_targets_pending_rows() {
        val selection = MediaStoreBridge.buildCleanupSelection(1234)
        assertTrue(selection.contains(MediaStore.MediaColumns.IS_PENDING))
    }

    @Test
    fun cleanup_removes_pending_older_than_24h() {
        val nowMinus25h = System.currentTimeMillis() - 25 * 60 * 60 * 1000L
        val selection = MediaStoreBridge.buildCleanupSelection(nowMinus25h)
        assertTrue(selection.contains(MediaStore.MediaColumns.IS_PENDING))
        assertTrue(selection.contains(MediaStore.MediaColumns.DATE_ADDED))
    }

    @Test
    fun gallery_items_added_payload_keeps_incremental_item_shape() {
        val payload = MediaStoreBridge.buildGalleryItemsAddedPayload(
            uri = "content://media/1",
            mediaId = "1",
            timestamp = 1700000000000L,
            mimeType = "image/jpeg",
            displayName = "IMG_0001.JPG",
            width = 1920,
            height = 1080,
            emittedAt = 1700000001234L,
        )

        val json = JSONObject(payload)
        val items = json.getJSONArray("items")
        assertEquals(1, items.length())
        val first = items.getJSONObject(0)
        assertEquals("1", first.getString("mediaId"))
        assertEquals("content://media/1", first.getString("uri"))
        assertEquals(1700000000000L, first.getLong("dateModifiedMs"))
        assertEquals("image/jpeg", first.getString("mimeType"))
        assertEquals("IMG_0001.JPG", first.getString("displayName"))
        assertEquals(1920, first.getInt("width"))
        assertEquals(1080, first.getInt("height"))
        assertEquals(1700000001234L, json.getLong("timestamp"))
    }

    @Test
    fun gallery_items_added_payload_serializes_missing_dimensions_as_null() {
        val payload = MediaStoreBridge.buildGalleryItemsAddedPayload(
            uri = "content://media/2",
            mediaId = "2",
            timestamp = 1700000005000L,
            mimeType = null,
            displayName = "IMG_0002.JPG",
            width = null,
            height = null,
            emittedAt = 1700000006000L,
        )

        val first = JSONObject(payload).getJSONArray("items").getJSONObject(0)
        assertTrue(first.isNull("mimeType"))
        assertTrue(first.isNull("width"))
        assertTrue(first.isNull("height"))
    }

    @Test
    fun normalize_directory_prefix_appends_trailing_slash_for_nested_paths() {
        assertEquals("DCIM/CameraFTP/album/", MediaStoreBridge.normalizeDirectoryPrefix("DCIM/CameraFTP/album"))
    }

    @Test
    fun build_list_selection_targets_exact_and_nested_relative_paths() {
        assertEquals(
            "relative_path = ? OR relative_path LIKE ?",
            MediaStoreBridge.buildListSelection("relative_path")
        )
    }

    @Test
    fun finalize_native_bridge_uses_finalize_only_entrypoint() {
        val methods = MediaStoreBridge.Companion::class.java.declaredMethods.map { it.name }.toSet()
        assertTrue(methods.contains("finalizeEntryAndEmitGalleryItemsAddedNative"))
        assertFalse(methods.contains("finalizeEntryAndEmitReadyNative"))
    }
}
