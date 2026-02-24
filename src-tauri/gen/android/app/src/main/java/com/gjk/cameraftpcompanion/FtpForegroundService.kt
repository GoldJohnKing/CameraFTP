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
        
        // Android 8.0+ requires startForeground() to be called within 5 seconds of service start
        // Show initial notification immediately to prevent service from being killed by system
        val initialNotification = buildNotification()
        startForeground(NOTIFICATION_ID, initialNotification)
        LogWriter.log("startForeground() called immediately with initial notification")
        Log.d(TAG, "Service created, foreground notification shown immediately")
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: action=${intent?.action}")
        // No action handling needed anymore
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
     * Build notification with three states:
     * - Red icon (tray_stopped): Server stopped
     * - Yellow icon (tray_idle): Running but no clients
     * - Green icon (tray_active): Running with clients
     */
    private fun buildNotification(): Notification {
        // Three-state icon: stopped(red) / running no connection(yellow) / running with connection(green)
        val iconRes = when {
            !serverIsRunning -> R.drawable.tray_stopped
            connectedClients == 0 -> R.drawable.tray_idle
            else -> R.drawable.tray_active
        }
        
        // 标题固定为"图传伴侣  运行中"
        val title = if (serverIsRunning) "图传伴侣  运行中" else "图传伴侣  已停止"

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
     */
    private fun buildStatusContent(): String {
        if (!serverIsRunning) {
            return "服务已停止"
        }

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
     */
    fun updateServerState(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        LogWriter.log("========== updateServerState ==========")
        LogWriter.log("Parameters: isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        LogWriter.log("Current state: serverIsRunning=$serverIsRunning, connectedClients=${this.connectedClients}")
        
        Log.d(TAG, "========== updateServerState called ==========")
        Log.d(TAG, "Parameters: isRunning=$isRunning, statsJson=$statsJson, connectedClients=$connectedClients")
        Log.d(TAG, "Current state: serverIsRunning=$serverIsRunning, connectedClients=${this.connectedClients}")
        
        val oldState = serverIsRunning
        @Suppress("UNUSED_VARIABLE")
        val oldClients = this.connectedClients  // Keep for future use
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
                // Server just started - start foreground with notification
                LogWriter.log(">>> Server STARTED - calling startForeground()")
                Log.d(TAG, ">>> Server STARTED - calling startForeground()")
                try {
                    val notification = buildNotification()
                    LogWriter.log("Notification built successfully, ID: $NOTIFICATION_ID")
                    startForeground(NOTIFICATION_ID, notification)
                    LogWriter.log(">>> startForeground() SUCCESS")
                    Log.d(TAG, ">>> startForeground() completed successfully")
                } catch (e: Exception) {
                    LogWriter.logError(">>> startForeground() FAILED", e)
                    Log.e(TAG, ">>> startForeground() FAILED", e)
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
