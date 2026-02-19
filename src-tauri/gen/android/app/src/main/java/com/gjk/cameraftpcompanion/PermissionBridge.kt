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
import java.io.File
import java.io.FileOutputStream

/**
 * Permission JavaScript Bridge
 * Provides permission checking and requesting functionality to the frontend
 */
class PermissionBridge(activity: MainActivity) : BaseJsBridge(activity) {
    companion object {
        private const val TAG = "PermissionBridge"
        // Request code for notification permission - shared with MainActivity
        const val REQUEST_POST_NOTIFICATIONS = 1001
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
}
