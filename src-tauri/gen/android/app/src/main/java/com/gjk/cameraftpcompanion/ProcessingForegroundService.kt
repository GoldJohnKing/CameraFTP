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
        // v2：Android 对"删除后以相同 ID 重建"的渠道恢复旧 importance（文档化行为），
        // 升级 importance 必须换新 ID。见 createNotificationChannel 注释。
        const val CHANNEL_ID = "processing_service_channel_v2"
        private const val LEGACY_CHANNEL_ID = "processing_service_channel"

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
     * Start foreground service by trying candidate FGS types in priority
     * order (mediaProcessing then dataSync on API 35+; dataSync only on
     * older systems). Calls the platform startForeground directly instead
     * of ServiceCompat so no androidx masking layer can drop the requested
     * type bits. A type-negotiation failure must never escape as an
     * uncaught exception on the main thread.
     */
    private fun startForegroundWithType(notification: Notification) {
        // 类型回退梯：androidx ServiceCompat.startForeground 的 Api34Impl 会把
        // 请求类型与 FOREGROUND_SERVICE_TYPE_ALLOWED_SINCE_U 掩码按位与，该掩码
        // 至今不含 MEDIA_PROCESSING → 0x2000 被归零 → targetSdk≥34 直接
        // "type none" 崩溃（真机 Xiaomi Android 16 实测，AOSP/androidx 源码级
        // 确认，见 known-deferred-issues #10）。故此处直调平台 startForeground
        // 并按优先级逐类型回退；全部失败则记录并 stopSelf——绝不让 FGS 类型
        // 协商失败演变成主线程未捕获异常。
        val candidates: List<Int> = if (Build.VERSION.SDK_INT >= 35) {
            listOf(ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING,
                   ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            listOf(ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        }
        var lastError: Exception? = null
        for (type in candidates) {
            try {
                if (Build.VERSION.SDK_INT >= 29) {
                    startForeground(NOTIFICATION_ID, notification, type)
                } else {
                    @Suppress("DEPRECATION")
                    startForeground(NOTIFICATION_ID, notification)
                }
                isInForeground = true
                Log.d(TAG, "startForegroundWithType: engaged with fgsType=$type (ladder)")
                return
            } catch (e: Exception) {
                lastError = e
                Log.w(TAG, "startForegroundWithType: fgsType=$type rejected: $e; trying next rung")
            }
        }
        Log.e(TAG, "startForegroundWithType: all rungs failed", lastError)
        stopForegroundServiceNow("fgs type ladder exhausted")
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
        // IMPORTANCE_DEFAULT（而非 LOW）：LOW/静默渠道在 HyperOS 上会折叠置底并对
        // 已下拉的通知栏延迟渲染，造成"任务已开始但通知迟迟不出现"的观感（真机实测
        // 通知 posted 时刻仅滞后入队 18ms）。DEFAULT 首发弹出 heads-up、置顶排布；
        // setSound(null) 保持静音避免声音打扰。
        val channel = NotificationChannel(
            CHANNEL_ID,
            getStringOrFallback(R.string.processing_service_channel_name, "Photo processing"),
            NotificationManager.IMPORTANCE_DEFAULT
        ).apply {
            description = getStringOrFallback(
                R.string.processing_service_channel_description,
                "Keeps photo processing (color grading & AI edit) running in the foreground",
            )
            setShowBadge(false)
            setSound(null, null)
            lockscreenVisibility = Notification.VISIBILITY_PUBLIC
        }

        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        // v2 渠道：Android 对"删除后以相同 ID 重建"的渠道会恢复旧 importance（文档化
        // 行为，防止应用重置用户设置），故升级重要性必须换新 ID；deleteNotificationChannel
        // 对不存在的 ID 是无害 no-op，顺带清理旧渠道。
        notificationManager.deleteNotificationChannel(LEGACY_CHANNEL_ID)
        notificationManager.createNotificationChannel(channel)
        Log.d(TAG, "createNotificationChannel: created notification channel (v2)")
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
            // Android 13+ 默认推迟前台服务的通知显示（应用在前台时视为冗余），
            // 造成"任务已开始但通知迟迟不显示"；IMMEDIATE 覆盖该默认（真机验证：
            // 入队→通知 posted 仅 +11ms，但可见性被系统推迟）。
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
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
