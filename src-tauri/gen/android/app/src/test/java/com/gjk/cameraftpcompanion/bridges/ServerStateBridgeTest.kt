/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.gjk.cameraftpcompanion.AndroidServiceStateCoordinator
import com.gjk.cameraftpcompanion.FtpForegroundService
import com.gjk.cameraftpcompanion.MainActivity
import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ServerStateBridgeTest {

    @Test
    fun bridge_forwards_to_coordinator_without_main_activity() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val bridge = ServerStateBridge(context)

        AndroidServiceStateCoordinator.clearState()
        bridge.onServerStateChanged(true, "{\"files_transferred\":3}", 1)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val startedIntent = shadowOf(ApplicationProvider.getApplicationContext<android.app.Application>()).nextStartedService
        assertTrue(snapshot.isRunning)
        assertEquals(1, snapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, startedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_START, startedIntent.action)
    }

    @Test
    fun service_control_is_coordinator_driven_from_application_context() {
        val application = ApplicationProvider.getApplicationContext<android.app.Application>()

        AndroidServiceStateCoordinator.clearState()
        shadowOf(application).clearStartedServices()

        AndroidServiceStateCoordinator.startService(application)

        val startedIntent = shadowOf(application).nextStartedService
        val runningSnapshot = AndroidServiceStateCoordinator.getLatestState()
        assertTrue(runningSnapshot.isRunning)
        assertEquals(FtpForegroundService::class.java.name, startedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_START, startedIntent.action)

        AndroidServiceStateCoordinator.updateRunningState(application, "{\"files_transferred\":7}", 2)

        val updatedSnapshot = AndroidServiceStateCoordinator.getLatestState()
        assertTrue(updatedSnapshot.isRunning)
        assertEquals(2, updatedSnapshot.connectedClients)
        assertEquals("{\"files_transferred\":7}", updatedSnapshot.statsJson)

        AndroidServiceStateCoordinator.stopService(application)

        val stoppedIntent = shadowOf(application).nextStoppedService
        val stoppedSnapshot = AndroidServiceStateCoordinator.getLatestState()
        assertFalse(stoppedSnapshot.isRunning)
        assertNull(stoppedSnapshot.statsJson)
        assertEquals(0, stoppedSnapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, stoppedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_STOP, stoppedIntent.action)
    }

    @Test
    fun main_activity_update_service_state_is_not_required_for_bridge_sync() {
        val application = ApplicationProvider.getApplicationContext<Context>()
        val bridge = ServerStateBridge(application)

        AndroidServiceStateCoordinator.clearState()

        bridge.onServerStateChanged(true, "{\"files_transferred\":4}", 1)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        assertTrue(snapshot.isRunning)
        assertEquals(1, snapshot.connectedClients)
        assertEquals("{\"files_transferred\":4}", snapshot.statsJson)
    }

    @Test
    fun main_activity_no_longer_exposes_service_state_relay_api() {
        val hasRelayApi = MainActivity::class.java.declaredMethods.any { method ->
            method.name == "updateServiceState"
        }

        assertFalse(hasRelayApi)
    }

    @Test
    fun main_activity_no_longer_registers_android_service_state_update_relay() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt")
        val source = String(Files.readAllBytes(sourcePath))

        assertFalse(source.contains("android-service-state-update"))
    }
}
