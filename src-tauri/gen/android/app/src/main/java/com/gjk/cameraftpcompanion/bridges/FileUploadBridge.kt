package com.gjk.cameraftpcompanion.bridges

import android.util.Log
import android.webkit.JavascriptInterface
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.MediaScannerHelper

/**
 * 文件上传JavaScript Bridge
 * 接收来自WebView的file-uploaded事件，触发媒体扫描
 */
class FileUploadBridge(private val mainActivity: MainActivity) : BaseJsBridge(mainActivity) {
    companion object {
        private const val TAG = "FileUploadBridge"
        // Must match: src-tauri/src/platform/android.rs DEFAULT_STORAGE_PATH
        private const val DEFAULT_STORAGE_PATH = "/storage/emulated/0/DCIM/CameraFTP"
    }

    /**
     * 由JavaScript调用，处理文件上传事件
     * @param path 文件路径（可能是相对路径或绝对路径）
     */
    @JavascriptInterface
    fun onFileUploaded(path: String?) {
        if (path.isNullOrEmpty()) {
            Log.w(TAG, "Received empty file path, skipping media scan")
            return
        }

        // 构建完整文件路径
        val fullPath = if (path.startsWith("/")) {
            path
        } else {
            "$DEFAULT_STORAGE_PATH/$path"
        }

        Log.i(TAG, "File uploaded: path=$fullPath")

        // 触发媒体扫描，让照片出现在相册中
        runOnUiThread {
            MediaScannerHelper.scanFile(mainActivity, fullPath)
        }
    }
}
