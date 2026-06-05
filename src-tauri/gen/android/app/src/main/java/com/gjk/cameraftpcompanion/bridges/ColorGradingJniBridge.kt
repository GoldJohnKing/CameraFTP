/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.util.Log
import org.json.JSONObject

/** Pure-Kotlin JSON result parser for JNI bridge responses. No native dependency. */
object JniResultParser {
    fun parseResult(json: String): Result<Unit> {
        val obj = JSONObject(json)
        if (obj.optBoolean("ok", false)) {
            return Result.success(Unit)
        }
        return Result.failure(Exception(obj.optString("error", "Unknown error")))
    }

    fun parseResultWithOutputPath(json: String): Result<String> {
        val obj = JSONObject(json)
        if (obj.optBoolean("ok", false)) {
            val path = obj.optString("outputPath", "")
            if (path.isEmpty()) return Result.failure(Exception("Empty outputPath"))
            return Result.success(path)
        }
        return Result.failure(Exception(obj.optString("error", "Unknown error")))
    }

    fun parseResultWithBuffer(json: String): Result<ByteArray> {
        val obj = JSONObject(json)
        if (obj.optBoolean("ok", false)) {
            val b64 = obj.optString("buffer", "")
            if (b64.isEmpty()) return Result.failure(Exception("Empty buffer"))
            return try {
                Result.success(android.util.Base64.decode(b64, android.util.Base64.DEFAULT))
            } catch (e: Exception) {
                Result.failure(Exception("Base64 decode failed: ${e.message}"))
            }
        }
        return Result.failure(Exception(obj.optString("error", "Unknown error")))
    }
}

class ColorGradingJniBridge {
    companion object {
        private const val TAG = "ColorGradingJni"

        init {
            System.loadLibrary("camera_ftp_companion_lib")
        }

        fun beginPreview(filePath: String): Result<Unit> {
            return try {
                val json = nativeBeginPreview(filePath)
                JniResultParser.parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "beginPreview failed", e)
                Result.failure(e)
            }
        }

        fun applyPreview(
            lutId: String,
            enableLensCorrection: Boolean,
            meteringMode: String,
            evOffset: Float,
            maxWidth: Int,
            maxHeight: Int
        ): Result<ByteArray> {
            return try {
                val json = nativeApplyPreview(lutId, enableLensCorrection, meteringMode, evOffset, maxWidth, maxHeight)
                JniResultParser.parseResultWithBuffer(json)
            } catch (e: Exception) {
                Log.e(TAG, "applyPreview failed", e)
                Result.failure(e)
            }
        }

        fun endPreview(): Result<Unit> {
            return try {
                val json = nativeEndPreview()
                JniResultParser.parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "endPreview failed", e)
                Result.failure(e)
            }
        }

        fun commitPreview(
            lutId: String,
            enableLensCorrection: Boolean,
            meteringMode: String,
            evOffset: Float
        ): Result<String> {
            return try {
                val json = nativeCommitPreview(lutId, enableLensCorrection, meteringMode, evOffset)
                JniResultParser.parseResultWithOutputPath(json)
            } catch (e: Exception) {
                Log.e(TAG, "commitPreview failed", e)
                Result.failure(e)
            }
        }

        fun getPresets(): String {
            return try {
                nativeGetPresets()
            } catch (e: Exception) {
                Log.e(TAG, "getPresets failed", e)
                "[]"
            }
        }

        fun getLastUsed(): String? {
            return try {
                val json = nativeGetLastUsed()
                if (json == "null") null else json
            } catch (e: Exception) {
                Log.e(TAG, "getLastUsed failed", e)
                null
            }
        }

        fun notifyDone(outputPath: String): Result<Unit> {
            return try {
                val json = nativeNotifyDone(outputPath)
                JniResultParser.parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "notifyDone failed", e)
                Result.failure(e)
            }
        }

        fun saveLastUsed(presetId: String, meteringMode: String, evOffset: Float): Result<Unit> {
            return try {
                val json = nativeSaveLastUsed(presetId, meteringMode, evOffset)
                JniResultParser.parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "saveLastUsed failed", e)
                Result.failure(e)
            }
        }

        @JvmStatic
        private external fun nativeBeginPreview(filePath: String): String
        @JvmStatic
        private external fun nativeApplyPreview(lutId: String, enableLensCorrection: Boolean, meteringMode: String, evOffset: Float, maxWidth: Int, maxHeight: Int): String
        @JvmStatic
        private external fun nativeEndPreview(): String
        @JvmStatic
        private external fun nativeCommitPreview(lutId: String, enableLensCorrection: Boolean, meteringMode: String, evOffset: Float): String
        @JvmStatic
        private external fun nativeGetPresets(): String
        @JvmStatic
        private external fun nativeGetLastUsed(): String
        @JvmStatic
        private external fun nativeNotifyDone(outputPath: String): String
        @JvmStatic
        private external fun nativeSaveLastUsed(presetId: String, meteringMode: String, evOffset: Float): String
    }
}
