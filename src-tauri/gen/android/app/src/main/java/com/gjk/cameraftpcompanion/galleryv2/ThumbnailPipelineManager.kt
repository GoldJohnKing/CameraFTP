/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.os.Handler
import android.os.Looper
import android.util.Log
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * A single thumbnail generation job submitted to the pipeline.
 */
data class ThumbJob(
    val requestId: String,
    val mediaId: String,
    val uri: String,
    val dateModifiedMs: Long,
    val sizeBucket: String,  // "s" or "m"
    val priority: String,    // "visible", "nearby", "prefetch"
    val viewId: String
)

/**
 * Outcome of a completed (or failed/cancelled) thumbnail job.
 */
data class ThumbResult(
    val requestId: String,
    val mediaId: String,
    val status: String,      // "ready", "failed", "cancelled"
    val localPath: String?,
    val errorCode: String?
)

/**
 * Snapshot of the pipeline queue state.
 */
data class QueueStats(
    val pending: Int,
    val running: Int,
    val cacheHitRate: Double
)

/**
 * Tracks an in-flight job and whether it has been cancelled.
 */
private data class RunningJob(
    val job: ThumbJob,
    var cancelled: Boolean = false
)

/**
 * Priority-aware thumbnail pipeline manager with worker thread pool,
 * retry matrix, and backpressure.
 *
 * Maintains three FIFO queues (visible / nearby / prefetch) with per-level
 * quotas and global capacity limits.  Jobs are deduplicated by a composite
 * key of `(mediaId, dateModifiedMs, sizeBucket)` so that re-scrolling the
 * same items does not re-enqueue work.
 *
 * A fixed thread pool executes jobs.  On failure the retry matrix decides
 * whether to re-enqueue based on error code and priority.  Transient I/O
 * errors use exponential backoff before re-enqueueing.
 *
 * All queue/map access is guarded by a [ReentrantLock].
 */
class ThumbnailPipelineManager(poolSize: Int = 3) {

    private val poolSize: Int = poolSize.coerceIn(2, 4)

    companion object {
        const val MAX_QUEUED = 600
        const val VISIBLE_QUOTA = 0.50
        const val NEARBY_QUOTA = 0.35
        const val PREFETCH_QUOTA = 0.15
        const val BATCH_SIZE = 64
        const val BATCH_DELAY_MS = 16L

        /** Backoff delays (ms) for transient I/O retries per priority. */
        private val IO_BACKOFF = mapOf(
            "visible" to 100L,
            "nearby" to 200L,
            "prefetch" to 0L
        )
    }

    // ── Thread pool ─────────────────────────────────────────────────────

    @Volatile
    private var workerPool: ExecutorService =
        Executors.newFixedThreadPool(poolSize)

    // ── Lock ────────────────────────────────────────────────────────────

    private val lock = ReentrantLock()

    // ── Batched result dispatch ─────────────────────────────────────────

    private val resultBuffer = mutableListOf<ThumbResult>()
    private val handler = Handler(Looper.getMainLooper())

    // ── Priority queues ────────────────────────────────────────────────

    private val visibleQueue = ArrayDeque<ThumbJob>()
    private val nearbyQueue = ArrayDeque<ThumbJob>()
    private val prefetchQueue = ArrayDeque<ThumbJob>()

    /**
     * Deduplication map: composite key → requestId.
     * Composite key = "$mediaId:$dateModifiedMs:$sizeBucket".
     */
    private val dedupMap = HashMap<String, String>()

    /** Map of requestIds currently being processed by a worker. */
    private val runningMap = HashMap<String, RunningJob>()

    /** Number of currently running jobs whose priority is "visible". */
    private var visibleRunningCount: Int = 0

    // ── Retry matrix ────────────────────────────────────────────────────

    /**
     * Maps `(errorCode, priority)` → max retry attempts.
     * Attempt count starts at 1 (first try), so a value of 2 means one retry.
     */
    private val retryMatrix: Map<String, Map<String, Int>> = mapOf(
        "io_transient" to mapOf("visible" to 2, "nearby" to 2, "prefetch" to 1),
        "decode_corrupt" to mapOf("visible" to 1, "nearby" to 1, "prefetch" to 1),
        "permission_denied" to mapOf("visible" to 1, "nearby" to 1, "prefetch" to 1),
        "oom_guard" to mapOf("visible" to 1, "nearby" to 1, "prefetch" to 1),
        "cancelled" to mapOf("visible" to 1, "nearby" to 1, "prefetch" to 1)
    )

