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
import org.json.JSONObject
import android.provider.MediaStore
import android.content.Context

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class MediaStoreBridgeTest {

    @Test
    fun parse_entry_result_reads_fd_and_uri() {
        val result = MediaStoreBridge.parseEntryResult("{\"fd\":123,\"uri\":\"content://media/1\"}")
        assertNotNull(result)
        assertEquals(123, result!!.fd)
        assertEquals("content://media/1", result.uri)
    }

    @Test
    fun parse_entry_result_returns_null_for_missing_fields() {
        val result = MediaStoreBridge.parseEntryResult("{\"fd\":123}")
        assertNull(result)
    }

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
    fun resolve_existing_uri_returns_first_when_present() {
        val result = MediaStoreBridge.resolveExistingUri(listOf("content://media/1", "content://media/2"))
        assertEquals("content://media/1", result)
    }

    @Test
    fun resolve_existing_uri_returns_null_for_empty_list() {
        val result = MediaStoreBridge.resolveExistingUri(emptyList())
        assertNull(result)
    }

    @Test
    fun fatal_write_error_detects_enospc_and_io() {
        assertTrue(MediaStoreBridge.isFatalWriteError("ENOSPC"))
        assertTrue(MediaStoreBridge.isFatalWriteError("EIO"))
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
    fun ready_payload_contains_required_fields() {
        val payload = MediaStoreBridge.buildReadyPayload("content://media/1", "DCIM/CameraFTP/", "IMG_1.JPG", 123, 1000)
        val json = JSONObject(payload)
        assertTrue(json.has("uri"))
        assertTrue(json.has("relativePath"))
        assertTrue(json.has("displayName"))
        assertTrue(json.has("size"))
        assertTrue(json.has("timestamp"))
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
    fun should_emit_media_store_ready_for_common_image_mime_types() {
        assertTrue(MediaStoreBridge.shouldEmitMediaStoreReady("image/jpeg"))
        assertTrue(MediaStoreBridge.shouldEmitMediaStoreReady("image/png"))
        assertTrue(MediaStoreBridge.shouldEmitMediaStoreReady("image/webp"))
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
    fun finalize_native_bridge_exposes_emit_ready_entrypoint() {
        val methodRef: (Context, String, Long?) -> Boolean = MediaStoreBridge.Companion::finalizeEntryAndEmitReadyNative
        assertNotNull(methodRef)
    }
}
