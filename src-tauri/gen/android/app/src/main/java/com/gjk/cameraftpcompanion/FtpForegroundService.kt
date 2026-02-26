package com.gjk.cameraftpcompanion

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import org.json.JSONObject

class FtpForegroundService : Service() {
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
    
    // State
    private var serverStats: JSONObject? = null
    private var connectedClients = 0
    
    // Locks
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    
    override fun onBind(intent: Intent?): IBinder? = null
    
    override fun onCreate() {
        super.onCreate()
        instance = this
        createNotificationChannel()
        acquireLocks()
        
        // Note: startForeground() is called in onStartCommand() to satisfy Android's
        // 5-second requirement after startForegroundService().
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Handle stop service action
        if (intent?.action == ACTION_STOP) {
            stopSelf()
            return START_NOT_STICKY
        }
        
        // CRITICAL: Must call startForeground() within 5 seconds of startForegroundService()
        // Otherwise, Android will throw ForegroundServiceDidNotStartInTimeException and crash the app
        // Service is only started when server is running, so always show running notification
        val notification = buildNotification()
        startForeground(NOTIFICATION_ID, notification)

        return START_STICKY
    }
    
    override fun onDestroy() {
        instance = null
        releaseLocks()
        super.onDestroy()
    }
    
    /**
     * Create notification channel for Android O+
     */
    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "FTP Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "FTP Server Status"
                setShowBadge(false)
                lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            }
            
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(channel)
        }
    }
    
    /**
     * Acquire WakeLock and WifiLock to keep device running
     */
    private fun acquireLocks() {
        // Acquire partial wake lock to keep CPU running
        // No timeout - service lifecycle manages release via onDestroy()
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "FtpForegroundService::WakeLock"
        ).apply {
            acquire() // Indefinite - released when service stops
        }

        // Acquire WiFi lock to keep WiFi connection alive
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        @Suppress("DEPRECATION")
        wifiLock = wifiManager.createWifiLock(
            WifiManager.WIFI_MODE_FULL_HIGH_PERF,
            "FtpForegroundService::WifiLock"
        ).apply {
            acquire()
        }
    }
    
    /**
     * Release all locks
     */
    private fun releaseLocks() {
        wakeLock?.let {
            if (it.isHeld) {
                it.release()
            }
        }
        wakeLock = null

        wifiLock?.let {
            if (it.isHeld) {
                it.release()
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

        val title = "图传伴侣 | 运行中"
        val content = buildStatusContent()
        
        // Intent to open MainActivity when tapped
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            packageManager.getLaunchIntentForPackage(packageName),
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
     */
    private fun buildStatusContent(): String {
        val stats = serverStats
        val files = stats?.optInt("files_transferred", 0) ?: 0
        val bytes = stats?.optLong("bytes_transferred", 0) ?: 0

        // 连接状态
        val connectionStatus = if (connectedClients > 0) "已连接" else "未连接"

        // 格式：连接状态 | 已接收图片数 | 已接收图片总大小
        return "$connectionStatus | ${files}张 | ${formatBytes(bytes)}"
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
     */
    fun updateServerState(statsJson: String?, connectedClients: Int) {
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

        // Update notification with new stats
        updateNotification()
    }
    
    /**
     * Update notification with current stats
     */
    private fun updateNotification() {
        try {
            val notification = buildNotification()
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.notify(NOTIFICATION_ID, notification)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to update notification", e)
        }
    }
}
