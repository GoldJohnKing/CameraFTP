/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.content.Context
import android.content.Intent

data class AndroidServiceStateSnapshot(
    val isRunning: Boolean = false,
    val statsJson: String? = null,
    val connectedClients: Int = 0,
)

object AndroidServiceStateCoordinator {
    @Volatile
    private var latestState = AndroidServiceStateSnapshot()

    @Synchronized
    private fun storeSnapshot(
        isRunning: Boolean,
        statsJson: String?,
        connectedClients: Int,
    ): AndroidServiceStateSnapshot {
        latestState = if (isRunning) {
            AndroidServiceStateSnapshot(true, statsJson, connectedClients)
        } else {
            AndroidServiceStateSnapshot()
        }

        return latestState
    }

    @JvmStatic
    fun syncNativeServiceState(
        callerContext: Context,
        isRunning: Boolean,
        statsJson: String?,
        connectedClients: Int,
    ) {
        if (isRunning) {
            updateRunningState(callerContext, statsJson, connectedClients)
        } else {
            stopService(callerContext)
        }
    }

    fun updateRunningState(callerContext: Context, statsJson: String?, connectedClients: Int) {
        val appContext = callerContext.applicationContext
        val previousState = latestState
        storeSnapshot(true, statsJson, connectedClients)

        if (!previousState.isRunning || FtpForegroundService.getInstance() == null) {
            startForegroundService(appContext)
        }

        FtpForegroundService.getInstance()?.refreshFromCoordinator()
    }

    fun stopService(callerContext: Context) {
        val appContext = callerContext.applicationContext
        storeSnapshot(false, null, 0)

        if (FtpForegroundService.getInstance() == null) {
            return
        }

        stopForegroundService(appContext)
    }

    fun getLatestState(): AndroidServiceStateSnapshot = latestState

    fun clearState() {
        latestState = AndroidServiceStateSnapshot()
    }

    private fun startForegroundService(appContext: Context) {
        val serviceIntent = Intent(appContext, FtpForegroundService::class.java).apply {
            action = FtpForegroundService.ACTION_START
        }

        appContext.startForegroundService(serviceIntent)
    }

    private fun stopForegroundService(appContext: Context) {
        val serviceIntent = Intent(appContext, FtpForegroundService::class.java).apply {
            action = FtpForegroundService.ACTION_STOP
        }

        appContext.startService(serviceIntent)
    }

    // ====================================================================
    // Processing (color grading / AI edit) FGS state — independent section.
    // Intentionally shares nothing with the FTP state above: separate lock,
    // separate flag, separate service, so neither channel can block or
    // reorder the other. Lives in this class to reuse the existing proguard
    // keep rule and the Rust-side class-loading path.
    // ====================================================================

    private val processingLock = Any()

    @Volatile
    private var processingActive = false

    /**
     * JNI entrypoint (called from Rust via class-name string lookup).
     * Edge semantics: false→true starts the foreground service when it is
     * not already running; true→false sends ACTION_STOP. Synchronized so
     * concurrent true/false transitions cannot reorder.
     */
    @JvmStatic
    fun syncNativeProcessingState(callerContext: Context, active: Boolean) {
        val appContext = callerContext.applicationContext
        synchronized(processingLock) {
            val previous = processingActive
            processingActive = active
            if (active) {
                if (!previous || ProcessingForegroundService.getInstance() == null) {
                    startProcessingForegroundService(appContext)
                }
            } else if (previous) {
                if (ProcessingForegroundService.getInstance() == null) {
                    // Service not created yet: its onStartCommand stale-start
                    // defense (getProcessingActive()==false) will stop it.
                    return
                }
                stopProcessingForegroundService(appContext)
            }
        }
    }

    /** Read by ProcessingForegroundService.onStartCommand for stale-start defense. */
    fun getProcessingActive(): Boolean = processingActive

    /** Called from ProcessingForegroundService.onTimeout. */
    fun clearProcessingState() {
        synchronized(processingLock) {
            processingActive = false
        }
    }

    private fun startProcessingForegroundService(appContext: Context) {
        val serviceIntent = Intent(appContext, ProcessingForegroundService::class.java).apply {
            action = ProcessingForegroundService.ACTION_START
        }

        appContext.startForegroundService(serviceIntent)
    }

    private fun stopProcessingForegroundService(appContext: Context) {
        val serviceIntent = Intent(appContext, ProcessingForegroundService::class.java).apply {
            action = ProcessingForegroundService.ACTION_STOP
        }

        // startService + action (not stopService): avoids background
        // stopService restrictions, mirrors the FTP stop path. Safe because
        // the running FGS elevates the process above background-start limits.
        appContext.startService(serviceIntent)
    }
}
