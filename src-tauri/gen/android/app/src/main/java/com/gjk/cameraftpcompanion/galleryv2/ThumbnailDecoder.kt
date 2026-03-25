/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.galleryv2

import android.content.ContentUris
import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import java.io.File

class ThumbnailDecoder(private val context: Context) {
    companion object {
        private const val TAG = "ThumbnailDecoder"
        private const val QUALITY = 92
    }

    /**
     * Decode and save a thumbnail.
     *
     * @param uri The media content URI
     * @param sizeBucket The size bucket ("s" for small, "m" for medium)
     * @param cacheDir The cache directory root
     * @param mediaId The media identifier for file naming
     * @param key The cache key (hash) for file naming
     * @return The absolute path to the saved file, or null on failure
     */
    fun decodeAndSave(uri: Uri, sizeBucket: String, cacheDir: File, mediaId: String, key: String): String? {
        return try {
            val bitmap = loadBitmap(uri) ?: return null
            val target = if (sizeBucket == "s") 200 else 360
            val scaled = centerCrop(bitmap, target)
            val dir = File(cacheDir, sizeBucket).apply { mkdirs() }
            // File naming: {mediaId}_{key}.jpg - allows prefix-based deletion by mediaId
            val file = File(dir, "${mediaId}_$key.jpg")
            file.outputStream().use { scaled.compress(Bitmap.CompressFormat.JPEG, QUALITY, it) }
            file.absolutePath
        } catch (e: Exception) {
            Log.e(TAG, "decodeAndSave error uri=$uri", e)
            null
        }
    }

    private fun loadBitmap(uri: Uri): Bitmap? = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            val src = ImageDecoder.createSource(context.contentResolver, uri)
            ImageDecoder.decodeBitmap(src) { decoder, info, _ ->
                val w = info.size.width; val h = info.size.height
                if (w > 0 && h > 0) {
                    val s = 720f / minOf(w, h)
                    decoder.setTargetSize(maxOf(1, (w*s).toInt()), maxOf(1, (h*s).toInt()))
                }
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            }
        } else {
            @Suppress("DEPRECATION")
            MediaStore.Images.Thumbnails.getThumbnail(
                context.contentResolver, ContentUris.parseId(uri),
                MediaStore.Images.Thumbnails.MINI_KIND, null)
        }
    } catch (e: Exception) { Log.e(TAG, "loadBitmap error", e); null }

    private fun centerCrop(bmp: Bitmap, target: Int): Bitmap {
        val e = minOf(bmp.width, bmp.height)
        val c = Bitmap.createBitmap(bmp, (bmp.width-e)/2, (bmp.height-e)/2, e, e)
        return if (c.width == target) c else Bitmap.createScaledBitmap(c, target, target, true)
    }
}
