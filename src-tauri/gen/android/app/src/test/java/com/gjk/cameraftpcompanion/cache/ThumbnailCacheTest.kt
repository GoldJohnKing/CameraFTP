/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.cache

import android.net.Uri
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ThumbnailCacheTest {

    @Test
    fun thumbnail_cache_key_changes_with_date_modified() {
        val uri = Uri.parse("content://media/1")
        val cache = ThumbnailCache(100)
        val key1 = cache.keyFor(uri, 1000)
        val key2 = cache.keyFor(uri, 2000)
        assertNotEquals(key1, key2)
    }

    @Test
    fun eviction_removes_oldest_when_cap_exceeded() {
        val cache = ThumbnailCache(maxBytes = 100) // small cap for test speed
        val uriA = Uri.parse("content://media/a")
        val uriB = Uri.parse("content://media/b")
        cache.put(uriA, 1000, 60)
        cache.put(uriB, 2000, 60)
        assertFalse(cache.contains(uriA, 1000))
    }

    @Test
    fun evict_if_present_removes_matching_uri() {
        val cache = ThumbnailCache(maxBytes = 100)
        val uriA = Uri.parse("content://media/a")
        cache.put(uriA, 1000, 10)
        cache.evictIfPresent(uriA)
        assertFalse(cache.contains(uriA, 1000))
    }
}
