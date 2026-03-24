/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.util.Log
import android.webkit.JavascriptInterface
import android.os.SystemClock
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.MediaScannerHelper
import java.io.File
import kotlin.concurrent.thread

/**
 * 文件上传JavaScript Bridge
 * 接收来自WebView的file-uploaded事件，触发媒体扫描
 */
class FileUploadBridge(private val mainActivity: MainActivity) : BaseJsBridge(mainActivity) {
    companion object {
        private const val TAG = "FileUploadBridge"
        // Must match: src-tauri/src/platform/android.rs DEFAULT_STORAGE_PATH
        private const val DEFAULT_STORAGE_PATH = "/storage/emulated/0/DCIM/CameraFTP"
        private const val FILE_READY_TIMEOUT_MS = 8000L
        private const val FILE_READY_STABLE_MS = 500L
        private const val FILE_READY_POLL_MS = 200L
    }

    /**
     * 由JavaScript调用，处理文件上传事件
     * @param path 文件路径（可能是相对路径或绝对路径）
     */
    @JavascriptInterface
    fun onFileUploaded(path: String?) {
        Log.d(TAG, "onFileUploaded: path=$path")
        if (path.isNullOrEmpty()) {
            Log.w(TAG, "Received empty file path, skipping media scan")
            return
        }

        // Build full file path
        val fullPath = if (path.startsWith("/")) {
            path
        } else {
            "$DEFAULT_STORAGE_PATH/$path"
        }
        Log.d(TAG, "onFileUploaded: fullPath=$fullPath")

        // Trigger media scan to make photos appear in gallery
        thread(name = "media-scan-wait") {
            val file = File(fullPath)
            val ready = waitForFileReady(file)
            if (!ready) {
                Log.w(TAG, "File not ready after timeout, scanning anyway: $fullPath")
            }

            runOnUiThread {
                MediaScannerHelper.scanFileWithReset(mainActivity, fullPath)
            }
        }
    }

    private fun waitForFileReady(file: File): Boolean {
        val deadline = SystemClock.elapsedRealtime() + FILE_READY_TIMEOUT_MS
        var lastSize = -1L
        var lastModified = -1L
        var stableStart: Long? = null

        while (SystemClock.elapsedRealtime() < deadline) {
            if (!file.exists()) {
                Thread.sleep(FILE_READY_POLL_MS)
                continue
            }

            val size = file.length()
            val modified = file.lastModified()

            if (size > 0 && size == lastSize && modified == lastModified) {
                if (stableStart == null) {
                    stableStart = SystemClock.elapsedRealtime()
                }
                if (SystemClock.elapsedRealtime() - stableStart >= FILE_READY_STABLE_MS) {
                    return true
                }
            } else {
                stableStart = null
                lastSize = size
                lastModified = modified
            }

            Thread.sleep(FILE_READY_POLL_MS)
        }

        return false
    }
}
