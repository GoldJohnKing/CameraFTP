/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Intent
import android.net.Uri
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class GalleryActionsBridgeTest {

    @Test
    fun share_intent_uses_media_store_uris() {
        val intent = GalleryActionsBridge.build_share_intent(listOf("content://media/1", "content://media/2"))
        assertEquals(Intent.ACTION_SEND_MULTIPLE, intent.action)
    }

    // ── build_share_intent: ClipData + read-permission grant (Android 10+) ──

    @Test
    fun share_intent_single_uri_sets_stream_clipdata_and_read_grant() {
        val uri = "content://media/external/images/media/1"
        val intent = GalleryActionsBridge.build_share_intent(listOf(uri))

        assertEquals(Intent.ACTION_SEND, intent.action)
        assertEquals("image/*", intent.type)

        @Suppress("DEPRECATION")
        val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
        assertEquals("EXTRA_STREAM must carry the shared URI", Uri.parse(uri), stream)

        val clip = intent.clipData
        assertNotNull("ClipData must be set for URI permission propagation", clip)
        assertEquals(1, clip!!.itemCount)
        assertEquals(Uri.parse(uri), clip.getItemAt(0).uri)

        assertTrue(
            "FLAG_GRANT_READ_URI_PERMISSION must be set on the intent",
            intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0
        )
    }

    @Test
    fun share_intent_multiple_uris_sets_stream_list_clipdata_and_read_grant() {
        val uris = listOf(
            "content://media/external/images/media/1",
            "content://media/external/images/media/2",
            "content://media/external/images/media/3"
        )
        val intent = GalleryActionsBridge.build_share_intent(uris)

        assertEquals(Intent.ACTION_SEND_MULTIPLE, intent.action)
        assertEquals("image/*", intent.type)

        @Suppress("DEPRECATION")
        val streams = intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
        assertNotNull("EXTRA_STREAM must carry the parcelable URI list", streams)
        assertEquals(uris.map { Uri.parse(it) }, streams)

        val clip = intent.clipData
        assertNotNull("ClipData must be set for URI permission propagation", clip)
        assertEquals("ClipData must contain every shared URI", uris.size, clip!!.itemCount)
        assertEquals(
            uris.map { Uri.parse(it) },
            (0 until clip.itemCount).map { clip.getItemAt(it).uri }
        )

        assertTrue(
            "FLAG_GRANT_READ_URI_PERMISSION must be set on the intent",
            intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0
        )
    }

}
