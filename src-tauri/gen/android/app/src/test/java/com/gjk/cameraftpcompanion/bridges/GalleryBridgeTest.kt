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
    fun share_intent_uses_media_store_uris() {
        val intent = GalleryBridge.build_share_intent(listOf("content://media/1", "content://media/2"))
        assertEquals(Intent.ACTION_SEND_MULTIPLE, intent.action)
    }

    @Test
    fun removed_v1_thumbnail_methods_are_not_exposed() {
        val methodNames = GalleryBridge::class.java.methods.map { it.name }.toSet()

        assertFalse(methodNames.contains("listMediaStoreImages"))
        assertFalse(methodNames.contains("removeThumbnails"))
        assertFalse(methodNames.contains("cleanupThumbnailsNotInList"))
        assertFalse(methodNames.contains("getThumbnail"))
        assertFalse(methodNames.contains("pick_newest"))
        assertFalse(methodNames.contains("build_query_selection"))
        assertFalse(methodNames.contains("sort_entries"))
    }
}
