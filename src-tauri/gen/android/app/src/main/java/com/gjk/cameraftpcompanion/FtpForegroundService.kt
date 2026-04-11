/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.IBinder
import android.os.Build
import android.os.PowerManager
import android.util.Log
import androidx.annotation.StringRes
import androidx.annotation.RequiresApi
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import org.json.JSONObject

class FtpForegroundService : Service() {
    @Volatile
    private var isInForeground = false

    companion object {
        const val TAG = "FtpForegroundService"
        const val NOTIFICATION_ID = 1001
        const val CHANNEL_ID = "ftp_service_channel"

        // Actions
        const val ACTION_START = "com.gjk.cameraftpcompanion.START_SERVICE"
        const val ACTION_STOP = "com.gjk.cameraftpcompanion.STOP_SERVICE"
        // Singleton instance for MainActivity to access
        @Volatile
        private var instance: FtpForegroundService? = null

        fun getInstance(): FtpForegroundService? {
            return instance
        }
    }

    // State (accessed from multiple threads - use synchronized access)
    @Volatile
    private var serverStats: JSONObject? = null
    @Volatile
    private var connectedClients = 0
    private val stateLock = Any()

    // Locks
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        Log.d(TAG, "onCreate: initializing service")
        super.onCreate()
        instance = this
        refreshFromCoordinator()
        createNotificationChannel()
        acquireLocks()

        // Note: startForeground() is called in onStartCommand() to satisfy Android's
        // 5-second requirement after startForegroundService().
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: action=${intent?.action}, startId=$startId")

        if (intent?.action == ACTION_STOP) {
            stopForegroundServiceNow("explicit stop action")
            return START_NOT_STICKY
        }

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        if (!snapshot.isRunning) {
            Log.d(TAG, "onStartCommand: ignoring start because coordinator is stopped")
            stopForegroundServiceNow("stale start while stopped")
            return START_NOT_STICKY
        }

        restoreStateFromSnapshot(snapshot)

        // CRITICAL: Must call startForeground() within 5 seconds of startForegroundService()
        // Otherwise, Android will throw ForegroundServiceDidNotStartInTimeException and crash the app
        // Service is only started when server is running, so always show running notification
        val notification = buildNotification()
        startForegroundWithType(notification)

