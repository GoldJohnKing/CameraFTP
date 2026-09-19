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
import org.json.JSONObject

/**
 * Foreground service held while color-grading / AI-edit tasks are queued or
 * processing, so the batch keeps running when the app is backgrounded or the
 * screen is locked. Started/stopped exclusively via
 * [AndroidServiceStateCoordinator.syncNativeProcessingState], which is driven
 * from the Rust processing-activity tracker (edge-triggered).
 *
 * Notification content is refreshed live from the Rust progress payload via
 * [AndroidServiceStateCoordinator.syncNativeProcessingProgress] (level data,
 * mirrors FtpForegroundService's stats-sync pattern): single-pipeline shows a
 * determinate progress bar, dual-pipeline shows both counters joined by a
 * separator (no bar — two totals cannot share one bar).
 *
 * Structure mirrors [FtpForegroundService] (singleton instance, ACTION_START /
 * ACTION_STOP, START_NOT_STICKY, double onTimeout override, stale-start
 * defense).
 */
class ProcessingForegroundService : Service() {
    @Volatile
    private var isInForeground = false

    /** 通知刷新主线程化用（见 refreshProcessingNotification 的竞态论证）。 */
    private val mainHandler = android.os.Handler(android.os.Looper.getMainLooper())

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
            // already stopping. The stop edge cleared the progress snapshot,
            // so buildNotification() falls back to static text and builds
            // fine in this stale scenario.
            startForegroundWithType(buildNotification())
            stopForegroundServiceNow("stale start while idle")
            return START_NOT_STICKY
        }

        // CRITICAL: Must call startForeground() within 5 seconds of startForegroundService()
        // Otherwise, Android will throw ForegroundServiceDidNotStartInTimeException and crash the app
        startForegroundWithType(buildNotification())
        // 补一次刷新：进度快照若在 buildNotification 与 startForeground 完成
        // 之间到达，refreshProcessingNotification 会因 isInForeground 尚未
        // 置位而丢弃该更新，静态文案滞留到下一个进度事件。此刻已置位，
        // 用最新快照重渲染一次（无新快照时为幂等同内容重发）。
        refreshProcessingNotification()

        return START_NOT_STICKY
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy: cleaning up service")
        instance = null
        isInForeground = false
        // 兜底：任何路径在 stopForeground(REMOVE) 之后重发布了通知（历史竞态），
        // 显式 cancel 确保服务销毁后不残留游离的普通通知。
        try {
            (getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager).cancel(NOTIFICATION_ID)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to cancel notification on destroy", e)
        }
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
     *
     * Content is composed from the coordinator's latest progress snapshot
     * (pushed by Rust at every progress-event emit point, and also carried by
     * the start edge so the very first notification renders numbers):
     * - single pipeline: "RAW 调色：1 / 2" (+ "（失败 N）" when failed>0)
     * - dual pipeline: both counters joined by " | "
     * - no/invalid snapshot: static fallback text (previous behavior).
     * Pure-text style matching the FTP FGS notification: no progress bar,
     * no BigTextStyle. onStartCommand's start path calls this too, so a
     * progress payload that raced ahead of the service start is rendered
     * without an extra refresh.
     */
    private fun buildNotification(): Notification {
        val title = getStringOrFallback(R.string.processing_notification_title, "Camera FTP companion | Processing")
        val progress = parseProgressSnapshot()
        val content = buildProgressContent(progress)

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

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
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

        // 纯文字样式（对齐 FTP FGS 通知）：不使用进度条与 BigTextStyle。
        return builder.build()
    }

    /** Parsed progress snapshot: (color grading, AI edit); null = use fallback. */
    private data class PipelineProgressLine(val done: Int, val total: Int, val failed: Int)

    private fun parseProgressSnapshot(): Pair<PipelineProgressLine?, PipelineProgressLine?>? {
        val raw = AndroidServiceStateCoordinator.getProcessingProgressJson() ?: return null
        return try {
            val root = JSONObject(raw)
            Pair(parseProgressLine(root, "cg"), parseProgressLine(root, "ai"))
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse processing progress JSON: $raw", e)
            null
        }
    }

    private fun parseProgressLine(root: JSONObject, key: String): PipelineProgressLine? {
        val obj = root.optJSONObject(key) ?: return null
        return PipelineProgressLine(
            obj.optInt("done", 0),
            obj.optInt("total", 0),
            obj.optInt("failed", 0),
        )
    }

    private fun buildProgressContent(
        progress: Pair<PipelineProgressLine?, PipelineProgressLine?>?,
    ): String {
        if (progress == null) {
            return getStringOrFallback(
                R.string.processing_notification_content,
                "Processing photos (color grading & AI edit)",
            )
        }

        val (cg, ai) = progress
        val parts = mutableListOf<String>()
        cg?.let { parts.add(formatPipelineLine(R.string.processing_progress_cg, "RAW grading: %1\$d / %2\$d", it)) }
        ai?.let { parts.add(formatPipelineLine(R.string.processing_progress_ai, "AI edit: %1\$d / %2\$d", it)) }
        if (parts.isEmpty()) {
            // Both pipelines idle in the snapshot: fall back to static text.
            return getStringOrFallback(
                R.string.processing_notification_content,
                "Processing photos (color grading & AI edit)",
            )
        }
        val separator = getStringOrFallback(R.string.processing_progress_separator, " | ")
        return parts.joinToString(separator)
    }

    private fun formatPipelineLine(
        @StringRes resId: Int,
        fallback: String,
        p: PipelineProgressLine,
    ): String {
        val base = getStringOrFallback(resId, fallback, p.done, p.total)
        return if (p.failed > 0) {
            base + getStringOrFallback(
                R.string.processing_progress_failed_suffix,
                " (%1\$d failed)",
                p.failed,
            )
        } else {
            base
        }
    }

    /**
     * Re-post the notification with the latest progress snapshot.
     * Called from AndroidServiceStateCoordinator (Rust progress pushes);
     * no-op unless the service is already in the foreground — liveness is
     * owned by the start/stop edge channel.
     */
    fun refreshProcessingNotification() {
        if (!isInForeground) {
            return
        }
        // 主线程化：守卫与 notify 在主线程原子执行，消除「JNI 线程读到
        // isInForeground==true 后、主线程 stopForeground(REMOVE) 完成前」
        // 的重发布竞态（该竞态会在服务停止后把通知重新 post 成普通通知，
        // 造成取消批次后通知残留——真机偶发复现）。
        mainHandler.post {
            if (!isInForeground) {
                return@post
            }
            try {
                val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                notificationManager.notify(NOTIFICATION_ID, buildNotification())
            } catch (e: Exception) {
                Log.e(TAG, "Failed to update processing notification", e)
            }
        }
    }

    private fun getStringOrFallback(@StringRes resId: Int, fallback: String): String {
        return try {
            getString(resId)
        } catch (_: Exception) {
            fallback
        }
    }

    private fun getStringOrFallback(
        @StringRes resId: Int,
        fallback: String,
        vararg formatArgs: Any,
    ): String {
        return try {
            getString(resId, *formatArgs)
        } catch (_: Exception) {
            String.format(fallback, *formatArgs)
        }
    }
}
