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
            Log.d(TAG, "getInstance() called, instance=$instance")
            return instance
        }
    }
    
    // State
    private var serverIsRunning = false
    private var serverStats: JSONObject? = null
    private var connectedClients = 0
    
    // Locks
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    
    // WebView reference (set by MainActivity)
    var webView: android.webkit.WebView? = null
    
    override fun onBind(intent: Intent?): IBinder? = null
    
    override fun onCreate() {
        super.onCreate()
        instance = this
        LogWriter.init()
        LogWriter.log("FtpForegroundService.onCreate() called")
        LogWriter.log("instance=$instance")
        createNotificationChannel()
        LogWriter.log("Notification channel created")
        acquireLocks()
        LogWriter.log("Locks acquired")
        
        // Note: startForeground() is called in onStartCommand() to satisfy Android's
        // 5-second requirement after startForegroundService(). The notification is
        // initially in "waiting" state and gets updated when server actually starts.
        LogWriter.log("Service created, foreground notification will be shown in onStartCommand()")
        Log.d(TAG, "Service created, foreground notification will be shown in onStartCommand()")
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: action=${intent?.action}")
        
        // CRITICAL: Must call startForeground() within 5 seconds of startForegroundService()
        // Otherwise, Android will throw ForegroundServiceDidNotStartInTimeException and crash the app
        if (!serverIsRunning) {
            Log.d(TAG, "Service started but server not running yet - showing init notification")
            val initNotification = buildInitNotification()
            startForeground(NOTIFICATION_ID, initNotification)
        }
        
        return START_STICKY
    }
    
    override fun onDestroy() {
        instance = null
        releaseLocks()
        Log.d(TAG, "Service destroyed")
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
            Log.d(TAG, "Notification channel created")
        }
    }
    
    /**
     * Acquire WakeLock and WifiLock to keep device running
     */
    private fun acquireLocks() {
        // Acquire partial wake lock to keep CPU running
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "FtpForegroundService::WakeLock"
        ).apply {
            acquire(10 * 60 * 1000L) // 10 minutes timeout, will be re-acquired
        }
        Log.d(TAG, "WakeLock acquired")
        
        // Acquire WiFi lock to keep WiFi connection alive
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        @Suppress("DEPRECATION")
        wifiLock = wifiManager.createWifiLock(
            WifiManager.WIFI_MODE_FULL_HIGH_PERF,
            "FtpForegroundService::WifiLock"
        ).apply {
            acquire()
        }
        Log.d(TAG, "WifiLock acquired")
    }
    
    /**
     * Release all locks
     */
    private fun releaseLocks() {
        wakeLock?.let {
            if (it.isHeld) {
                it.release()
                Log.d(TAG, "WakeLock released")
            }
        }
        wakeLock = null
        
        wifiLock?.let {
            if (it.isHeld) {
                it.release()
                Log.d(TAG, "WifiLock released")
            }
        }
        wifiLock = null
    }
    
    /**
     * Build initial notification shown immediately when service starts.
     * This is required to satisfy Android's 5-second foreground service requirement.
     */
    private fun buildInitNotification(): Notification {
        val title = "图传伴侣 | 启动中"
        val content = "正在初始化服务..."
        
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            packageManager.getLaunchIntentForPackage(packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(content)
            .setSmallIcon(R.drawable.tray_idle)  // Yellow icon for waiting state
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }
    
    /**
     * Build notification for running server state only.
     * Notification is only shown when server is running.
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
            bytes < 1024 * 1024 -> String.format("%.1f KB", bytes / 1024.0)
            bytes < 1024 * 1024 * 1024 -> String.format("%.1f MB", bytes / (1024.0 * 1024))
            else -> String.format("%.1f GB", bytes / (1024.0 * 1024 * 1024))
        }
    }
    
    /**
     * Update server state and show/hide notification based on server state
     * - Server running: show notification with stats
     * - Server stopped: remove notification
     */
    fun updateServerState(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        LogWriter.log("========== updateServerState ==========")
        LogWriter.log("Parameters: isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        LogWriter.log("Current state: serverIsRunning=$serverIsRunning, connectedClients=${this.connectedClients}")
        
        Log.d(TAG, "========== updateServerState called ==========")
        Log.d(TAG, "Parameters: isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        Log.d(TAG, "Current state: serverIsRunning=$serverIsRunning, connectedClients=${this.connectedClients}")
        
        val oldState = serverIsRunning
        serverIsRunning = isRunning
        this.connectedClients = connectedClients
        
        if (statsJson != null) {
            try {
                serverStats = JSONObject(statsJson)
                LogWriter.log("Parsed stats: $serverStats")
                Log.d(TAG, "Parsed stats: $serverStats")
            } catch (e: Exception) {
                LogWriter.logError("Error parsing stats JSON: $statsJson", e)
                Log.e(TAG, "Error parsing stats JSON: $statsJson", e)
                serverStats = null
            }
        } else {
            serverStats = null
        }
        
        LogWriter.log("State transition: oldState=$oldState -> newState=$isRunning")
        Log.d(TAG, "State transition: oldState=$oldState -> newState=$isRunning")
        
        if (isRunning) {
            // Server is running - show/update notification
            if (!oldState) {
                // Server just started - update foreground notification from init to running
                LogWriter.log(">>> Server STARTED - updating notification to running state")
                Log.d(TAG, ">>> Server STARTED - updating notification to running state")
                try {
                    val notification = buildNotification()
                    LogWriter.log("Notification built successfully, ID: $NOTIFICATION_ID")
                    // Service is already in foreground from onStartCommand(), just update notification
                    startForeground(NOTIFICATION_ID, notification)
                    LogWriter.log(">>> Notification updated to running state SUCCESS")
                    Log.d(TAG, ">>> Notification updated to running state successfully")
                } catch (e: Exception) {
                    LogWriter.logError(">>> Update notification FAILED", e)
                    Log.e(TAG, ">>> Update notification FAILED", e)
                }
            } else {
                // Just update existing notification
                LogWriter.log(">>> Server state update - updating notification")
                Log.d(TAG, ">>> Server state update - updating notification")
                updateNotification()
            }
        } else {
            // Server is stopped - stop foreground and remove notification
            if (oldState) {
                LogWriter.log(">>> Server STOPPED - calling stopForeground()")
                Log.d(TAG, ">>> Server STOPPED - calling stopForeground()")
                stopForeground(STOP_FOREGROUND_REMOVE)
                LogWriter.log(">>> stopForeground() completed")
                Log.d(TAG, ">>> stopForeground() completed")
            }
        }
        
        LogWriter.log("========== updateServerState completed ==========")
        Log.d(TAG, "========== updateServerState completed ==========")
    }
    
    /**
     * Update the notification with current state
     */
    private fun updateNotification() {
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val notification = buildNotification()
        notificationManager.notify(NOTIFICATION_ID, notification)
        Log.d(TAG, "Notification updated")
    }
    
    /**
     * Send JS event to WebView
     */
    private fun sendJsEvent(eventName: String, data: JSONObject?) {
        webView?.post {
            try {
                val jsCode = if (data != null) {
                    "window.__androidBridge?.emit('$eventName', ${data.toString()})"
                } else {
                    "window.__androidBridge?.emit('$eventName')"
                }
                webView?.evaluateJavascript(jsCode, null)
                Log.d(TAG, "JS event sent: $eventName")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to send JS event: ${e.message}")
            }
        }
    }
}
