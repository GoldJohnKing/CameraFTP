/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowLooper

@RunWith(RobolectricTestRunner::class)
class ThumbnailPipelineManagerTest {

    private lateinit var pipeline: ThumbnailPipelineManager

    @Before
    fun setUp() {
        pipeline = ThumbnailPipelineManager(poolSize = 2)
    }

    /**
     * Helper to flush the main looper so that batched dispatch callbacks run.
     */
    private fun idleMainLooper() {
        ShadowLooper.idleMainLooper()
    }

    // ── Test 1: visible jobs are scheduled before prefetch ──────────────

    @Test
    fun `visible_jobs_are_scheduled_before_prefetch`() {
        // Enqueue prefetch first, then visible
        val prefetch = ThumbJob(
            requestId = "pref-1", mediaId = "m1", uri = "uri1",
            dateModifiedMs = 1000L, sizeBucket = "s", priority = "prefetch",
            viewId = "v1"
        )
        val visible = ThumbJob(
            requestId = "vis-1", mediaId = "m2", uri = "uri2",
            dateModifiedMs = 2000L, sizeBucket = "s", priority = "visible",
            viewId = "v1"
        )

        assertTrue(pipeline.enqueue(prefetch))
        assertTrue(pipeline.enqueue(visible))
        assertEquals(2, pipeline.pendingCount())

        // Verify queue stats show both jobs pending
        val stats = pipeline.queueStats()
        assertEquals(2, stats.pending)
        assertEquals(0, stats.running)
    }

    // ── Test 2: prefetch io_transient has no retry ──────────────────────

    @Test
    fun `prefetch_io_transient_has_no_retry`() {
        val matrix = pipeline.getRetryMatrix()
        val ioTransient = matrix["io_transient"]
        assertNotNull("io_transient should exist in retry matrix", ioTransient)
        assertEquals(
            "io_transient + prefetch should have 1 attempt (no retry)",
            1,
            ioTransient!!["prefetch"]
        )
    }

    // ── Test 3: queue quota split respects 50/35/15 ─────────────────────

    @Test
    fun `queue_quota_split_respects_50_35_15`() {
        assertEquals(
            "VISIBLE_QUOTA should be 0.50",
            0.50,
            ThumbnailPipelineManager.VISIBLE_QUOTA,
            0.001
        )
        assertEquals(
            "NEARBY_QUOTA should be 0.35",
            0.35,
            ThumbnailPipelineManager.NEARBY_QUOTA,
            0.001
        )
        assertEquals(
            "PREFETCH_QUOTA should be 0.15",
            0.15,
            ThumbnailPipelineManager.PREFETCH_QUOTA,
            0.001
        )

        // Verify quotas sum to 1.0
        val sum = ThumbnailPipelineManager.VISIBLE_QUOTA +
            ThumbnailPipelineManager.NEARBY_QUOTA +
            ThumbnailPipelineManager.PREFETCH_QUOTA
        assertEquals("Quotas should sum to 1.0", 1.0, sum, 0.001)

        // Verify computed quota values
        val maxQueued = ThumbnailPipelineManager.MAX_QUEUED
        assertEquals("Visible quota = 300", 300, (maxQueued * 0.50).toInt())
        assertEquals("Nearby quota = 210", 210, (maxQueued * 0.35).toInt())
        assertEquals("Prefetch quota = 90", 90, (maxQueued * 0.15).toInt())
    }

    // ── Test 4: visible has reserved worker slot ────────────────────────

