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
 * - **L2**: Disk cache under `thumb/v2/<bucket>/<hash>.jpg`.
 *
 * L1 default capacity is `min(32 MB, heapClass * 0.08)`.
 * L2 default capacity is 256 MB.
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
     * @return The cache [File] on disk, or `null` if not cached.
     */
    fun get(key: String, sizeBucket: String): File? {
        // L1 hit → ensure the file still exists on disk
        if (l1.get(key) != null) {
            val file = diskFile(key, sizeBucket)
            if (file.exists()) return file
            // Stale L1 entry; remove it
            l1.remove(key)
        }

        // L2 hit
        val file = diskFile(key, sizeBucket)
        if (file.exists()) {
            val data = file.readBytes()
            l1.put(key, data)
            return file
        }

        return null
    }

    /**
     * Store thumbnail data in both L1 and L2.
     */
    fun put(key: String, sizeBucket: String, data: ByteArray) {
        // L1
        l1.put(key, data)

        // L2
        val file = diskFile(key, sizeBucket)
        file.parentFile?.mkdirs()
        file.writeBytes(data)

        // Enforce L2 capacity
        enforceL2Capacity()
    }

    /**
     * Remove all cached entries whose key was derived from any of the given media IDs.
     *
     * Because we don't store the original mediaId in the key, callers must supply
     * the set of keys to remove. A higher-level index should map mediaId → keys.
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

    private fun diskFile(key: String, sizeBucket: String): File {
        val root = cacheRoot ?: throw IllegalStateException("ThumbnailCacheV2 not initialized")
        return File(root, "$sizeBucket/$key.jpg")
    }

    private fun deleteDiskEntries(key: String) {
        val root = cacheRoot ?: return
        root.walkTopDown()
            .filter { it.isFile && it.nameWithoutExtension == key }
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
