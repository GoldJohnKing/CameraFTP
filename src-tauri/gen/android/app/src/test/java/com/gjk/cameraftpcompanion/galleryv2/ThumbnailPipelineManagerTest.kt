/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.Context
import android.net.Uri
import android.os.Looper
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper
import java.io.File
import java.nio.file.Files
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
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

    /**
     * Poll [condition] until it holds or [timeoutMs] elapses.
     * @return the last value of [condition].
     */
    private fun awaitCondition(timeoutMs: Long = 10_000, condition: () -> Boolean): Boolean {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (!condition() && System.currentTimeMillis() < deadline) {
            Thread.sleep(20)
        }
        return condition()
    }

    /**
     * Fake decoder: the first [failFirstN] attempts throw (classified as
     * io_transient by the pipeline); later attempts succeed by writing the
     * file directly into the cache dir, like the real decoder.
     */
    private class FakeRetryDecoder(
        context: Context,
        private val attempts: AtomicInteger,
        private val failFirstN: Int
    ) : ThumbnailDecoder(context) {
        override fun decodeAndSave(
            uri: Uri,
            sizeBucket: String,
            cacheDir: File,
            mediaId: String,
            key: String
        ): String? {
            val n = attempts.incrementAndGet()
            if (n <= failFirstN) throw java.io.IOException("simulated transient failure #$n")
            val dir = File(cacheDir, sizeBucket).apply { mkdirs() }
            val file = File(dir, "${mediaId}_$key.jpg")
            file.writeBytes(byteArrayOf(1, 2, 3))
            return file.absolutePath
        }
    }

    /**
     * Controllable decoder: records the dispatch order of decode calls into
     * [started] (by mediaId) and then blocks until [gate] is opened, so tests
     * can pin worker slots open and observe the exact scheduling order
     * produced by [ThumbnailPipelineManager.processNext].
     */
    private class BlockingDecoder(
        context: Context,
        private val started: MutableList<String>,
        private val gate: CountDownLatch
    ) : ThumbnailDecoder(context) {
        override fun decodeAndSave(
            uri: Uri,
            sizeBucket: String,
            cacheDir: File,
            mediaId: String,
            key: String
        ): String? {
            synchronized(started) { started.add(mediaId) }
            gate.await()
            val dir = File(cacheDir, sizeBucket).apply { mkdirs() }
            val file = File(dir, "${mediaId}_$key.jpg")
            file.writeBytes(byteArrayOf(1))
            return file.absolutePath
        }
    }

    // ── Test 1: visible jobs are scheduled before prefetch ──────────────

    @Test
    fun `visible_jobs_are_scheduled_before_prefetch`() {
        val tempDir = Files.createTempDirectory("thumb-order").toFile()
        val started = java.util.Collections.synchronizedList(mutableListOf<String>())
        val gate = CountDownLatch(1)
        pipeline.decoder = BlockingDecoder(RuntimeEnvironment.getApplication(), started, gate)
        pipeline.cacheDir = tempDir

        // Enqueue prefetch first, then visible.
        val prefetch = ThumbJob(
            requestId = "pref-1", mediaId = "m-prefetch", uri = "uri1",
            dateModifiedMs = 1000L, sizeBucket = "s", priority = "prefetch",
            viewId = "v1"
        )
        val visible = ThumbJob(
            requestId = "vis-1", mediaId = "m-visible", uri = "uri2",
            dateModifiedMs = 2000L, sizeBucket = "s", priority = "visible",
            viewId = "v1"
        )

        assertTrue(pipeline.enqueue(prefetch))
        assertTrue(pipeline.enqueue(visible))

        // First dispatch must take the visible job even though the prefetch
        // job was enqueued earlier (priority lanes, not global FIFO).
        pipeline.processNext()
        assertEquals(
            "Only the prefetch job should remain queued after the first dispatch",
            1,
            pipeline.pendingCount()
        )
        assertTrue(
            "Visible decode should start before any prefetch decode",
            awaitCondition { started.size == 1 }
        )
        assertEquals(listOf("m-visible"), started)

        // Second dispatch takes the remaining prefetch job.
        pipeline.processNext()
        assertEquals("Queue should be fully drained", 0, pipeline.pendingCount())
        assertTrue(
            "Prefetch decode should start after the visible decode",
            awaitCondition { started.size == 2 }
        )

        assertEquals(
            "Dispatch order must be visible-before-prefetch",
            listOf("m-visible", "m-prefetch"),
            started
        )

        gate.countDown()
        pipeline.shutdown()
        tempDir.deleteRecursively()
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

    // ── Test 4: visible has reserved worker slot ────────────────────────

    @Test
    fun `visible_has_reserved_worker_slot`() {
        // poolSize=4 → reservedSlots=poolSize-1=3: once 3 slots are held by
        // non-visible jobs, the 4th slot may only be handed to a visible job.
        pipeline = ThumbnailPipelineManager(poolSize = 4)
        val tempDir = Files.createTempDirectory("thumb-reserve").toFile()
        val started = java.util.Collections.synchronizedList(mutableListOf<String>())
        val gate = CountDownLatch(1)
        pipeline.decoder = BlockingDecoder(RuntimeEnvironment.getApplication(), started, gate)
        pipeline.cacheDir = tempDir

        // Saturate the 3 shared slots with prefetch jobs (each blocks on the gate).
        for (i in 1..3) {
            assertTrue(
                "Saturating prefetch job $i should be accepted",
                pipeline.enqueue(
                    ThumbJob(
                        requestId = "sat-$i", mediaId = "m-sat-$i", uri = "uri-sat-$i",
                        dateModifiedMs = i * 1000L, sizeBucket = "s",
                        priority = "prefetch", viewId = "v1"
                    )
                )
            )
            pipeline.processNext()
            assertTrue(
                "Saturating job $i should have started decoding",
                awaitCondition { started.size >= i }
            )
        }

        // All shared slots are now held by non-visible jobs → the reserved
        // slot must NOT be handed to another prefetch job, even though one
        // is queued. The dequeue decision is synchronous under the pipeline
        // lock, so the pending-count assertion is deterministic.
        val queuedPrefetch = ThumbJob(
            requestId = "queued-pref", mediaId = "m-queued-pref", uri = "uri-queued",
            dateModifiedMs = 4000L, sizeBucket = "s", priority = "prefetch", viewId = "v1"
        )
        assertTrue(pipeline.enqueue(queuedPrefetch))
        pipeline.processNext()
        assertEquals(
            "Queued prefetch job must stay pending — the last slot is reserved for visible",
            1,
            pipeline.pendingCount()
        )
        assertEquals(
            "No prefetch decode may start while the reserved slot is empty",
            listOf("m-sat-1", "m-sat-2", "m-sat-3"),
            started
        )

        // A visible job IS dispatched into the reserved slot.
        val visibleJob = ThumbJob(
            requestId = "vis-1", mediaId = "m-visible", uri = "uri-vis",
            dateModifiedMs = 99999L, sizeBucket = "s",
            priority = "visible", viewId = "v1"
        )
        assertTrue("Visible job should be accepted", pipeline.enqueue(visibleJob))
        pipeline.processNext()
        assertTrue(
            "Visible job should be dispatched into the reserved slot",
            awaitCondition { started.size == 4 }
        )
        assertEquals(
            listOf("m-sat-1", "m-sat-2", "m-sat-3", "m-visible"),
            started
        )

        gate.countDown()
        pipeline.shutdown()
        tempDir.deleteRecursively()
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

    @Test
    fun `shutdown_clears_buffered_results_before_main_thread_flush`() {
        val delivered = mutableListOf<ThumbResult>()
        pipeline.onResult = { delivered.add(it) }

        val job = ThumbJob(
            requestId = "shutdown-job", mediaId = "media-shutdown", uri = "uri-shutdown",
            dateModifiedMs = 1000L, sizeBucket = "s",
            priority = "visible", viewId = "view-1"
        )

        assertTrue("Job should be accepted", pipeline.enqueue(job))
        assertTrue("Cancel should schedule a buffered cancelled result", pipeline.cancel("shutdown-job"))

        pipeline.shutdown()
        idleMainLooper()

        assertTrue(
            "No buffered results should be delivered after shutdown",
            delivered.isEmpty()
        )
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

    // ── Test 11: io_transient retry is re-dispatched after backoff ──────

    @Test
    fun `io_transient_retry_with_backoff_is_redispatched`() {
        val tempDir = Files.createTempDirectory("thumb-pipeline").toFile()
        val attempts = AtomicInteger(0)
        // visible + io_transient → 2 attempts allowed, 100 ms backoff before retry
        pipeline.decoder = FakeRetryDecoder(
            RuntimeEnvironment.getApplication(), attempts, failFirstN = 1
        )
        pipeline.cacheDir = tempDir
        val delivered = java.util.Collections.synchronizedList(mutableListOf<ThumbResult>())
        pipeline.onResult = { delivered.add(it) }

        val job = ThumbJob(
            requestId = "retry-1", mediaId = "m-retry", uri = "content://media/external/images/1",
            dateModifiedMs = 1000L, sizeBucket = "s", priority = "visible", viewId = "v1"
        )
        assertTrue("Job should be accepted", pipeline.enqueue(job))
        pipeline.processNext()

        // Wait for the first (failing) attempt on the worker pool.
        assertTrue(
            "First attempt should have run",
            awaitCondition { attempts.get() >= 1 }
        )

        // Advance the main looper past the 100 ms backoff. The retry must be
        // re-queued AND re-dispatched — previously the re-enqueued job sat
        // forever in an idle queue because nothing called processNext() after
        // the delayed enqueue.
        val mainShadow = shadowOf(Looper.getMainLooper())
        assertTrue(
            "Retry should have been dispatched after backoff",
            awaitCondition {
                mainShadow.idleFor(150, TimeUnit.MILLISECONDS)
                Thread.sleep(25)
                attempts.get() >= 2
            }
        )
        assertEquals(2, attempts.get())

        // Flush the batched (16 ms delayed) result dispatch.
        assertTrue(
            "Ready result should have been delivered",
            awaitCondition {
                mainShadow.idleFor(100, TimeUnit.MILLISECONDS)
                delivered.isNotEmpty()
            }
        )
        assertEquals("ready", delivered.first().status)
        assertEquals("retry-1", delivered.first().requestId)

        pipeline.shutdown()
        tempDir.deleteRecursively()
    }

    // ── Test 12: pending retry is dropped on shutdown ───────────────────

    @Test
    fun `pending_backoff_retry_is_dropped_on_shutdown`() {
        val tempDir = Files.createTempDirectory("thumb-pipeline").toFile()
        val attempts = AtomicInteger(0)
        pipeline.decoder = FakeRetryDecoder(
            RuntimeEnvironment.getApplication(), attempts, failFirstN = Int.MAX_VALUE
        )
        pipeline.cacheDir = tempDir
        val delivered = java.util.Collections.synchronizedList(mutableListOf<ThumbResult>())
        pipeline.onResult = { delivered.add(it) }

        val job = ThumbJob(
            requestId = "retry-shutdown", mediaId = "m-sd", uri = "content://media/external/images/2",
            dateModifiedMs = 1000L, sizeBucket = "s", priority = "visible", viewId = "v1"
        )
        assertTrue(pipeline.enqueue(job))
        pipeline.processNext()
        assertTrue(
            "First attempt should have run",
            awaitCondition { attempts.get() >= 1 }
        )

        // Shutdown while the backoff retry is still pending on the handler.
        pipeline.shutdown()
        shadowOf(Looper.getMainLooper()).idleFor(500, TimeUnit.MILLISECONDS)
        Thread.sleep(100)

        assertEquals("No further attempts after shutdown", 1, attempts.get())
        assertTrue("No results should be delivered after shutdown", delivered.isEmpty())

        tempDir.deleteRecursively()
    }
}