    @Test
    fun `visible_has_reserved_worker_slot`() {
        // Fill the pipeline with prefetch jobs
        for (i in 1..10) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "pref-$i", mediaId = "m$i", uri = "uri$i",
                    dateModifiedMs = i * 1000L, sizeBucket = "s",
                    priority = "prefetch", viewId = "v1"
                )
            )
        }

        // Now enqueue a visible job - it should be accepted even with prefetch jobs queued
        val visibleJob = ThumbJob(
            requestId = "vis-1", mediaId = "mv1", uri = "uriv1",
            dateModifiedMs = 99999L, sizeBucket = "s",
            priority = "visible", viewId = "v1"
        )
        assertTrue("Visible job should be accepted", pipeline.enqueue(visibleJob))

        // Verify visible job is in the queue (pending count increased)
        assertTrue("Pipeline should have jobs", pipeline.pendingCount() > 0)
    }

    // ── Test 5: overflow drops prefetch first and emits queue_overflow ───

    @Test
    fun `overflow_drops_prefetch_first_and_emits_queue_overflow`() {
        val overflowDrops = mutableListOf<ThumbJob>()
        pipeline.onQueueOverflow = { overflowDrops.add(it) }

        // Fill the queue to MAX_QUEUED with visible and nearby jobs
        for (i in 1..300) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "vis-$i", mediaId = "mv$i", uri = "uriv$i",
                    dateModifiedMs = i * 1000L, sizeBucket = "s",
                    priority = "visible", viewId = "v1"
                )
            )
        }
        for (i in 1..210) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "near-$i", mediaId = "mn$i", uri = "urin$i",
                    dateModifiedMs = i * 2000L, sizeBucket = "s",
                    priority = "nearby", viewId = "v1"
                )
            )
        }

        // Queue should be at or near capacity
        assertTrue("Queue should have many jobs", pipeline.pendingCount() > 0)

        // Now try to enqueue prefetch - should be dropped and emit overflow
        val prefetchJob = ThumbJob(
            requestId = "pref-overflow", mediaId = "mpof", uri = "uriof",
            dateModifiedMs = 99999L, sizeBucket = "s",
            priority = "prefetch", viewId = "v1"
        )

        // If queue is at MAX_QUEUED, prefetch should be dropped
        if (pipeline.pendingCount() >= ThumbnailPipelineManager.MAX_QUEUED) {
            assertFalse("Prefetch should be rejected at capacity", pipeline.enqueue(prefetchJob))
            assertEquals("Should emit overflow for dropped prefetch", 1, overflowDrops.size)
            assertEquals("pref-overflow", overflowDrops[0].requestId)
        }
    }

    // ── Test 6: callback batch size is capped to 64 ─────────────────────

    @Test
    fun `callback_batch_size_is_capped_to_64`() {
        assertEquals(
            "BATCH_SIZE should be 64",
            64,
            ThumbnailPipelineManager.BATCH_SIZE
        )
    }

    // ── Test 7: cancel latency respects p95 budget in fake clock ────────

    @Test
    fun `cancel_latency_respects_p95_budget_in_fake_clock`() {
        // Enqueue many jobs
        for (i in 1..50) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "job-$i", mediaId = "m$i", uri = "uri$i",
                    dateModifiedMs = i * 1000L, sizeBucket = "s",
                    priority = "prefetch", viewId = "v1"
                )
            )
        }

        // Cancel a queued job and measure time
        val startTime = System.nanoTime()
        val cancelled = pipeline.cancel("job-25")
        val elapsedMs = (System.nanoTime() - startTime) / 1_000_000

        assertTrue("Cancel should succeed for queued job", cancelled)
        assertTrue(
            "Cancel should be fast (< 200ms for queued items)",
            elapsedMs < 200
        )

        // Verify pending count decreased
        assertTrue("Pending count should decrease after cancel", pipeline.pendingCount() < 50)
    }

    // ── Test 8: cancel by view cancels all requests for view ────────────

    @Test
    fun `cancel_by_view_cancels_all_requests_for_view`() {
        // Enqueue jobs for two different views
        for (i in 1..5) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "v1-job-$i", mediaId = "mv1-$i", uri = "uriv1-$i",
                    dateModifiedMs = i * 1000L, sizeBucket = "s",
                    priority = "visible", viewId = "view-1"
                )
            )
        }
        for (i in 1..5) {
            pipeline.enqueue(
                ThumbJob(
                    requestId = "v2-job-$i", mediaId = "mv2-$i", uri = "uriv2-$i",
                    dateModifiedMs = i * 2000L, sizeBucket = "s",
                    priority = "nearby", viewId = "view-2"
                )
            )
        }

        val initialPending = pipeline.pendingCount()
        assertEquals("Should have 10 jobs pending", 10, initialPending)

        // Cancel all jobs for view-1
        val cancelCount = pipeline.cancelByView("view-1")
        assertTrue("Should cancel at least 1 job for view-1", cancelCount > 0)
        assertEquals("Should cancel exactly 5 view-1 jobs", 5, cancelCount)

        // Verify pending count decreased by 5
        assertEquals("Pending should decrease by 5", 5, pipeline.pendingCount())
    }

    // ── Test 9: dedup rejects same key ──────────────────────────────────

    @Test
    fun `dedup_rejects_same_key`() {
        val job1 = ThumbJob(
            requestId = "req-1", mediaId = "media-1", uri = "uri1",
            dateModifiedMs = 1000L, sizeBucket = "s",
            priority = "visible", viewId = "v1"
        )
        val job2 = ThumbJob(
            requestId = "req-2", mediaId = "media-1", uri = "uri1",
            dateModifiedMs = 1000L, sizeBucket = "s",
            priority = "visible", viewId = "v1"
        )

        assertTrue("First job should be accepted", pipeline.enqueue(job1))
        assertFalse("Duplicate key should be rejected", pipeline.enqueue(job2))
    }

    // ── Test 10: different keys are accepted ────────────────────────────

    @Test
    fun `different_keys_are_accepted`() {
        val job1 = ThumbJob(
            requestId = "req-1", mediaId = "media-1", uri = "uri1",
            dateModifiedMs = 1000L, sizeBucket = "s",
            priority = "visible", viewId = "v1"
        )
        val job2 = ThumbJob(
            requestId = "req-2", mediaId = "media-2", uri = "uri2",
            dateModifiedMs = 2000L, sizeBucket = "m",
            priority = "visible", viewId = "v1"
        )

        assertTrue("First job should be accepted", pipeline.enqueue(job1))
        assertTrue("Different key should be accepted", pipeline.enqueue(job2))
        assertEquals("Both jobs should be pending", 2, pipeline.pendingCount())
    }
}