    /** Per-request retry attempt counter. */
    private val retryCount = HashMap<String, Int>()

    // ── Cache hit tracking ─────────────────────────────────────────────

    private var totalRequests: Long = 0
    private var cacheHits: Long = 0

    // ── Result callback ─────────────────────────────────────────────────

    /**
     * Callback invoked when a job completes, fails permanently, or is
     * cancelled.  Set this before submitting jobs.
     */
    var onResult: ((ThumbResult) -> Unit)? = null
    var decoder: ThumbnailDecoder? = null
    var cacheDir: java.io.File? = null

    /**
     * Callback invoked when a prefetch job is dropped due to queue overflow.
     */
    var onQueueOverflow: ((ThumbJob) -> Unit)? = null

    // ── Public API ─────────────────────────────────────────────────────

    /**
     * Enqueue a thumbnail job respecting deduplication, global capacity,
     * and per-priority quotas.
     *
     * @return `true` if the job was accepted, `false` if rejected.
     */
    fun enqueue(job: ThumbJob): Boolean = lock.withLock {
        val key = compositeKey(job.mediaId, job.dateModifiedMs, job.sizeBucket)

        // Dedup: same key already queued or running
        if (dedupMap.containsKey(key)) {
            return@withLock false
        }

        val totalPending = pendingCountLocked()

        // Global overflow: drop prefetch jobs when at capacity
        if (totalPending >= MAX_QUEUED && job.priority == "prefetch") {
            onQueueOverflow?.invoke(job)
            return@withLock false
        }

        // Evict oldest prefetch jobs to make room for higher-priority jobs
        if (totalPending >= MAX_QUEUED && job.priority != "prefetch") {
            while (pendingCountLocked() >= MAX_QUEUED && prefetchQueue.isNotEmpty()) {
                val evicted = prefetchQueue.removeFirst()
                val evictKey = compositeKey(evicted.mediaId, evicted.dateModifiedMs, evicted.sizeBucket)
                dedupMap.remove(evictKey)
                retryCount.remove(evicted.requestId)
                onQueueOverflow?.invoke(evicted)
            }
        }

        // Per-priority quota check
        val quota = priorityQuota(job.priority)
        val levelCount = queueForPriority(job.priority).size
        if (levelCount >= quota) {
            return@withLock false
        }

        // Accept the job
        dedupMap[key] = job.requestId
        queueForPriority(job.priority).addLast(job)
        true
    }

    /**
     * Total number of pending jobs across all three queues.
     */
    fun pendingCount(): Int = lock.withLock { pendingCountLocked() }

    /**
     * Return a snapshot of current queue statistics.
     */
    fun queueStats(): QueueStats = lock.withLock {
        val hitRate = if (totalRequests > 0) {
            cacheHits.toDouble() / totalRequests.toDouble()
        } else {
            0.0
        }
        QueueStats(
            pending = pendingCountLocked(),
            running = runningMap.size,
            cacheHitRate = hitRate
        )
    }

    /**
     * Record a cache hit (call when a thumbnail is served from cache
     * without needing a pipeline job).
     */
    fun recordCacheHit() = lock.withLock {
        totalRequests++
        cacheHits++
    }

    /**
     * Record a cache miss (call when a job is actually dispatched to a worker).
     */
    fun recordCacheMiss() = lock.withLock {
        totalRequests++
    }

    /**
     * Dequeue the next job from the highest-priority non-empty queue
     * and submit it to the worker pool.
     *
     * Call this after a job completes or when the pipeline is idle.
     * Safe to call even if the pool is fully saturated — the job will
     * be queued internally by the executor.
     */
    fun processNext() {
        val job = lock.withLock {
            // Reserve 1 slot for visible jobs: if non-visible running jobs
            // have consumed (poolSize - 1) slots, only dispatch visible.
            val nonVisibleRunning = runningCountLocked() - visibleRunningCount
            val reservedSlots = poolSize - 1  // 1 slot reserved for visible

            val next = if (nonVisibleRunning >= reservedSlots) {
                // Only dispatch visible jobs when shared slots are full
                visibleQueue.removeFirstOrNull()
            } else {
                visibleQueue.removeFirstOrNull()
                    ?: nearbyQueue.removeFirstOrNull()
                    ?: prefetchQueue.removeFirstOrNull()
            }

            if (next != null) {
                runningMap[next.requestId] = RunningJob(next)
                if (next.priority == "visible") {
                    visibleRunningCount++
                }
            }
            next
        } ?: return

        workerPool.submit {
            executeJob(job)
        }
    }

