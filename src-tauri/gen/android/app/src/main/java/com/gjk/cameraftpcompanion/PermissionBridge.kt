/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.storage.StorageManager
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
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Permission JavaScript Bridge
 * Provides permission checking and requesting functionality to the frontend
 */
class PermissionBridge(activity: Activity) : BaseJsBridge(activity) {
    companion object {
        private const val TAG = "PermissionBridge"
        // Request code for notification permission - shared with MainActivity
        const val REQUEST_POST_NOTIFICATIONS = 1001
        // Request code for storage permission request
        const val REQUEST_STORAGE_PERMISSIONS = 1002
        // Limits for ClipData to prevent Intent size issues
        private const val MAX_URIS_IN_CLIP_DATA = 100
        // Upper bound for waiting on the UI-thread startActivity outcome
        private const val AWAIT_UI_OUTCOME_MS = 2_000L

        /**
         * Get required permissions for MediaStore-based operations
         * Uses READ_MEDIA_IMAGES instead of MANAGE_EXTERNAL_STORAGE
         */
        fun get_required_permissions(): List<String> {
            return listOf(
                Manifest.permission.READ_MEDIA_IMAGES,
                Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
            )
        }

        fun build_app_permission_settings_intent(packageName: String): Intent {
            return Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.fromParts("package", packageName, null)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }

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
        val hasFullImageAccess = ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.READ_MEDIA_IMAGES
        ) == PackageManager.PERMISSION_GRANTED

        if (!hasFullImageAccess && hasPartialStoragePermission()) {
            Log.d(TAG, "checkStoragePermission: partial photo access only")
        }

        return hasFullImageAccess
    }

    private fun hasPartialStoragePermission(): Boolean {
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
        return ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.POST_NOTIFICATIONS
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Check battery optimization whitelist
     * Internal helper - not exposed to JavaScript
     */
    fun checkBatteryOptimization(): Boolean {
        val powerManager = activity.getSystemService(Context.POWER_SERVICE) as PowerManager
        return powerManager.isIgnoringBatteryOptimizations(activity.packageName)
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
        ActivityCompat.requestPermissions(
            activity,
            get_required_permissions().toTypedArray(),
            REQUEST_STORAGE_PERMISSIONS
        )
    }

    /**
     * Request notification permission
     */
    @JavascriptInterface
    fun requestNotificationPermission() {
        Log.d(TAG, "requestNotificationPermission: requesting notification permission")
        ActivityCompat.requestPermissions(
            activity,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            REQUEST_POST_NOTIFICATIONS
        )
    }

    /**
     * Request battery optimization whitelist - opens the settings page
     */
    @JavascriptInterface
    fun requestBatteryOptimization() {
        Log.d(TAG, "requestBatteryOptimization: requesting battery optimization whitelist")
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
                    Toast.makeText(activity, "无法打开链接: ${e.message}", Toast.LENGTH_SHORT).show()
                } catch (toastError: Exception) {
                    Log.e(TAG, "Failed to show toast", toastError)
                }
            }
        }
    }

    /**
     * Open image with external app, supporting browsing other images in the same directory.
     * Uses MediaStore URIs only.
     *
     * Semantics: the returned JSON reflects the outcome of the UI-thread
     * `startActivity`. `@JavascriptInterface` methods run on the WebView's
     * JavaBridge thread (not the main thread), so we may block on a short
     * bounded latch while the open runs. If called on the main thread,
     * [Activity.runOnUiThread] executes the block synchronously and the
     * latch is already counted down — no deadlock. On a rare latch timeout
     * (main thread blocked > 2 s) we conservatively report failure even
     * though the open may still complete later.
     *
     * @param path The MediaStore URI or file path to the image
     * @return JSON string with success status
     */
    @JavascriptInterface
    fun openImageWithChooser(path: String?): String {
        Log.d(TAG, "openImageWithChooser: path=$path")

        if (path.isNullOrEmpty()) {
            return failureJson("Empty path")
        }

        // Handle MediaStore URI directly
        if (path.startsWith("content://")) {
            val uri = Uri.parse(path)
            return runOnUiAwaitingOutcome { openWithMediaStoreUri(uri) }
        }

        // Non-content inputs must be resolved to MediaStore first.
        val resolvedUri = resolveToMediaStoreUri(path)
        if (resolvedUri == null) {
            Log.e(TAG, "openImageWithChooser: unable to resolve MediaStore URI from input: $path")
            runOnUiThread {
                Toast.makeText(activity, "无法打开图片", Toast.LENGTH_SHORT).show()
            }
            return failureJson("MediaStore URI not found")
        }

        return runOnUiAwaitingOutcome { openWithMediaStoreUri(resolvedUri) }
    }

    private fun failureJson(message: String): String {
        val result = JSONObject()
        result.put("success", false)
        result.put("message", message)
        return result.toString()
    }

    /**
     * Run [block] on the UI thread and wait up to [AWAIT_UI_OUTCOME_MS] for
     * its outcome so the JS return value reflects the actual result (the
     * old code returned `{"success":true}` before anything was attempted).
     */
    private fun runOnUiAwaitingOutcome(block: () -> Unit): String {
        val latch = CountDownLatch(1)
        val failure = AtomicReference<Exception?>(null)
        runOnUiThread {
            try {
                block()
            } catch (e: Exception) {
                Log.e(TAG, "openImageWithChooser: failed to open image", e)
                failure.set(e)
                try {
                    Toast.makeText(activity, "无法打开图片: ${e.message}", Toast.LENGTH_SHORT).show()
                } catch (toastError: Exception) {
                    Log.e(TAG, "Failed to show toast", toastError)
                }
            } finally {
                latch.countDown()
            }
        }

        val completed = try {
            latch.await(AWAIT_UI_OUTCOME_MS, TimeUnit.MILLISECONDS)
        } catch (e: InterruptedException) {
            Thread.currentThread().interrupt()
            false
        }

        val result = JSONObject()
        return when {
            failure.get() != null -> failureJson(failure.get()?.message ?: "open failed")
            !completed -> {
                Log.w(TAG, "openImageWithChooser: UI outcome not known within ${AWAIT_UI_OUTCOME_MS}ms")
                failureJson("open timed out")
            }
            else -> {
                result.put("success", true)
                result.toString()
            }
        }
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

        val storageManager = activity.getSystemService(Context.STORAGE_SERVICE) as StorageManager
        val storageRoot = storageManager.primaryStorageVolume.directory?.absolutePath
            ?: return null
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
