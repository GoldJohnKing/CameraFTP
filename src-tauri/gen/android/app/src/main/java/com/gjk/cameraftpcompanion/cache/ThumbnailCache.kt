/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.cache

import android.content.Context
import android.net.Uri
import android.util.Log
import java.io.File
import java.security.MessageDigest

/**
 * LRU cache for image thumbnails.
 * Tracks total bytes used and evicts oldest entries when capacity is exceeded.
 */
class ThumbnailCache(private val maxBytes: Long) {

    companion object {
        private const val TAG = "ThumbnailCache"
        private const val THUMBNAIL_SUBDIR = "thumbnails"
    }

    private val entries = mutableListOf<CacheEntry>()
    private var currentBytes = 0L
    private var cacheDir: File? = null

    data class CacheEntry(
        val uri: Uri,
        val dateModified: Long,
        val key: String,
        var bytes: Int
    )

    /**
     * Generate a unique cache key for a URI + dateModified combination.
     * The key changes when dateModified changes, ensuring stale thumbnails are replaced.
     */
    fun keyFor(uri: Uri, dateModified: Long): String {
        val combined = uri.toString() + dateModified.toString()
        return hash(combined)
    }

    /**
     * Add or update a cached thumbnail.
     * Evicts oldest entries if capacity would be exceeded.
     */
    fun put(uri: Uri, dateModified: Long, bytes: Int) {
        val key = keyFor(uri, dateModified)

        // Remove existing entry for this URI (any dateModified)
        evictIfPresent(uri)

        // Check if we need to evict to make room
        while (entries.isNotEmpty() && currentBytes + bytes > maxBytes) {
            evictOldest()
        }

        // Add new entry
        val entry = CacheEntry(uri, dateModified, key, bytes)
        entries.add(entry)
        currentBytes += bytes

        Log.d(TAG, "Added thumbnail: uri=$uri, bytes=$bytes, totalBytes=$currentBytes/${maxBytes}")
    }

    /**
     * Check if a thumbnail is cached for the given URI and dateModified.
     */
    fun contains(uri: Uri, dateModified: Long): Boolean {
        val key = keyFor(uri, dateModified)
        return entries.any { it.key == key }
    }

    /**
     * Remove any cached entry for the given URI (regardless of dateModified).
     * Also deletes the physical cache file.
     */
    fun evictIfPresent(uri: Uri) {
        val iterator = entries.iterator()
        while (iterator.hasNext()) {
            val entry = iterator.next()
            if (entry.uri == uri) {
                currentBytes -= entry.bytes
                iterator.remove()
                deleteCacheFile(entry.key)
                Log.d(TAG, "Evicted thumbnail for uri=$uri")
            }
        }
    }

    /**
     * Get the cache file path for a thumbnail.
     * Returns null if not cached.
     */
    fun getCacheFile(uri: Uri, dateModified: Long): File? {
        if (!contains(uri, dateModified)) return null

        val key = keyFor(uri, dateModified)
        val dir = getOrCreateCacheDir() ?: return null
        val file = File(dir, "thumb_$key.jpg")
        return if (file.exists()) file else null
    }

    /**
     * Initialize the cache directory lazily.
     */
    fun initialize(context: Context) {
        if (cacheDir == null) {
            cacheDir = File(context.cacheDir, THUMBNAIL_SUBDIR).apply {
                if (!exists()) {
                    if (!mkdirs()) {
                        Log.e(TAG, "Failed to create thumbnail cache directory: $absolutePath")
                    }
                }
            }
        }
    }

    private fun evictOldest() {
        if (entries.isEmpty()) return

        val oldest = entries.removeAt(0)
        currentBytes -= oldest.bytes
        deleteCacheFile(oldest.key)
        Log.d(TAG, "Evicted oldest thumbnail: uri=${oldest.uri}")
    }

    private fun getOrCreateCacheDir(): File? {
        return cacheDir
    }

    private fun deleteCacheFile(key: String) {
        val dir = cacheDir ?: return
        val file = File(dir, "thumb_$key.jpg")
        if (file.exists() && file.delete()) {
            Log.d(TAG, "Deleted cache file: ${file.name}")
        }
    }

    private fun hash(input: String): String {
        val md = MessageDigest.getInstance("MD5")
        val digest = md.digest(input.toByteArray())
        return digest.joinToString("") { "%02x".format(it) }
    }
}

/**
 * Singleton provider for the ThumbnailCache instance.
 */
object ThumbnailCacheProvider {
    val instance: ThumbnailCache = ThumbnailCache(100 * 1024 * 1024) // 100MB

    /**
     * Initialize the cache with application context.
     * Should be called once during app startup.
     */
    fun initialize(context: Context) {
        instance.initialize(context)
    }
}
