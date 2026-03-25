/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.Context
import android.util.Log
import android.util.LruCache
import java.io.File

/**
 * Two-tier thumbnail cache for the V2 gallery.
 *
 * - **L1**: In-memory [LruCache] sized by byte count.
 * - **L2**: Disk cache under `thumb/v2/<bucket>/<mediaId>_<hash>.jpg`.
 *
 * L1 default capacity is `min(32 MB, heapClass * 0.08)`.
 * L2 default capacity is 256 MB.
 *
 * File naming convention: `{mediaId}_{hash}.jpg` allows efficient prefix-based
 * deletion when invalidating cache entries by mediaId.
 */
class ThumbnailCacheV2(
    private val l1MaxBytes: Int = defaultL1Bytes(),
    private val l2MaxBytes: Long = DEFAULT_L2_BYTES
) {

    companion object {
        private const val TAG = "ThumbnailCacheV2"
        private const val CACHE_ROOT = "thumb/v2"
        private const val DEFAULT_L2_BYTES = 256L * 1024 * 1024 // 256 MB

        /** Compute a sensible L1 size from the available heap. */
        fun defaultL1Bytes(): Int {
            val heap = Runtime.getRuntime().maxMemory()
            val byHeap = (heap * 0.08).toLong()
            return minOf(32L * 1024 * 1024, byHeap).toInt()
        }
    }

    // ── L1 in-memory cache ────────────────────────────────────────────

    private val l1 = object : LruCache<String, ByteArray>(l1MaxBytes) {
        override fun sizeOf(key: String, value: ByteArray): Int = value.size
    }

    // ── L2 disk cache ─────────────────────────────────────────────────

    private var cacheRoot: File? = null

    /**
     * Initialize the disk cache directory. Must be called once during app startup.
     */
    fun initialize(context: Context) {
        if (cacheRoot == null) {
            cacheRoot = File(context.cacheDir, CACHE_ROOT).apply {
                if (!exists() && !mkdirs()) {
                    Log.e(TAG, "Failed to create disk cache directory: $absolutePath")
                }
            }
        }
    }

    // ── Public API ────────────────────────────────────────────────────

    /**
     * Retrieve a cached thumbnail.
     *
     * Checks L1 first, then falls back to L2 disk. Promotes L2 hits to L1.
     *
     * @param mediaId The media identifier for L2 file lookup
     * @param key The cache key (hash) for L1 lookup
     * @param sizeBucket The size bucket ("s" or "m")
     * @return The cache [File] on disk, or `null` if not cached.
     */
    fun get(mediaId: String, key: String, sizeBucket: String): File? {
        // L1 hit → ensure the file still exists on disk
        if (l1.get(key) != null) {
            val file = diskFile(mediaId, key, sizeBucket)
            if (file.exists()) return file
            // Stale L1 entry; remove it
            l1.remove(key)
        }

        // L2 hit
        val file = diskFile(mediaId, key, sizeBucket)
        if (file.exists()) {
            val data = file.readBytes()
            l1.put(key, data)
            return file
        }

        return null
    }

    /**
     * Store thumbnail data in both L1 and L2.
     *
     * @param mediaId The media identifier for L2 file naming
     * @param key The cache key (hash) for L1 storage
     * @param sizeBucket The size bucket ("s" or "m")
     * @param data The thumbnail JPEG data
     */
    fun put(mediaId: String, key: String, sizeBucket: String, data: ByteArray) {
        // L1
        l1.put(key, data)

        // L2
        val file = diskFile(mediaId, key, sizeBucket)
        file.parentFile?.mkdirs()
        file.writeBytes(data)

        // Enforce L2 capacity
        enforceL2Capacity()
    }

    /**
     * Remove all cached entries for the given media IDs.
     *
     * Uses file name prefix matching to delete all cache files for each mediaId,
     * regardless of the hash key (which varies with dateModifiedMs, etc.).
     *
     * @param mediaIds Set of media IDs to invalidate
     */
    fun invalidateByMediaId(mediaIds: Set<String>) {
        // Clear L1 entries (we need to clear all since we can't match by mediaId)
        // This is safe because L1 will be repopulated on next access
        l1.evictAll()

        // Delete L2 files by mediaId prefix
        val root = cacheRoot ?: return
        for (mediaId in mediaIds) {
            val prefix = "$mediaId" + "_"
            root.walkTopDown()
                .filter { it.isFile && it.name.startsWith(prefix) && it.extension == "jpg" }
                .forEach {
                    if (it.delete()) {
                        Log.d(TAG, "Invalidated by mediaId: ${it.name}")
                    }
                }
        }
    }

    /**
     * Remove all cached entries whose key matches exactly.
     *
     * @param keys Set of cache keys to invalidate
     */
    fun invalidate(keys: Set<String>) {
        for (key in keys) {
            l1.remove(key)
            deleteDiskEntries(key)
        }
    }

    /**
     * Evict entries until total L2 usage is within [maxBytes].
     * Eviction is LRU (oldest files by last-modified time).
     */
    fun cleanup(maxBytes: Long = l2MaxBytes) {
        val root = cacheRoot ?: return
        if (!root.isDirectory) return

        val files = root.walkTopDown()
            .filter { it.isFile && it.extension == "jpg" }
            .toList()

        val totalBytes = files.sumOf { it.length() }
        if (totalBytes <= maxBytes) return

        // Sort oldest first
        val sorted = files.sortedBy { it.lastModified() }
        var freed = totalBytes
        for (file in sorted) {
            if (freed <= maxBytes) break
            val size = file.length()
            if (file.delete()) {
                freed -= size
                Log.d(TAG, "Cleanup evicted: ${file.name}")
            }
        }
    }

    // ── Internals ─────────────────────────────────────────────────────

    private fun diskFile(mediaId: String, key: String, sizeBucket: String): File {
        val root = cacheRoot ?: throw IllegalStateException("ThumbnailCacheV2 not initialized")
        return File(root, "$sizeBucket/${mediaId}_$key.jpg")
    }

    private fun deleteDiskEntries(key: String) {
        val root = cacheRoot ?: return
        root.walkTopDown()
            .filter { it.isFile && it.nameWithoutExtension.endsWith("_$key") }
            .forEach {
                if (it.delete()) {
                    Log.d(TAG, "Invalidated disk entry: ${it.name}")
                }
            }
    }

    private fun enforceL2Capacity() {
        cleanup(l2MaxBytes)
    }
}
