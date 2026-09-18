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
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.annotation.StringRes
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Foreground service held while color-grading / AI-edit tasks are queued or
 * processing, so the batch keeps running when the app is backgrounded or the
 * screen is locked. Started/stopped exclusively via
 * [AndroidServiceStateCoordinator.syncNativeProcessingState], which is driven
 * from the Rust processing-activity tracker (edge-triggered).
 *
 * Structure mirrors [FtpForegroundService] (singleton instance, ACTION_START /
 * ACTION_STOP, START_NOT_STICKY, double onTimeout override, stale-start
 * defense), with a static notification (no dynamic progress refresh).
 */
class ProcessingForegroundService : Service() {
    @Volatile
    private var isInForeground = false

    companion object {
        const val TAG = "ProcessingForegroundService"
        const val NOTIFICATION_ID = 1002
        const val CHANNEL_ID = "processing_service_channel"

        // Actions
        const val ACTION_START = "com.gjk.cameraftpcompanion.PROCESSING_START_SERVICE"
        const val ACTION_STOP = "com.gjk.cameraftpcompanion.PROCESSING_STOP_SERVICE"
        // Singleton instance for the coordinator to access
        @Volatile
        private var instance: ProcessingForegroundService? = null

        fun getInstance(): ProcessingForegroundService? {
            return instance
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        Log.d(TAG, "onCreate: initializing service")
        super.onCreate()
        instance = this
        createNotificationChannel()

        // Note: startForeground() is called in onStartCommand() to satisfy Android's
        // 5-second requirement after startForegroundService().
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: action=${intent?.action}, startId=$startId")

        if (intent?.action == ACTION_STOP) {
            stopForegroundServiceNow("explicit stop action")
            return START_NOT_STICKY
        }

        if (!AndroidServiceStateCoordinator.getProcessingActive()) {
            Log.d(TAG, "onStartCommand: ignoring start because coordinator reports no processing activity")
            // The intent may have arrived via startForegroundService(): the
            // contract requires calling startForeground() before stopping,
            // otherwise some API 26+ ROMs throw
            // ForegroundServiceDidNotStartInTimeException even though we are
            // already stopping. The notification is static (no state deps),
            // so it builds fine in this stale scenario.
            startForegroundWithType(buildNotification())
            stopForegroundServiceNow("stale start while idle")
            return START_NOT_STICKY
        }

        // CRITICAL: Must call startForeground() within 5 seconds of startForegroundService()
        // Otherwise, Android will throw ForegroundServiceDidNotStartInTimeException and crash the app
        startForegroundWithType(buildNotification())

        return START_NOT_STICKY
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up service")
        instance = null
        isInForeground = false
        super.onDestroy()
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        Log.w(TAG, "onTimeout(startId, fgsType): startId=$startId, fgsType=$fgsType")
        AndroidServiceStateCoordinator.clearProcessingState()
        stopForegroundServiceNow("fgs timeout startId=$startId type=$fgsType")
    }

    override fun onTimeout(startId: Int) {
        Log.w(TAG, "onTimeout(startId): startId=$startId")
        AndroidServiceStateCoordinator.clearProcessingState()
        stopForegroundServiceNow("fgs timeout startId=$startId")
    }

    /**
     * Start foreground service with the mediaProcessing type on API 35+
     * (targetSdk 36 semantics: media processing is the correct type for
     * photo editing pipelines), falling back to dataSync on older systems.
     */
    private fun startForegroundWithType(notification: Notification) {
        val fgsType = if (Build.VERSION.SDK_INT >= 35) {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING
        } else {
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
        }
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            fgsType
        )
        isInForeground = true
        Log.d(TAG, "startForegroundWithType: started with fgsType=$fgsType")
    }

    private fun stopForegroundServiceNow(reason: String) {
        Log.d(TAG, "stopForegroundServiceNow: $reason")
        if (isInForeground) {
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            isInForeground = false
        }
        stopSelf()
    }

    /**
     * Create notification channel for the processing service notification.
     */
    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            getStringOrFallback(R.string.processing_service_channel_name, "Photo processing"),
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = getStringOrFallback(
                R.string.processing_service_channel_description,
                "Keeps photo processing (color grading & AI edit) running in the foreground",
            )
            setShowBadge(false)
            lockscreenVisibility = Notification.VISIBILITY_PUBLIC
        }

        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.createNotificationChannel(channel)
        Log.d(TAG, "createNotificationChannel: created notification channel")
    }

    /**
     * Build notification for the processing state.
     * Static text only — no dynamic progress refresh by design.
     */
    private fun buildNotification(): Notification {
        val title = getStringOrFallback(R.string.processing_notification_title, "Camera FTP companion | Processing")
        val content = getStringOrFallback(
            R.string.processing_notification_content,
            "Processing photos (color grading & AI edit)",
        )

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
            .setSmallIcon(R.drawable.tray_active)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    private fun getStringOrFallback(@StringRes resId: Int, fallback: String): String {
        return try {
            getString(resId)
        } catch (_: Exception) {
            fallback
        }
    }
}
