/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ThumbnailKeyV2Test {

    @Test
    fun `same input produces same key`() {
        val a = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 0, 2_048_000)
        val b = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 0, 2_048_000)
        assertEquals(a, b)
    }

    @Test
    fun `different input produces different key`() {
        val base = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 0, 2_048_000)

        val diffId = ThumbnailKeyV2.of("img2", 1_700_000_000_000, "256x256", 0, 2_048_000)
        val diffDate = ThumbnailKeyV2.of("img1", 1_800_000_000_000, "256x256", 0, 2_048_000)
        val diffBucket = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "512x512", 0, 2_048_000)
        val diffOri = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 6, 2_048_000)
        val diffSize = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 0, 999)

        assertNotEquals(base, diffId)
        assertNotEquals(base, diffDate)
        assertNotEquals(base, diffBucket)
        assertNotEquals(base, diffOri)
        assertNotEquals(base, diffSize)
    }

    @Test
    fun `key is valid lowercase hex string of length 40`() {
        val key = ThumbnailKeyV2.of("img1", 1_700_000_000_000, "256x256", 0, 2_048_000)
        assertEquals(40, key.length)
        assertTrue(key.matches(Regex("[0-9a-f]{40}")))
    }
}
