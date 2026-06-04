/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.util.Log
import org.json.JSONObject

class ColorGradingJniBridge {
    companion object {
        private const val TAG = "ColorGradingJni"

        init {
            System.loadLibrary("camera_ftp_companion_lib")
        }

        fun beginPreview(filePath: String): Result<Unit> {
            return try {
                val json = nativeBeginPreview(filePath)
                parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "beginPreview failed", e)
                Result.failure(e)
            }
        }

        fun applyPreview(lutId: String, meteringMode: String, evOffset: Float): Result<String> {
            return try {
                val json = nativeApplyPreview(lutId, meteringMode, evOffset)
                parseResultWithUrl(json)
            } catch (e: Exception) {
                Log.e(TAG, "applyPreview failed", e)
                Result.failure(e)
            }
        }

        fun endPreview(): Result<Unit> {
            return try {
                val json = nativeEndPreview()
                parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "endPreview failed", e)
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

        private fun parseResult(json: String): Result<Unit> {
            val obj = JSONObject(json)
            if (obj.optBoolean("ok", false)) {
                return Result.success(Unit)
            }
            return Result.failure(Exception(obj.optString("error", "Unknown error")))
        }

        private fun parseResultWithUrl(json: String): Result<String> {
            val obj = JSONObject(json)
            if (obj.optBoolean("ok", false)) {
                return Result.success(obj.optString("url", ""))
            }
            return Result.failure(Exception(obj.optString("error", "Unknown error")))
        }

        @JvmStatic
        private external fun nativeBeginPreview(filePath: String): String
        @JvmStatic
        private external fun nativeApplyPreview(lutId: String, meteringMode: String, evOffset: Float): String
        @JvmStatic
        private external fun nativeEndPreview(): String
        @JvmStatic
        private external fun nativeGetPresets(): String
    }
}
