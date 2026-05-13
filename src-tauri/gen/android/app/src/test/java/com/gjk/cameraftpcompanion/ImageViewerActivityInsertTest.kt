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
class ImageViewerActivityInsertTest {

    private val baseUris = listOf("uri_a", "uri_b", "uri_c")

    // --- computeInsertState ---

    @Test
    fun insert_atBeginning_adjustsCurrentIndex() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 0, "uri_new", 0)!!

        assertEquals(4, result.uris.size)
        assertEquals("uri_new", result.uris[0])
        assertEquals("uri_a", result.uris[1])
        // currentIndex was 0, insert at 0 → shifts to 1
        assertEquals(1, result.currentIndex)
    }

    @Test
    fun insert_atEnd_doesNotShiftCurrentIndex() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 1, "uri_new", 3)!!

        assertEquals(4, result.uris.size)
        assertEquals("uri_new", result.uris[3])
        // Insert at end (3), currentIndex was 1 → no shift
        assertEquals(1, result.currentIndex)
    }

    @Test
    fun insert_duplicate_returnsNull() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 0, "uri_b", 0)

        assertNull(result)
    }

    @Test
    fun insert_clampsIndexToEnd() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 1, "uri_new", 100)!!

        assertEquals(4, result.uris.size)
        assertEquals("uri_new", result.uris[3])
        assertEquals(1, result.currentIndex)
    }

    @Test
    fun insert_inMiddle_shiftsCurrentIndex() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 2, "uri_new", 1)!!

        assertEquals(listOf("uri_a", "uri_new", "uri_b", "uri_c"), result.uris)
        // currentIndex was 2, insert at 1 (before 2) → shifts to 3
        assertEquals(3, result.currentIndex)
    }

    @Test
    fun insert_atExactCurrentIndex_shiftsCurrentIndex() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 1, "uri_new", 1)!!

        assertEquals(listOf("uri_a", "uri_new", "uri_b", "uri_c"), result.uris)
        // currentIndex was 1, insert at 1 (== currentIndex) → shifts to 2
        assertEquals(2, result.currentIndex)
    }

    @Test
    fun insert_afterCurrentIndex_doesNotShift() {
        val result = ImageViewerActivity.computeInsertState(baseUris, 0, "uri_new", 2)!!

        assertEquals(listOf("uri_a", "uri_b", "uri_new", "uri_c"), result.uris)
        // currentIndex was 0, insert at 2 (after 0) → no shift
        assertEquals(0, result.currentIndex)
    }

    @Test
    fun insert_intoEmptyList_currentIndexAtZero_setsToFirstItem() {
        val result = ImageViewerActivity.computeInsertState(emptyList(), 0, "uri_new", 0)!!

        assertEquals(listOf("uri_new"), result.uris)
        // Empty list: only item, index must be 0 (not out-of-bounds 1)
        assertEquals(0, result.currentIndex)
    }

    // --- computeNavigateToExistingIndex ---

    @Test
    fun navigate_findsCorrectIndex() {
        val newIndex = ImageViewerActivity.computeNavigateToExistingIndex(baseUris, 0, "uri_c")!!

        assertEquals(2, newIndex)
    }

    @Test
    fun navigate_uriNotFound_returnsNull() {
        val newIndex = ImageViewerActivity.computeNavigateToExistingIndex(baseUris, 1, "uri_nonexistent")

        assertNull(newIndex)
    }

    @Test
    fun navigate_sameAsCurrent_returnsNull() {
        val newIndex = ImageViewerActivity.computeNavigateToExistingIndex(baseUris, 1, "uri_b")

        assertNull(newIndex)
    }

    @Test
    fun navigate_toFirstIndex() {
        val newIndex = ImageViewerActivity.computeNavigateToExistingIndex(baseUris, 2, "uri_a")!!

        assertEquals(0, newIndex)
    }
}
