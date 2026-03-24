/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Intent
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class GalleryBridgeTest {

    @Test
    fun pick_freshest_entry_prefers_newest_date_modified() {
        val uri_a = "content://media/1"
        val uri_b = "content://media/2"
        val entry_a = GalleryBridge.MediaEntry(uri_a, 100, 1000, 1, 10, 100)
        val entry_b = GalleryBridge.MediaEntry(uri_b, 200, 2000, 2, 5, 200)
        assertEquals(entry_b, GalleryBridge.pick_newest(entry_a, entry_b))
    }

    @Test
    fun build_uri_window_51_uri_max_target_plus_25_each_side() {
        val uris = (0 until 200).map { "content://media/$it" }
        val result = GalleryBridge.build_uri_window(uris, 150)
        assertEquals(51, result.size)
        assertTrue(result.contains("content://media/150"))
    }

    @Test
    fun build_uri_window_handles_start_and_end_edges() {
        val uris = (0 until 10).map { "content://media/$it" }
        assertEquals(10, GalleryBridge.build_uri_window(uris, 0).size)
        assertEquals(10, GalleryBridge.build_uri_window(uris, 9).size)
    }

    @Test
    fun list_media_store_images_uses_correct_relative_path() {
        val selection = GalleryBridge.build_query_selection()
        assertTrue(selection.contains("DCIM/CameraFTP/"))
    }

    @Test
    fun list_media_store_images_includes_nested_cameraftp_subdirectories() {
        val selection = GalleryBridge.build_query_selection()
        assertTrue(selection.contains("DCIM/CameraFTP/"))
        assertFalse(selection.contains("NOT LIKE"))
    }

    @Test
    fun sort_order_uses_date_modified_desc_then_added_then_size() {
        val uri_a = "content://media/1"
        val uri_b = "content://media/2"
        val uri_c = "content://media/3"
        val items = listOf(
            GalleryBridge.MediaEntry(uri_a, 100, 1000, 1, 10, 100),
            GalleryBridge.MediaEntry(uri_b, 100, 1000, 2, 5, 200),
            GalleryBridge.MediaEntry(uri_c, 100, 2000, 1, 1, 300)
        )
        val sorted = GalleryBridge.sort_entries(items)
        assertEquals(listOf(uri_b, uri_c, uri_a), sorted.map { it.uri })
    }

    @Test
    fun open_external_gallery_no_handler_shows_toast() {
        val should_toast = GalleryBridge.should_show_no_handler_toast(false)
        assertTrue(should_toast)
    }

    @Test
    fun open_external_gallery_grants_read_permission() {
        assertTrue(GalleryBridge.should_grant_read_permission())
    }

    @Test
    fun share_intent_uses_media_store_uris() {
        val intent = GalleryBridge.build_share_intent(listOf("content://media/1", "content://media/2"))
        assertEquals(Intent.ACTION_SEND_MULTIPLE, intent.action)
    }

    @Test
    fun delete_uses_media_store_uri_not_path() {
        val selection = GalleryBridge.build_delete_selection("content://media/1")
        assertEquals("${android.provider.MediaStore.Images.Media._ID}=?", selection)
        assertEquals("", GalleryBridge.build_delete_selection("/storage/emulated/0/DCIM/test.jpg"))
    }

    @Test
    fun thumbnail_cleanup_keeps_legacy_and_current_cache_keys() {
        val legacyKey = "legacymd5"
        val currentKey = "currentmd5"

        assertFalse(
            GalleryBridge.shouldRemoveCachedThumbnail(
                fileName = "thumb_${legacyKey}.jpg",
                legacyKeys = setOf(legacyKey),
                activeKeys = emptySet(),
            )
        )

        assertFalse(
            GalleryBridge.shouldRemoveCachedThumbnail(
                fileName = "thumb_${currentKey}.jpg",
                legacyKeys = emptySet(),
                activeKeys = setOf(currentKey),
            )
        )

        assertTrue(
            GalleryBridge.shouldRemoveCachedThumbnail(
                fileName = "thumb_orphan.jpg",
                legacyKeys = setOf(legacyKey),
                activeKeys = setOf(currentKey),
            )
        )
    }
}
