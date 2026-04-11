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
}