    /**
     * Cancel a single job by its requestId.
     *
     * - If the job is still queued, it is removed from the queue and
     *   the dedup map, and a `cancelled` result is delivered.
     * - If the job is already running, it is marked as cancelled; the
     *   in-flight work will finish but its result will be dropped and
     *   a `cancelled` result is delivered immediately.
     *
     * @return `true` if the job was found and cancelled, `false` if not found.
     */
    fun cancel(requestId: String): Boolean = lock.withLock {
        // Check running map first
        val running = runningMap[requestId]
        if (running != null) {
            running.cancelled = true
            deliverResult(ThumbResult(requestId, running.job.mediaId, "cancelled", null, "cancelled"))
            return@withLock true
        }

        // Search queues
        for (queue in listOf(visibleQueue, nearbyQueue, prefetchQueue)) {
            val iterator = queue.iterator()
            while (iterator.hasNext()) {
                val queued = iterator.next()
                if (queued.requestId == requestId) {
                    iterator.remove()
                    val key = compositeKey(queued.mediaId, queued.dateModifiedMs, queued.sizeBucket)
                    dedupMap.remove(key)
                    retryCount.remove(requestId)
                    deliverResult(ThumbResult(requestId, queued.mediaId, "cancelled", null, "cancelled"))
                    return@withLock true
                }
            }
        }

        false
    }

    /**
     * Cancel all jobs (queued and running) associated with a given viewId.
     *
     * @return The number of jobs cancelled.
     */
    fun cancelByView(viewId: String): Int = lock.withLock {
        var count = 0

        // Cancel running jobs for this view
        for ((_, running) in runningMap) {
            if (running.job.viewId == viewId && !running.cancelled) {
                running.cancelled = true
                deliverResult(ThumbResult(running.job.requestId, running.job.mediaId, "cancelled", null, "cancelled"))
                count++
            }
        }

        // Cancel queued jobs for this view
        for (queue in listOf(visibleQueue, nearbyQueue, prefetchQueue)) {
            val iterator = queue.iterator()
            while (iterator.hasNext()) {
                val job = iterator.next()
                if (job.viewId == viewId) {
                    iterator.remove()
                    val key = compositeKey(job.mediaId, job.dateModifiedMs, job.sizeBucket)
                    dedupMap.remove(key)
                    retryCount.remove(job.requestId)
                    deliverResult(ThumbResult(job.requestId, job.mediaId, "cancelled", null, "cancelled"))
                    count++
                }
            }
        }

        count
    }

    /**
     * Shut down the worker pool.  No new jobs will be accepted after this.
     */
    fun shutdown() {
        workerPool.shutdownNow()
    }

    // ── Internals ──────────────────────────────────────────────────────

    /**
     * Execute a single job.  Runs on a worker thread.
     */
    private fun executeJob(job: ThumbJob) {
        val cancelled = lock.withLock { runningMap[job.requestId]?.cancelled == true }
        if (cancelled) { finishJob(job, "cancelled", null, "cancelled"); return }

        val dec = decoder
        val dir = cacheDir
        if (dec == null || dir == null) {
            Log.e("ThumbPipeline", "executeJob: decoder=${dec != null} cacheDir=${dir != null}")
            finishJob(job, "failed", null, "decoder_not_configured")
            return
        }

        try {
            val uri = android.net.Uri.parse(job.uri)
            val key = ThumbnailKeyV2.of(job.mediaId, job.dateModifiedMs, job.sizeBucket, 0, 0)
            Log.d("ThumbPipeline", "executeJob: mediaId=${job.mediaId} uri=$uri bucket=${job.sizeBucket}")
            val path = dec.decodeAndSave(uri, job.sizeBucket, dir, key)
            if (path != null) {
                Log.d("ThumbPipeline", "executeJob: ready path=$path")
                finishJob(job, "ready", path, null)
            } else {
                Log.e("ThumbPipeline", "executeJob: decode failed for ${job.uri}")
                finishJob(job, "failed", null, "decode_corrupt")
            }
        } catch (e: Exception) {
            Log.e("ThumbPipeline", "executeJob: exception", e)
            finishJob(job, "failed", null, "io_transient")
        }
    }

