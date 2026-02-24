package com.gjk.cameraftpcompanion

import android.Manifest
import android.app.Activity
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
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import org.json.JSONObject

/**
 * Permission JavaScript Bridge
 * Provides permission checking and requesting functionality to the frontend
 */
class PermissionBridge(private val activity: Activity) {
    companion object {
        private const val TAG = "PermissionBridge"
        private const val REQUEST_MANAGE_STORAGE = 1003
        private const val REQUEST_POST_NOTIFICATIONS = 1002
        private const val REQUEST_IGNORE_BATTERY_OPTIMIZATIONS = 1001
    }

    /**
     * Check if all required permissions are granted
     * Returns JSON string with permission status
     */
    @JavascriptInterface
    fun checkAllPermissions(): String {
        val storageGranted = checkStoragePermission()
        val notificationGranted = checkNotificationPermission()
        val batteryOptimizationGranted = checkBatteryOptimization()
        
        // Use JSONObject for proper formatting
        val json = JSONObject()
        json.put("storage", storageGranted)
        json.put("notification", notificationGranted)
        json.put("batteryOptimization", batteryOptimizationGranted)
        
        val result = json.toString()
        Log.d(TAG, "checkAllPermissions: $result")
        return result
    }

    /**
     * Check storage permission (MANAGE_EXTERNAL_STORAGE for Android 11+)
     */
    @JavascriptInterface
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
     */
    @JavascriptInterface
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
     */
    @JavascriptInterface
    fun checkBatteryOptimization(): Boolean {
        val result = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val powerManager = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
            val isIgnoring = powerManager.isIgnoringBatteryOptimizations(activity.packageName)
            Log.d(TAG, "checkBatteryOptimization: isIgnoring=$isIgnoring")
            isIgnoring
        } else {
            Log.d(TAG, "checkBatteryOptimization: not required (API < 23)")
            true // Not required before Android 6
        }
        return result
    }

    /**
     * Request storage permission
     */
    @JavascriptInterface
    fun requestStoragePermission() {
        Log.d(TAG, "Requesting storage permission")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            try {
                val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                    data = Uri.parse("package:${activity.packageName}")
                }
                activity.startActivity(intent)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to open storage settings", e)
                // Fallback to general settings
                val intent = Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)
                activity.startActivity(intent)
            }
        } else {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(
                    Manifest.permission.READ_EXTERNAL_STORAGE,
                    Manifest.permission.WRITE_EXTERNAL_STORAGE
                ),
                REQUEST_MANAGE_STORAGE
            )
        }
    }

    /**
     * Request notification permission
     */
    @JavascriptInterface
    fun requestNotificationPermission() {
        Log.d(TAG, "Requesting notification permission")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                REQUEST_POST_NOTIFICATIONS
            )
        }
    }

    /**
     * Request battery optimization whitelist
     */
    @JavascriptInterface
    fun requestBatteryOptimization() {
        Log.d(TAG, "Requesting battery optimization whitelist")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val powerManager = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
            if (!powerManager.isIgnoringBatteryOptimizations(activity.packageName)) {
                try {
                    val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                        data = Uri.parse("package:${activity.packageName}")
                    }
                    // Use startActivityForResult so we can detect when user returns
                    activity.startActivityForResult(intent, REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to open battery optimization settings", e)
                }
            }
        }
    }

    /**
     * Open app settings page
     */
    @JavascriptInterface
    fun openAppSettings() {
        Log.d(TAG, "Opening app settings")
        try {
            val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.parse("package:${activity.packageName}")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            activity.startActivity(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to open app settings", e)
        }
    }
}
