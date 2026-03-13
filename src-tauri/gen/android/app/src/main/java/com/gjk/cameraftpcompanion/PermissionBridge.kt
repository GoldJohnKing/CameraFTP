/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import android.webkit.JavascriptInterface
import android.widget.Toast
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import com.gjk.cameraftpcompanion.bridges.BaseJsBridge
import org.json.JSONObject
import android.content.ClipData
import android.database.Cursor
import android.provider.MediaStore
import java.io.File
import java.io.FileOutputStream
import java.util.Locale
import kotlin.concurrent.thread

/**
 * Permission JavaScript Bridge
 * Provides permission checking and requesting functionality to the frontend
 */
class PermissionBridge(activity: MainActivity) : BaseJsBridge(activity) {
    companion object {
        private const val TAG = "PermissionBridge"
        // Request code for notification permission - shared with MainActivity
        const val REQUEST_POST_NOTIFICATIONS = 1001
        private const val MEDIASTORE_WAIT_TIMEOUT_MS = 2000L
        private const val MEDIASTORE_WAIT_POLL_MS = 150L
    }

    /**
     * Check if all required permissions are granted
     * Returns JSON string with permission status
     */
    @JavascriptInterface
    fun checkAllPermissions(): String {
        Log.d(TAG, "checkAllPermissions: checking all permissions")
        val storageGranted = checkStoragePermission()
        val notificationGranted = checkNotificationPermission()
        val batteryOptimizationGranted = checkBatteryOptimization()

        // Use JSONObject for proper formatting
        val json = JSONObject()
        json.put("storage", storageGranted)
        json.put("notification", notificationGranted)
        json.put("batteryOptimization", batteryOptimizationGranted)

        Log.d(TAG, "checkAllPermissions: storage=$storageGranted, notification=$notificationGranted, batteryOptimization=$batteryOptimizationGranted")
        return json.toString()
    }