    /**
     * Handle job completion: check retry matrix on failure, otherwise
     * deliver the result and free the slot.
     */
    private fun finishJob(
        job: ThumbJob,
        status: String,
        localPath: String?,
        errorCode: String?
    ) {
        lock.withLock {
            val runningJob = runningMap[job.requestId]
            val wasCancelled = runningJob?.cancelled == true
            if (runningJob != null && runningJob.job.priority == "visible") {
                visibleRunningCount--
            }
            runningMap.remove(job.requestId)

            if (wasCancelled || status == "cancelled") {
                retryCount.remove(job.requestId)
                deliverResult(ThumbResult(job.requestId, job.mediaId, "cancelled", null, "cancelled"))
                return@withLock
            }

            if (status == "ready") {
                retryCount.remove(job.requestId)
                val key = compositeKey(job.mediaId, job.dateModifiedMs, job.sizeBucket)
                dedupMap.remove(key)
                deliverResult(ThumbResult(job.requestId, job.mediaId, "ready", localPath, null))
                return@withLock
            }

            // Failure path — check retry matrix
            val attempts = (retryCount[job.requestId] ?: 0) + 1
            retryCount[job.requestId] = attempts

            val maxAttempts = retryMatrix[errorCode]?.get(job.priority) ?: 1
            if (attempts < maxAttempts) {
                // Re-enqueue for retry
                val backoff = if (errorCode == "io_transient") {
                    IO_BACKOFF[job.priority] ?: 0L
                } else {
                    0L
                }

                if (backoff > 0) {
                    // Schedule re-enqueue after backoff on a pool thread
                    workerPool.submit {
                        Thread.sleep(backoff)
                        lock.withLock {
                            queueForPriority(job.priority).addLast(job)
                        }
                    }
                } else {
                    queueForPriority(job.priority).addLast(job)
                }
                return@withLock
            }

            // Exhausted retries — permanent failure
            retryCount.remove(job.requestId)
            val key = compositeKey(job.mediaId, job.dateModifiedMs, job.sizeBucket)
            dedupMap.remove(key)
            deliverResult(ThumbResult(job.requestId, job.mediaId, "failed", null, errorCode))
        }

        // After finishing a job, try to process the next one (backpressure)
        processNext()
    }

    /**
     * Deliver a result by buffering it and dispatching in batches.
     * Buffers up to [BATCH_SIZE] items before flushing, or flushes after
     * [BATCH_DELAY_MS] milliseconds for frame-splitting.
     */
    private fun deliverResult(result: ThumbResult) {
        synchronized(resultBuffer) {
            resultBuffer.add(result)
            if (resultBuffer.size >= BATCH_SIZE) {
                flushResults()
            } else if (resultBuffer.size == 1) {
                handler.postDelayed({ flushResults() }, BATCH_DELAY_MS)
            }
        }
    }

    /**
     * Flush all buffered results through the [onResult] callback.
     */
    private fun flushResults() {
        val batch: List<ThumbResult>
        synchronized(resultBuffer) {
            if (resultBuffer.isEmpty()) return
            batch = resultBuffer.toList()
            resultBuffer.clear()
        }
        batch.forEach { onResult?.invoke(it) }
    }

    /**
     * Dynamically adjust the worker pool size.
     * Shuts down the old pool and creates a new one with the given size.
     */
    fun setWorkerCount(count: Int) {
        val clamped = count.coerceIn(2, 4)
        lock.withLock {
            workerPool.shutdownNow()
            workerPool = Executors.newFixedThreadPool(clamped)
        }
    }

    /**
     * Expose the retry matrix for testing.
     */
    fun getRetryMatrix(): Map<String, Map<String, Int>> = retryMatrix

    private fun pendingCountLocked(): Int =
        visibleQueue.size + nearbyQueue.size + prefetchQueue.size

    private fun runningCountLocked(): Int = runningMap.size

    private fun compositeKey(mediaId: String, dateModifiedMs: Long, sizeBucket: String): String =
        "$mediaId:$dateModifiedMs:$sizeBucket"

    private fun queueForPriority(priority: String): ArrayDeque<ThumbJob> = when (priority) {
        "visible" -> visibleQueue
        "nearby" -> nearbyQueue
        "prefetch" -> prefetchQueue
        else -> throw IllegalArgumentException("Unknown priority: $priority")
    }

    private fun priorityQuota(priority: String): Int = when (priority) {
        "visible" -> (MAX_QUEUED * VISIBLE_QUOTA).toInt()
        "nearby" -> (MAX_QUEUED * NEARBY_QUOTA).toInt()
        "prefetch" -> (MAX_QUEUED * PREFETCH_QUOTA).toInt()
        else -> throw IllegalArgumentException("Unknown priority: $priority")
    }
}
