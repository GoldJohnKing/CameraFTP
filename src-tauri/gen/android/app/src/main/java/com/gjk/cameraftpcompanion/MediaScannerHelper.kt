/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.media.MediaScannerConnection
import android.net.Uri
import android.util.Log
import android.provider.MediaStore
import java.io.File

/**
 * Android 媒体扫描辅助类
 * 用于在文件写入后通知系统媒体扫描器更新媒体数据库
 */
object MediaScannerHelper {

    private const val TAG = "MediaScannerHelper"

    /**
     * 扫描单个文件
     * @param activity Activity实例
     * @param filePath 文件的完整路径
     */
    fun scanFile(activity: Activity, filePath: String) {
        val file = File(filePath)
        if (!file.exists()) {
            Log.w(TAG, "File does not exist, skipping scan: $filePath")
            return
        }

        // 获取文件的MIME类型
        val mimeType = getMimeType(filePath)

        MediaScannerConnection.scanFile(
            activity,
            arrayOf(filePath),
            arrayOf(mimeType)
        ) { path: String?, uri: Uri? ->
            if (uri == null) {
                Log.w(TAG, "Media scan failed for: $path")
            }
        }
    }

    fun scanFileWithReset(activity: Activity, filePath: String) {
        val rowsDeleted = try {
            activity.contentResolver.delete(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                "${MediaStore.Images.Media.DATA}=?",
                arrayOf(filePath)
            )
        } catch (e: Exception) {
            Log.w(TAG, "Failed to clear MediaStore entry for: $filePath", e)
            0
        }

        if (rowsDeleted > 0) {
            Log.d(TAG, "Cleared $rowsDeleted stale MediaStore rows for: $filePath")
        }

        scanFile(activity, filePath)
    }

    /**
     * 根据文件扩展名获取MIME类型
     */
    private fun getMimeType(filePath: String): String {
        val lower = filePath.lowercase()
        return when {
            lower.endsWith(".jpg") || lower.endsWith(".jpeg") -> "image/jpeg"
            lower.endsWith(".png") -> "image/png"
            lower.endsWith(".gif") -> "image/gif"
            lower.endsWith(".bmp") -> "image/bmp"
            lower.endsWith(".webp") -> "image/webp"
            lower.endsWith(".mp4") -> "video/mp4"
            lower.endsWith(".avi") -> "video/x-msvideo"
            lower.endsWith(".mov") -> "video/quicktime"
            lower.endsWith(".mkv") -> "video/x-matroska"
            lower.endsWith(".heic") || lower.endsWith(".heif") -> "image/heic"
            lower.endsWith(".raw") || 
            lower.endsWith(".cr2") || 
            lower.endsWith(".nef") || 
            lower.endsWith(".arw") || 
            lower.endsWith(".dng") -> "image/x-dcraw"
            else -> "*/*"
        }
    }
}
