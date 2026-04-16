/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.Base64
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.File
import kotlin.math.roundToInt

class ImageProcessorBridge {
    companion object {
        private const val TAG = "ImageProcessorBridge"

        /**
         * Decode an image file, downsample to fit [maxLongSide], and re-encode as JPEG.
         * Uses ImageDecoder.setTargetSize() for single-pass decode+downsample.
         * Returns base64-encoded JPEG, or null on failure (including OOM).
         */
        @JvmStatic
        fun prepareForUpload(filePath: String, maxLongSide: Int, jpegQuality: Int): String? {
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

                val stream = ByteArrayOutputStream()
                bitmap.compress(Bitmap.CompressFormat.JPEG, jpegQuality, stream)
                val bytes = stream.toByteArray()

                Base64.encodeToString(bytes, Base64.NO_WRAP)
            } catch (e: OutOfMemoryError) {
                Log.e(TAG, "OOM preparing image: $filePath", e)
                null
            } catch (e: Exception) {
                Log.e(TAG, "Failed to prepare image: $filePath", e)
                null
            }
        }
    }
}
