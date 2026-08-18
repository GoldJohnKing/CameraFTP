/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class ImageViewerDeleteRegressionTest {

    @Test
    fun replace_uris_should_remove_deleted_uri_from_adapter_and_keep_next_item_selected() {
        val activityUris = mutableListOf("content://media/1", "content://media/2", "content://media/3")
        val adapter = ImageViewerAdapter(activityUris)
        var currentIndex = 1

        activityUris.removeAt(currentIndex)
        adapter.replaceUris(activityUris)

        val adapterUris = adapter.readInternalUris()
        assertEquals(2, adapter.itemCount)
        assertFalse(adapterUris.contains("content://media/2"))
        assertEquals(listOf("content://media/1", "content://media/3"), adapterUris)
        assertEquals(1, currentIndex)
        assertEquals("content://media/3", activityUris[currentIndex])
    }

    @Test
    fun replace_uris_should_remove_deleted_uri_from_adapter_and_fallback_to_previous_when_deleting_last() {
        val activityUris = mutableListOf("content://media/1", "content://media/2", "content://media/3")
        val adapter = ImageViewerAdapter(activityUris)
        var currentIndex = 2

        activityUris.removeAt(currentIndex)
        if (currentIndex >= activityUris.size && activityUris.isNotEmpty()) {
            currentIndex = activityUris.lastIndex
        }
        adapter.replaceUris(activityUris)

        val adapterUris = adapter.readInternalUris()
        assertEquals(2, adapter.itemCount)
        assertFalse(adapterUris.contains("content://media/3"))
        assertEquals(listOf("content://media/1", "content://media/2"), adapterUris)
        assertEquals(1, currentIndex)
        assertEquals("content://media/2", activityUris[currentIndex])
    }

    @Suppress("UNCHECKED_CAST")
    private fun ImageViewerAdapter.readInternalUris(): List<String> {
        val field = ImageViewerAdapter::class.java.getDeclaredField("uris")
        field.isAccessible = true
        return (field.get(this) as MutableList<String>).toList()
    }
}