        return START_NOT_STICKY
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up service")
        instance = null
        isInForeground = false
        releaseLocks()
        super.onDestroy()
    }

    @RequiresApi(Build.VERSION_CODES.VANILLA_ICE_CREAM)
    override fun onTimeout(startId: Int, fgsType: Int) {
        Log.w(TAG, "onTimeout(startId, fgsType): startId=$startId, fgsType=$fgsType")
        AndroidServiceStateCoordinator.clearState()
        stopForegroundServiceNow("fgs timeout startId=$startId type=$fgsType")
    }

    override fun onTimeout(startId: Int) {
        Log.w(TAG, "onTimeout(startId): startId=$startId")
        AndroidServiceStateCoordinator.clearState()
        stopForegroundServiceNow("fgs timeout startId=$startId")
    }

    /**
     * Start foreground service with connectedDevice type.
     * Required for MIUI and other OEM ROMs to recognize service type.
     */
    private fun startForegroundWithType(notification: Notification) {
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
        )
        isInForeground = true
        Log.d(TAG, "startForegroundWithType: started with FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE")
    }

    private fun stopForegroundServiceNow(reason: String) {
        Log.d(TAG, "stopForegroundServiceNow: $reason")
        applyServerState(null, 0)
        if (isInForeground) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            isInForeground = false
        }
        stopSelf()
    }

    /**
     * Create notification channel for foreground service notification.
     */
    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            getStringOrFallback(R.string.ftp_service_channel_name, "FTP service"),
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = getStringOrFallback(
                R.string.ftp_service_channel_description,
                "Keeps FTP transfers running in the foreground",
            )
            setShowBadge(false)
            lockscreenVisibility = Notification.VISIBILITY_PUBLIC
        }

        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.createNotificationChannel(channel)
        Log.d(TAG, "createNotificationChannel: created notification channel")
    }

    /**
     * Acquire WakeLock and WifiLock to keep device running
     */
    private fun acquireLocks() {
        Log.d(TAG, "acquireLocks: acquiring wake lock and wifi lock")

        // Acquire partial wake lock to keep CPU running
        // No timeout - service lifecycle manages release via onDestroy()
        runCatching {
            val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
            val nextWakeLock = powerManager.newWakeLock(
                PowerManager.PARTIAL_WAKE_LOCK,
                "FtpForegroundService::WakeLock"
            )
            wakeLock = nextWakeLock
            nextWakeLock.acquire() // Indefinite - released when service stops
        }.onFailure { error ->
            Log.e(TAG, "Failed to acquire wake lock", error)
            wakeLock = null
        }

        // Acquire WiFi lock to keep WiFi connection alive
        runCatching {
            val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            @Suppress("DEPRECATION")
            val nextWifiLock = wifiManager.createWifiLock(
                WifiManager.WIFI_MODE_FULL_HIGH_PERF,
                "FtpForegroundService::WifiLock"
            )
            wifiLock = nextWifiLock
            nextWifiLock.acquire()
        }.onFailure { error ->
            Log.e(TAG, "Failed to acquire wifi lock", error)
            wifiLock = null
        }
    }

    /**
     * Release all locks
     */
    private fun releaseLocks() {
        Log.d(TAG, "releaseLocks: releasing wake lock and wifi lock")

        wakeLock?.let {
            runCatching {
                if (it.isHeld) {
                    it.release()
                }
            }.onFailure { error ->
                Log.e(TAG, "Failed to release wake lock", error)
            }
        }
        wakeLock = null

        wifiLock?.let {
            runCatching {
                if (it.isHeld) {
                    it.release()
                }
            }.onFailure { error ->
                Log.e(TAG, "Failed to release wifi lock", error)
            }
        }
        wifiLock = null
    }

    /**
     * Build notification for running server state.
     * Shows green icon with connection stats.
     */
    private fun buildNotification(): Notification {
        // Single state: server running (green icon)
        val iconRes = R.drawable.tray_active

        val title = getStringOrFallback(R.string.notification_title_running, "FTP server running")
        val content = buildStatusContent()

        // Intent to open MainActivity when tapped
        // Fallback to explicit intent if package manager returns null
        val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            ?: Intent(this, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(iconRes)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    /**
     * Build status content: 连接状态 | 已接收图片数 | 已接收图片总大小
     * Note: This is only called when server is running
     * Thread-safe: reads state under lock.
     */
    private fun buildStatusContent(): String {
        val (stats, clients) = synchronized(stateLock) {
            Pair(serverStats, connectedClients)
        }

        val files = stats?.optInt("filesReceived", stats.optInt("files_transferred", 0)) ?: 0
        val bytes = stats?.optLong("bytesReceived", stats.optLong("bytes_transferred", 0)) ?: 0

        // Connection status
        val connectionStatus = if (clients > 0) {
            getStringOrFallback(R.string.status_connected, "Connected")
        } else {
            getStringOrFallback(R.string.status_disconnected, "Disconnected")
        }

        // Format: connection status | received files count | total size
        return try {
            getString(R.string.status_format, connectionStatus, files, formatBytes(bytes))
        } catch (_: Exception) {
            "$connectionStatus | $files | ${formatBytes(bytes)}"
        }
    }

    /**
     * Format bytes to human-readable string
     */
    private fun formatBytes(bytes: Long): String {
        return when {
            bytes < 1024 -> "$bytes B"
            bytes < 1024 * 1024 -> "%.1f KB".format(bytes / 1024.0)
            bytes < 1024 * 1024 * 1024 -> "%.1f MB".format(bytes / (1024.0 * 1024))
            else -> "%.1f GB".format(bytes / (1024.0 * 1024 * 1024))
        }
    }

    /**
     * Update server stats and notification content.
     * Called when server is running to update stats display.
     * Thread-safe: can be called from any thread (e.g., JS bridge).
     */
    fun refreshFromCoordinator() {
        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        Log.d(TAG, "refreshFromCoordinator: connectedClients=${snapshot.connectedClients}")

        if (!snapshot.isRunning) {
            return
        }

        val statsChanged = applyServerState(snapshot.statsJson, snapshot.connectedClients)
        if (!statsChanged) {
            return
        }

        // Update notification with new stats
        updateNotification()
    }

    private fun restoreStateFromSnapshot(snapshot: AndroidServiceStateSnapshot) {
        if (!snapshot.isRunning) {
            return
        }

        applyServerState(snapshot.statsJson, snapshot.connectedClients)
    }

    private fun getStringOrFallback(@StringRes resId: Int, fallback: String): String {
        return try {
            getString(resId)
        } catch (_: Exception) {
            fallback
        }
    }

    private fun applyServerState(statsJson: String?, connectedClients: Int): Boolean {
        synchronized(stateLock) {
            val previousStatsJson = serverStats?.toString()
            val statsChanged = previousStatsJson != statsJson || this.connectedClients != connectedClients
            this.connectedClients = connectedClients

            if (statsJson != null) {
                try {
                    serverStats = JSONObject(statsJson)
                } catch (e: Exception) {
                    Log.e(TAG, "Error parsing stats JSON: $statsJson", e)
                    serverStats = null
                }
            } else {
                serverStats = null
            }

            return statsChanged
        }
    }

    /**
     * Update notification with current stats
     */
    private fun updateNotification() {
        Log.d(TAG, "updateNotification: updating notification")

        try {
            val notification = buildNotification()
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.notify(NOTIFICATION_ID, notification)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to update notification", e)
        }
    }
}
