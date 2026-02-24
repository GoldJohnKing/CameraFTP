package com.gjk.cameraftpcompanion

import android.media.MediaScannerConnection
import android.net.Uri
import android.util.Log
import java.io.File

/**
 * Android 媒体扫描辅助类
 * 用于在文件写入后通知系统媒体扫描器更新媒体数据库
 */
object MediaScannerHelper {

    private const val TAG = "MediaScannerHelper"

    /**
     * 扫描单个文件
     * @param activity MainActivity实例
     * @param filePath 文件的完整路径
     */
    fun scanFile(activity: MainActivity, filePath: String) {
        val file = File(filePath)
        if (!file.exists()) {
            Log.w(TAG, "File does not exist, skipping scan: $filePath")
            return
        }

        // 获取文件的MIME类型
        val mimeType = getMimeType(filePath)
        Log.d(TAG, "Scanning file: $filePath, MIME type: $mimeType")

        MediaScannerConnection.scanFile(
            activity,
            arrayOf(filePath),
            arrayOf(mimeType)
        ) { path: String?, uri: Uri? ->
            if (uri != null) {
                Log.i(TAG, "Media scan completed for: $path, URI: $uri")
            } else {
                Log.w(TAG, "Media scan failed for: $path")
            }
        }
    }

    /**
     * 扫描多个文件
     * @param activity MainActivity实例
     * @param filePaths 文件路径列表
     */
    fun scanFiles(activity: MainActivity, filePaths: List<String>) {
        if (filePaths.isEmpty()) {
            return
        }

        // 过滤掉不存在的文件
        val existingFiles = filePaths.filter { File(it).exists() }
        if (existingFiles.isEmpty()) {
            Log.w(TAG, "No existing files to scan")
            return
        }

        val mimeTypes = existingFiles.map { getMimeType(it) }.toTypedArray()

        Log.d(TAG, "Scanning ${existingFiles.size} files")

        MediaScannerConnection.scanFile(
            activity,
            existingFiles.toTypedArray(),
            mimeTypes
        ) { path: String?, uri: Uri? ->
            if (uri != null) {
                Log.d(TAG, "Media scan completed for: $path")
            } else {
                Log.w(TAG, "Media scan failed for: $path")
            }
        }
    }

    /**
     * 扫描整个目录
     * 递归扫描目录下的所有媒体文件
     * @param activity MainActivity实例
     * @param directoryPath 目录路径
     */
    fun scanDirectory(activity: MainActivity, directoryPath: String) {
        val dir = File(directoryPath)
        if (!dir.exists() || !dir.isDirectory) {
            Log.w(TAG, "Directory does not exist or is not a directory: $directoryPath")
            return
        }

        // 收集所有媒体文件
        val mediaFiles = mutableListOf<String>()
        collectMediaFiles(dir, mediaFiles)

        if (mediaFiles.isNotEmpty()) {
            Log.i(TAG, "Found ${mediaFiles.size} media files to scan in: $directoryPath")
            scanFiles(activity, mediaFiles)
        } else {
            Log.d(TAG, "No media files found in: $directoryPath")
        }
    }

    /**
     * 递归收集媒体文件
     */
    private fun collectMediaFiles(dir: File, mediaFiles: MutableList<String>) {
        dir.listFiles()?.forEach { file ->
            when {
                file.isDirectory -> collectMediaFiles(file, mediaFiles)
                isMediaFile(file.name) -> mediaFiles.add(file.absolutePath)
            }
        }
    }

    /**
     * 判断文件是否为媒体文件
     */
    private fun isMediaFile(fileName: String): Boolean {
        val lower = fileName.lowercase()
        return lower.endsWith(".jpg") ||
                lower.endsWith(".jpeg") ||
                lower.endsWith(".png") ||
                lower.endsWith(".gif") ||
                lower.endsWith(".bmp") ||
                lower.endsWith(".webp") ||
                lower.endsWith(".mp4") ||
                lower.endsWith(".avi") ||
                lower.endsWith(".mov") ||
                lower.endsWith(".mkv") ||
                lower.endsWith(".heic") ||
                lower.endsWith(".heif") ||
                lower.endsWith(".raw") ||
                lower.endsWith(".cr2") ||
                lower.endsWith(".nef") ||
                lower.endsWith(".arw") ||
                lower.endsWith(".dng")
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
