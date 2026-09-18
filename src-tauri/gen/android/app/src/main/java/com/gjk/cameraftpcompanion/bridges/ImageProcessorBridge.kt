/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Context
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.Log
import java.io.File
import java.util.UUID
import kotlin.math.roundToInt

class ImageProcessorBridge {
    companion object {
        private const val TAG = "ImageProcessorBridge"

        /** Prefix for the temp JPEG files exchanged with Rust over JNI. */
        private const val TEMP_FILE_PREFIX = "ai_edit_"

        /**
         * Temp files older than this are swept at the start of each call.
         * Normally the Rust side deletes the file right after reading it;
         * the sweep only covers files orphaned by a process death between
         * the Kotlin write and the Rust read. One hour safely exceeds any
         * in-flight request lifetime, so concurrent requests never see
         * each other's files swept.
         */
        private const val TEMP_FILE_STALE_MS = 60L * 60 * 1000

        /**
         * Decode an image file, downsample to fit [maxLongSide], re-encode as
         * JPEG and write it to a temp file under [context]'s cacheDir.
         * Uses ImageDecoder.setTargetSize() for single-pass decode+downsample.
         *
         * Returns the ABSOLUTE PATH of the temp JPEG file (the Rust caller
         * reads the bytes and deletes the file), or null on failure
         * (including OOM) — the temp file, if already created, is deleted
         * on failure before returning null.
         */
        @JvmStatic
        fun prepareForUpload(
            context: Context,
            filePath: String,
            maxLongSide: Int,
            jpegQuality: Int,
        ): String? {
            val cacheDir = context.cacheDir
            cleanupStaleTempFiles(cacheDir)
            var tempFile: File? = null
            return try {
                val file = File(filePath)
                val source = ImageDecoder.createSource(file)
                val bitmap = ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                    val w = info.size.width
                    val h = info.size.height
                    val longSide = maxOf(w, h)
                    if (longSide > maxLongSide) {
                        val scale = maxLongSide.toFloat() / longSide.toFloat()
                        decoder.setTargetSize(
                            maxOf(1, (w * scale).roundToInt()),
                            maxOf(1, (h * scale).roundToInt())
                        )
                    }
                    decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                }

                tempFile = File(cacheDir, "${TEMP_FILE_PREFIX}${UUID.randomUUID()}.jpg")
                val compressed = tempFile.outputStream().use { out ->
                    bitmap.compress(Bitmap.CompressFormat.JPEG, jpegQuality, out)
                }
                bitmap.recycle()
                if (!compressed) {
                    throw java.io.IOException("bitmap.compress returned false")
                }
                tempFile.absolutePath
            } catch (e: OutOfMemoryError) {
                Log.e(TAG, "OOM preparing image: $filePath", e)
                tempFile?.delete()
                null
            } catch (e: Exception) {
                Log.e(TAG, "Failed to prepare image: $filePath", e)
                tempFile?.delete()
                null
            }
        }

        /**
         * Best-effort sweep of orphaned `ai_edit_*.jpg` temp files (see
         * [TEMP_FILE_STALE_MS]). Failures are logged and ignored — a stale
         * file only costs disk space in cacheDir.
         */
        private fun cleanupStaleTempFiles(cacheDir: File) {
            try {
                val cutoff = System.currentTimeMillis() - TEMP_FILE_STALE_MS
                cacheDir.listFiles { f ->
                    f.isFile && f.name.startsWith(TEMP_FILE_PREFIX)
                }?.forEach { f ->
                    if (f.lastModified() < cutoff && f.delete()) {
                        Log.d(TAG, "Swept stale AI-edit temp file: ${f.name}")
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "cleanupStaleTempFiles failed", e)
            }
        }
    }
}