    /**
     * Check storage permission (MANAGE_EXTERNAL_STORAGE for Android 11+)
     * Internal helper - not exposed to JavaScript
     */
    fun checkStoragePermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.WRITE_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
        }
    }

    /**
     * Check notification permission (Android 13+)
     * Internal helper - not exposed to JavaScript
     */
    fun checkNotificationPermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.POST_NOTIFICATIONS
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            true // Not required before Android 13
        }
    }

    /**
     * Check battery optimization whitelist
     * Internal helper - not exposed to JavaScript
     */
    fun checkBatteryOptimization(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val powerManager = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
            powerManager.isIgnoringBatteryOptimizations(activity.packageName)
        } else {
            true // Not required before Android 6
        }
    }

    /**
     * Request storage permission - opens the manage storage settings page
     */
    @JavascriptInterface
    fun requestStoragePermission() {
        Log.d(TAG, "requestStoragePermission: opening storage settings")
        // Delegate to StorageHelper to avoid code duplication
        StorageHelper.openManageStorageSettings(activity)
    }

    /**
     * Request notification permission
     */
    @JavascriptInterface
    fun requestNotificationPermission() {
        Log.d(TAG, "requestNotificationPermission: requesting notification permission")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                REQUEST_POST_NOTIFICATIONS
            )
        }
    }

    /**
     * Request battery optimization whitelist - opens the settings page
     */
    @JavascriptInterface
    fun requestBatteryOptimization() {
        Log.d(TAG, "requestBatteryOptimization: requesting battery optimization whitelist")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val powerManager = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
            if (!powerManager.isIgnoringBatteryOptimizations(activity.packageName)) {
                try {
                    val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                        data = Uri.parse("package:${activity.packageName}")
                    }
                    activity.startActivity(intent)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to open battery optimization settings", e)
                }
            } else {
                Log.d(TAG, "requestBatteryOptimization: already whitelisted")
            }
        }
    }

    /**
     * Open external link in default browser
     * @param url The URL to open
     */
    @JavascriptInterface
    fun openExternalLink(url: String?) {
        Log.d(TAG, "openExternalLink called: url=$url, thread=${Thread.currentThread().name}")
        if (url.isNullOrEmpty()) {
            Log.w(TAG, "openExternalLink: empty URL provided")
            return
        }
        runOnUiThread {
            try {
                val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                Log.d(TAG, "openExternalLink: starting activity with intent $intent")
                activity.startActivity(intent)
                Log.d(TAG, "openExternalLink: successfully opened $url")
            } catch (e: Exception) {
                Log.e(TAG, "openExternalLink: failed to open URL", e)
                // Try to show a toast or handle the error
                try {
                    android.widget.Toast.makeText(activity, "无法打开链接: ${e.message}", android.widget.Toast.LENGTH_SHORT).show()
                } catch (toastError: Exception) {
                    Log.e(TAG, "Failed to show toast", toastError)
                }
            }
        }
    }

    /**
     * Save asset image to gallery (Pictures directory)
     * @param assetPath The path to the asset image (e.g., "wechat.png")
     * @return JSON string with success status and message
     */
    @JavascriptInterface
    fun saveImageToGallery(assetPath: String?): String {
        Log.d(TAG, "saveImageToGallery: assetPath=$assetPath")
        
        val result = JSONObject()
        
        if (assetPath.isNullOrEmpty()) {
            result.put("success", false)
            result.put("message", "Empty asset path")
            return result.toString()
        }
        
        // Check storage permission first
        if (!checkStoragePermission()) {
            Log.d(TAG, "saveImageToGallery: no storage permission, opening settings")
            // Show Android Toast before opening settings (won't be covered by new activity)
            runOnUiThread {
                Toast.makeText(activity, "需要存储权限才能保存图片，请授予权限", Toast.LENGTH_LONG).show()
            }
            // Open storage permission settings
            StorageHelper.openManageStorageSettings(activity)
            result.put("success", false)
            result.put("reason", "permission_denied")
            result.put("message", "Storage permission required")
            return result.toString()
        }
        
        return try {
            // Create destination file in Pictures directory
            val picturesDir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES)
            val appDir = File(picturesDir, "CameraFTP")
            if (!appDir.exists()) {
                appDir.mkdirs()
            }
            
            val destFile = File(appDir, assetPath)
            
            // Copy from assets to destination
            activity.assets.open(assetPath).use { input ->
                FileOutputStream(destFile).use { output ->
                    input.copyTo(output)
                }
            }
            
            // Scan the file to make it appear in gallery
            MediaScannerHelper.scanFile(activity, destFile.absolutePath)
            
            Log.d(TAG, "saveImageToGallery: successfully saved to ${destFile.absolutePath}")
            result.put("success", true)
            result.put("message", "Image saved to gallery")
            result.toString()
        } catch (e: Exception) {
            Log.e(TAG, "saveImageToGallery: failed to save image", e)
            result.put("success", false)
            result.put("message", e.message ?: "Unknown error")
            result.toString()
        }
    }

    /**
     * Open image with external app, supporting browsing other images in the same directory
     * Uses MediaStore URIs for best compatibility with system galleries
     * Falls back to FileProvider URIs if MediaStore has no results
     * @param path The absolute path to the image file
     * @return JSON string with success status
     */
    @JavascriptInterface
    fun openImageWithChooser(path: String?): String {
        Log.d(TAG, "openImageWithChooser: path=$path")

        val result = JSONObject()

        if (path.isNullOrEmpty()) {
            result.put("success", false)
            result.put("message", "Empty path")
            return result.toString()
        }

        val imageFile = File(path)
        if (!imageFile.exists()) {
            Log.e(TAG, "openImageWithChooser: file does not exist: $path")
            result.put("success", false)
            result.put("message", "File does not exist")
            return result.toString()
        }

        thread(name = "open-image") {
            try {
                val directoryPath = imageFile.parentFile?.absolutePath ?: ""
                val mediaStoreEntries = queryImagesFromMediaStore(directoryPath)
                val targetEntry = mediaStoreEntries[imageFile.absolutePath]

                if (targetEntry != null && isMediaStoreEntryFresh(targetEntry, imageFile)) {
                    runOnUiThread {
                        openWithMediaStore(imageFile, mediaStoreEntries)
                    }
                    return@thread
                }

                if (targetEntry != null) {
                    Log.w(TAG, "MediaStore entry is stale, waiting for refresh: ${imageFile.absolutePath}")
                }

                MediaScannerHelper.scanFile(activity, imageFile.absolutePath)
                val refreshedEntries = waitForFreshMediaStoreEntry(directoryPath, imageFile)

                runOnUiThread {
                    if (refreshedEntries != null) {
                        openWithMediaStore(imageFile, refreshedEntries)
                    } else {
                        openWithFileProvider(imageFile)
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "openImageWithChooser: failed to open image", e)
                runOnUiThread {
                    Toast.makeText(activity, "无法打开图片: ${e.message}", Toast.LENGTH_SHORT).show()
                }
            }
        }

        result.put("success", true)
        return result.toString()
    }

    /**
     * Query images from MediaStore in the specified directory
     * Returns a map of file path to content URI
     */
    private data class MediaStoreEntry(
        val uri: Uri,
        val dateModifiedSeconds: Long,
        val sizeBytes: Long,
        val dateAddedSeconds: Long,
    )

    private fun queryImagesFromMediaStore(directoryPath: String): Map<String, MediaStoreEntry> {
        val uriMap = mutableMapOf<String, MediaStoreEntry>()
        
        if (directoryPath.isEmpty()) return uriMap

        val projection = arrayOf(
            MediaStore.Images.Media._ID,
            MediaStore.Images.Media.DATA,
            MediaStore.Images.Media.DATE_MODIFIED,
            MediaStore.Images.Media.SIZE,
            MediaStore.Images.Media.DATE_ADDED
        )

        // Query images in the directory (excluding subdirectories)
        val selection = "${MediaStore.Images.Media.DATA} LIKE ? AND ${MediaStore.Images.Media.DATA} NOT LIKE ?"
        val selectionArgs = arrayOf(
            "$directoryPath/%",
            "$directoryPath/%/%"
        )

        val cursor: Cursor? = activity.contentResolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            projection,
            selection,
            selectionArgs,
            "${MediaStore.Images.Media.DATE_ADDED} DESC"
        )

        cursor?.use {
            val idColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
            val dataColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATA)
            val modifiedColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)
            val sizeColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.SIZE)
            val addedColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_ADDED)

            while (it.moveToNext()) {
                val id = it.getLong(idColumn)
                val filePath = it.getString(dataColumn)
                val dateModified = it.getLong(modifiedColumn)
                val sizeBytes = it.getLong(sizeColumn)
                val dateAdded = it.getLong(addedColumn)
                val contentUri = Uri.withAppendedPath(
                    MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                    id.toString()
                )
                val entry = MediaStoreEntry(contentUri, dateModified, sizeBytes, dateAdded)
                val existing = uriMap[filePath]
                uriMap[filePath] = selectNewestEntry(existing, entry)
            }
        }

        Log.d(TAG, "queryImagesFromMediaStore: found ${uriMap.size} images in $directoryPath")
        return uriMap
    }

    private fun selectNewestEntry(
        existing: MediaStoreEntry?,
        candidate: MediaStoreEntry,
    ): MediaStoreEntry {
        if (existing == null) return candidate

        val existingScore = maxOf(existing.dateModifiedSeconds, existing.dateAddedSeconds)
        val candidateScore = maxOf(candidate.dateModifiedSeconds, candidate.dateAddedSeconds)

        if (candidateScore != existingScore) {
            return if (candidateScore > existingScore) candidate else existing
        }

        if (candidate.sizeBytes != existing.sizeBytes) {
            return if (candidate.sizeBytes > existing.sizeBytes) candidate else existing
        }

        return existing
    }

    private fun isMediaStoreEntryFresh(entry: MediaStoreEntry, file: File): Boolean {
        val fileSize = file.length()
        val fileModifiedSeconds = file.lastModified() / 1000

        if (entry.sizeBytes <= 0L || entry.dateModifiedSeconds <= 0L) {
            return false
        }

        if (entry.sizeBytes != fileSize) {
            return false
        }

        return entry.dateModifiedSeconds >= fileModifiedSeconds
    }

    private fun waitForFreshMediaStoreEntry(
        directoryPath: String,
        file: File,
    ): Map<String, MediaStoreEntry>? {
        val deadline = android.os.SystemClock.elapsedRealtime() + MEDIASTORE_WAIT_TIMEOUT_MS

        while (android.os.SystemClock.elapsedRealtime() < deadline) {
            val entries = queryImagesFromMediaStore(directoryPath)
            val targetEntry = entries[file.absolutePath]
            if (targetEntry != null && isMediaStoreEntryFresh(targetEntry, file)) {
                return entries
            }
            Thread.sleep(MEDIASTORE_WAIT_POLL_MS)
        }

        return null
    }

    /**
     * Open image using MediaStore content URIs (preferred method)
     */
    private fun openWithMediaStore(targetFile: File, uriMap: Map<String, MediaStoreEntry>) {
        val targetPath = targetFile.absolutePath
        val targetUri = uriMap[targetPath]!!.uri

        // Build ClipData with all URIs for browsing support
        val allUris = uriMap.values.map { it.uri }
        val clipData = ClipData.newRawUri(null, targetUri)
        
        // Limit to prevent Intent size issues
        val maxUris = 100
        var addedCount = 0
        for (uri in allUris) {
            if (uri != targetUri && addedCount < maxUris) {
                clipData.addItem(ClipData.Item(uri))
                addedCount++
            }
        }

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(targetUri, "image/*")
            setClipData(clipData)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        activity.startActivity(intent)
        Log.d(TAG, "openWithMediaStore: opened with ${allUris.size} URIs via MediaStore")
    }

    /**
     * Open image using FileProvider URIs (fallback method)
     */
    private fun openWithFileProvider(targetFile: File) {
        val parentDir = targetFile.parentFile

        // Get all image files in directory
        val imageFiles = if (parentDir != null) {
            val imageExtensions = setOf("jpg", "jpeg", "png", "gif", "bmp", "webp", "heic", "heif")
            parentDir.listFiles { file ->
                file.isFile && file.extension.lowercase(Locale.getDefault()) in imageExtensions
            }?.sortedByDescending { it.lastModified() } ?: emptyList()
        } else {
            emptyList()
        }

        // Limit to prevent Intent size issues
        val maxFiles = 50
        val limitedFiles = imageFiles.take(maxFiles)

        val targetUri = getUriForFile(targetFile)

        if (limitedFiles.size <= 1) {
            // Single image - simple intent
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(targetUri, "image/*")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(intent)
            Log.d(TAG, "openWithFileProvider: opened single image")
            return
        }

        // Multiple images - use ClipData for browsing support
        val clipData = ClipData.newRawUri(null, targetUri)
        for (file in limitedFiles) {
            if (file != targetFile) {
                clipData.addItem(ClipData.Item(getUriForFile(file)))
            }
        }

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(targetUri, "image/*")
            setClipData(clipData)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        activity.startActivity(intent)
        Log.d(TAG, "openWithFileProvider: opened with ${limitedFiles.size} images via FileProvider")
    }

    /**
     * Get URI for a file using FileProvider on Android N+
     */
    private fun getUriForFile(file: File): Uri {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            androidx.core.content.FileProvider.getUriForFile(
                activity,
                "${activity.packageName}.fileprovider",
                file
            )
        } else {
            Uri.fromFile(file)
        }
    }
}
