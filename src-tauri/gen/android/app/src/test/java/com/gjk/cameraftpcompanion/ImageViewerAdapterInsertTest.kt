/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ImageViewerAdapterInsertTest {

    @Test
    fun insertUri_atBeginning_incrementsItemCount() {
        val adapter = ImageViewerAdapter(listOf("uri_a", "uri_b"))
        assertEquals(2, adapter.itemCount)

        adapter.insertUri(0, "uri_new")

        assertEquals(3, adapter.itemCount)
        assertEquals("uri_new", adapter.getUriAt(0))
        assertEquals("uri_a", adapter.getUriAt(1))
        assertEquals("uri_b", adapter.getUriAt(2))
    }

    @Test
    fun insertUri_inMiddle_insertsAtCorrectPosition() {
        val adapter = ImageViewerAdapter(listOf("uri_a", "uri_b", "uri_c"))

        adapter.insertUri(1, "uri_new")

        assertEquals(4, adapter.itemCount)
        assertEquals("uri_a", adapter.getUriAt(0))
        assertEquals("uri_new", adapter.getUriAt(1))
        assertEquals("uri_b", adapter.getUriAt(2))
        assertEquals("uri_c", adapter.getUriAt(3))
    }

    @Test
    fun insertUri_atEnd_appendsUri() {
        val adapter = ImageViewerAdapter(listOf("uri_a"))

        adapter.insertUri(1, "uri_b")

        assertEquals(2, adapter.itemCount)
        assertEquals("uri_a", adapter.getUriAt(0))
        assertEquals("uri_b", adapter.getUriAt(1))
    }

    @Test
    fun insertUri_clampsIndexToValidRange() {
        val adapter = ImageViewerAdapter(listOf("uri_a"))

        adapter.insertUri(5, "uri_b")

        assertEquals(2, adapter.itemCount)
        assertEquals("uri_a", adapter.getUriAt(0))
        assertEquals("uri_b", adapter.getUriAt(1))
    }

    @Test
    fun insertUri_duplicateIgnored() {
        val adapter = ImageViewerAdapter(listOf("uri_a", "uri_b"))

        adapter.insertUri(0, "uri_a")

        assertEquals(2, adapter.itemCount)
    }
}
