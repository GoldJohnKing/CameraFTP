/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.io.File

/**
 * L2 disk-cache eviction tests for [ThumbnailCacheV2].
 *
 * Files get explicit, distinct `lastModified` stamps (real filesystem
 * granularity cannot be trusted to order rapidly-created files) so the LRU
 * order under test is exactly the one the test defines.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class ThumbnailCacheV2Test {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = RuntimeEnvironment.getApplication()
    }

    private fun newCache(l2MaxBytes: Long): ThumbnailCacheV2 =
        ThumbnailCacheV2(l1MaxBytes = 4 * 1024 * 1024, l2MaxBytes = l2MaxBytes).apply {
            initialize(context)
        }

    /** Mirrors the private CACHE_ROOT location used by [ThumbnailCacheV2.initialize]. */
    private fun cacheRoot(): File = File(context.cacheDir, "thumb/v2")

    /** Put a [bytes]-sized entry and stamp it with an explicit LRU timestamp. */
    private fun putStamped(cache: ThumbnailCacheV2, index: Int, bytes: ByteArray, lastModifiedMs: Long): File {
        val mediaId = "media-$index"
        val key = ThumbnailKeyV2.of(mediaId, index * 1000L, "s", 0, 0)
        cache.put(mediaId, key, "s", bytes)
        val file = File(File(cacheRoot(), "s"), "${mediaId}_$key.jpg")
        assertTrue("cache file should exist for $mediaId", file.exists())
        assertTrue("test must be able to stamp lastModified", file.setLastModified(lastModifiedMs))
        return file
    }

    private fun remainingJpgBytes(): Long =
        cacheRoot().walkTopDown().filter { it.isFile && it.extension == "jpg" }.sumOf { it.length() }

    // ── Direct cleanup(): LRU order + capacity re-establishment ─────────

    @Test
    fun cleanup_evicts_oldest_first_and_reestablishes_capacity() {
        val cache = newCache(l2MaxBytes = 100L) // room for two 40-byte files
        val base = System.currentTimeMillis() - 1_000_000L
        val files = (1..5).map { i -> putStamped(cache, i, ByteArray(40), base + i * 1_000L) }

        cache.cleanup()

        // 200 bytes on disk → evict the three oldest → 80 bytes remain.
        assertFalse("oldest entry must be evicted", files[0].exists())
        assertFalse(files[1].exists())
        assertFalse(files[2].exists())
        assertTrue("newest entries must survive", files[3].exists())
        assertTrue(files[4].exists())

        assertTrue(
            "cleanup must re-establish the configured capacity",
            remainingJpgBytes() <= 100L
        )
    }

    @Test
    fun cleanup_keeps_newest_entries_when_evicting_for_capacity() {
        // Capacity 50 fits only one 40-byte file → the older one is evicted.
        val cache = newCache(l2MaxBytes = 50L)
        val base = System.currentTimeMillis() - 1_000_000L
        val old = putStamped(cache, 1, ByteArray(40), base)
        val newest = putStamped(cache, 2, ByteArray(40), base + 5_000L)

        cache.cleanup()

        assertFalse("the older file is the LRU victim", old.exists())
        assertTrue("the newest file survives", newest.exists())
    }

    @Test
    fun cleanup_under_capacity_is_a_noop() {
        val cache = newCache(l2MaxBytes = 10_000L)
        val base = System.currentTimeMillis() - 1_000_000L
        val f1 = putStamped(cache, 1, ByteArray(40), base)
        val f2 = putStamped(cache, 2, ByteArray(40), base + 1_000L)

        cache.cleanup()

        assertTrue("nothing may be evicted below capacity", f1.exists())
        assertTrue(f2.exists())
    }

    @Test
    fun cleanup_ignores_non_jpg_files() {
        val cache = newCache(l2MaxBytes = 0L) // everything jpg is over capacity
        val jpg = putStamped(cache, 1, ByteArray(32), System.currentTimeMillis() - 10_000L)
        val bucketDir = File(cacheRoot(), "s").apply { mkdirs() }
        val stray = File(bucketDir, "stray.txt").apply { writeBytes(ByteArray(64)) }

        cache.cleanup()

        assertFalse("jpg entries are evicted when over capacity", jpg.exists())
        assertTrue("non-jpg files must not be touched by L2 cleanup", stray.exists())
    }

    // ── Throttled enforcement (every 32 puts) ──────────────────────────

    @Test
    fun l2_enforcement_runs_on_the_32nd_put_and_allows_bounded_overshoot() {
        val fileBytes = 32
        val capacity = 64L // exactly two files
        val cache = newCache(l2MaxBytes = capacity)
        val base = System.currentTimeMillis() - 10_000_000L

        val files = (1..33).map { i -> putStamped(cache, i, ByteArray(fileBytes), base + i * 1_000L) }

        // Puts 1..31 (31 × 32 = 992 bytes) exceeded capacity but no walk ran.
        // Put #32 hit the L2_ENFORCE_EVERY_PUTS boundary: the walk trimmed
        // 1024 bytes back to ≤ 64, evicting the 30 oldest files and keeping
        // file 31 (newest stamped) and file 32 (natural now() timestamp).
        assertFalse("oldest files must be evicted by the 32nd put", files[0].exists())
        assertFalse(files[10].exists())
        assertFalse(files[29].exists())
        assertTrue("newest stamped pre-walk file must survive", files[30].exists())
        assertTrue("file written by the enforcing put must survive", files[31].exists())

        // Put #33 did not trigger a walk (counter reset) — the documented
        // one-extra-file overshoot slack.
        assertTrue("post-walk put must be kept as documented slack", files[32].exists())
        assertEquals(
            "Overshoot between walks is bounded by L2_ENFORCE_EVERY_PUTS - 1 files",
            3 * fileBytes.toLong(),
            remainingJpgBytes()
        )

        // An explicit cleanup re-establishes the configured capacity,
        // evicting the oldest of the three survivors.
        cache.cleanup()
        assertFalse("explicit cleanup removes the overshoot", files[30].exists())
        assertTrue(files[31].exists())
        assertTrue(files[32].exists())
        assertTrue(remainingJpgBytes() <= capacity)
    }

    @Test
    fun l2_enforcement_below_the_32nd_put_does_not_run() {
        val cache = newCache(l2MaxBytes = 8L) // over capacity after one file
        val base = System.currentTimeMillis() - 1_000_000L
        val files = (1..10).map { i -> putStamped(cache, i, ByteArray(32), base + i * 1_000L) }

        // 10 < 32 puts: even though the cache is far over capacity, the
        // throttled walk must not have evicted anything.
        files.forEach { assertTrue("no eviction may occur before the 32nd put", it.exists()) }
        assertEquals(10 * 32L, remainingJpgBytes())
    }
}
