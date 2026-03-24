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
import android.content.ContentUris
import android.provider.MediaStore
import java.io.File
import java.io.FileOutputStream
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
        // Limits for ClipData to prevent Intent size issues
        private const val MAX_URIS_IN_CLIP_DATA = 100

        /**
         * Get required permissions for MediaStore-based operations
         * Uses READ_MEDIA_IMAGES instead of MANAGE_EXTERNAL_STORAGE
         */
        @JvmStatic
        fun get_required_permissions(): List<String> {
            return listOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
            )
        }

        @JvmStatic
        fun build_app_permission_settings_intent(packageName: String): Intent {
            return Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", packageName, null)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }

        @JvmStatic
        fun should_open_settings_for_storage_request(hasFullAccess: Boolean, hasPartialAccess: Boolean): Boolean {
            return !hasFullAccess && hasPartialAccess
        }
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
     * Check storage permission (READ_MEDIA_IMAGES for Android 13+)
     * Internal helper - not exposed to JavaScript
     */
    fun checkStoragePermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val hasFullImageAccess = ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.READ_MEDIA_IMAGES
            ) == PackageManager.PERMISSION_GRANTED

            if (hasFullImageAccess) {
                true
            } else {
                val hasSelectedPhotoAccess = ContextCompat.checkSelfPermission(
                    activity,
                    Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
                ) == PackageManager.PERMISSION_GRANTED
                if (hasSelectedPhotoAccess) {
                    Log.d(TAG, "checkStoragePermission: partial photo access only")
                }
                false
            }
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.READ_MEDIA_IMAGES
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            // For Android 11-12, still need WRITE_EXTERNAL_STORAGE
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.WRITE_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
        }
    }

    private fun hasPartialStoragePermission(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            return false
        }

        val hasFullImageAccess = ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.READ_MEDIA_IMAGES
        ) == PackageManager.PERMISSION_GRANTED
        val hasSelectedPhotoAccess = ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
        ) == PackageManager.PERMISSION_GRANTED
        return should_open_settings_for_storage_request(hasFullImageAccess, hasSelectedPhotoAccess)
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
     * Request storage permission.
     *
     * Partial access opens app settings directly, while denied access
     * still triggers runtime permission request.
     */
    @JavascriptInterface
    fun requestStoragePermission() {
        val hasFullAccess = checkStoragePermission()
        if (hasFullAccess) {
            Log.d(TAG, "requestStoragePermission: full storage permission already granted")
            return
        }

        if (hasPartialStoragePermission()) {
            Log.d(TAG, "requestStoragePermission: partial access, opening app permission settings")
            try {
                activity.startActivity(build_app_permission_settings_intent(activity.packageName))
            } catch (e: Exception) {
                Log.e(TAG, "requestStoragePermission: failed to open app permission settings", e)
            }
            return
        }

        Log.d(TAG, "requestStoragePermission: denied access, requesting runtime permissions")
        val permissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            get_required_permissions().toTypedArray()
        } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            arrayOf(Manifest.permission.READ_MEDIA_IMAGES)
        } else {
            arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE)
        }

        ActivityCompat.requestPermissions(
            activity,
            permissions,
            REQUEST_POST_NOTIFICATIONS
        )
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
            Log.d(TAG, "saveImageToGallery: no storage permission, requesting permission")
            // Show Android Toast before requesting permission
            runOnUiThread {
                Toast.makeText(activity, "需要存储权限才能保存图片，请授予权限", Toast.LENGTH_LONG).show()
            }
            // Request storage permission
            requestStoragePermission()
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
     * Open image with external app, supporting browsing other images in the same directory.
     * Uses MediaStore URIs only.
     * @param path The MediaStore URI or file path to the image
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

        // Handle MediaStore URI directly
        if (path.startsWith("content://")) {
            val uri = Uri.parse(path)
            thread(name = "open-image-uri") {
                try {
                    runOnUiThread {
                        openWithMediaStoreUri(uri)
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "openImageWithChooser: failed to open URI", e)
                    runOnUiThread {
                        Toast.makeText(activity, "无法打开图片: ${e.message}", Toast.LENGTH_SHORT).show()
                    }
                }
            }
            result.put("success", true)
            return result.toString()
        }

        // Non-content inputs must be resolved to MediaStore first.
        val resolvedUri = resolveToMediaStoreUri(path)
        if (resolvedUri == null) {
            Log.e(TAG, "openImageWithChooser: unable to resolve MediaStore URI from input: $path")
            runOnUiThread {
                Toast.makeText(activity, "无法打开图片", Toast.LENGTH_SHORT).show()
            }
            result.put("success", false)
            result.put("message", "MediaStore URI not found")
            return result.toString()
        }

        thread(name = "open-image") {
            try {
                runOnUiThread {
                    openWithMediaStoreUri(resolvedUri)
                }
            } catch (e: Exception) {
                Log.e(TAG, "openImageWithChooser: failed to open image", e)
                runOnUiThread {
                    Toast.makeText(activity, "无法打开图片", Toast.LENGTH_SHORT).show()
                }
            }
        }

        result.put("success", true)
        return result.toString()
    }

    /**
     * Open a single image using MediaStore URI directly
     * Used when the input is already a content:// URI
     */
    private fun openWithMediaStoreUri(uri: Uri) {
        // Query for other images in the same directory for browsing support
        val projection = arrayOf(MediaStore.Images.Media.RELATIVE_PATH)
        var relativePath: String? = null

        activity.contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) {
                relativePath = cursor.getString(cursor.getColumnIndexOrThrow(MediaStore.Images.Media.RELATIVE_PATH))
            }
        }

        // Build list of URIs in the same directory for swipe browsing
        val windowUris = mutableListOf<Uri>()
        windowUris.add(uri) // Add target first

        if (!relativePath.isNullOrEmpty()) {
            // Query other images in same directory
            val windowProjection = arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DATE_MODIFIED
            )
            val selection = "${MediaStore.Images.Media.RELATIVE_PATH} = ?"
            val selectionArgs = arrayOf(relativePath)

            activity.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                windowProjection,
                selection,
                selectionArgs,
                "${MediaStore.Images.Media.DATE_MODIFIED} DESC"
            )?.use { cursor ->
                val idColumn = cursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                while (cursor.moveToNext() && windowUris.size < MAX_URIS_IN_CLIP_DATA) {
                    val id = cursor.getLong(idColumn)
                    val contentUri = ContentUris.withAppendedId(
                        MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                        id
                    )
                    if (contentUri != uri) {
                        windowUris.add(contentUri)
                    }
                }
            }
        }

        // Build ClipData with all URIs for browsing support
        val clipData = ClipData.newRawUri(null, windowUris.first())
        for (i in 1 until windowUris.size) {
            clipData.addItem(ClipData.Item(windowUris[i]))
        }

        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "image/*")
            setClipData(clipData)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        activity.startActivity(intent)
        Log.d(TAG, "openWithMediaStoreUri: opened with ${windowUris.size} URIs via MediaStore")
    }

    private fun resolveToMediaStoreUri(path: String): Uri? {
        val imageFile = File(path)
        if (!imageFile.exists()) {
            return null
        }

        val storageRoot = Environment.getExternalStorageDirectory().absolutePath
        val absolutePath = imageFile.absolutePath
        if (!absolutePath.startsWith("$storageRoot/")) {
            return null
        }

        val relativeToStorageRoot = absolutePath.removePrefix("$storageRoot/")
        val displayName = imageFile.name
        val parentRelativePath = relativeToStorageRoot.substringBeforeLast("/", "")
        val mediaRelativePath = if (parentRelativePath.isEmpty()) "" else "$parentRelativePath/"

        val projection = arrayOf(MediaStore.Images.Media._ID)
        val selection = "${MediaStore.Images.Media.RELATIVE_PATH} = ? AND ${MediaStore.Images.Media.DISPLAY_NAME} = ?"
        val selectionArgs = arrayOf(mediaRelativePath, displayName)

        activity.contentResolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            projection,
            selection,
            selectionArgs,
            "${MediaStore.Images.Media.DATE_ADDED} DESC"
        )?.use { cursor ->
            if (cursor.moveToFirst()) {
                val id = cursor.getLong(cursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID))
                return ContentUris.withAppendedId(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, id)
            }
        }

        return null
    }
}
