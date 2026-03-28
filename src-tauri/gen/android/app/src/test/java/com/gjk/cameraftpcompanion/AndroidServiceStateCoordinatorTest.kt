/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Service
import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider.getApplicationContext
import androidx.test.core.app.ApplicationProvider
import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertNull
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class AndroidServiceStateCoordinatorTest {

    @Test
    fun legacy_server_state_bridge_source_is_removed() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt")

        assertFalse(Files.exists(sourcePath))
    }

    @Test
    fun update_service_state_persists_snapshot_before_service_instance_exists() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        assertTrue(snapshot.isRunning)
        assertEquals(2, snapshot.connectedClients)
        assertEquals("{\"files_transferred\":1}", snapshot.statsJson)
    }

    @Test
    fun update_service_state_starts_foreground_service_when_running() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)

        val startedIntent = shadowOf(getApplicationContext<android.app.Application>()).nextStartedService
        assertEquals(FtpForegroundService::class.java.name, startedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_START, startedIntent.action)
    }

    @Test
    fun update_service_state_stops_foreground_service_via_stop_service_call() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(getApplicationContext<android.app.Application>()).clearStartedServices()

        AndroidServiceStateCoordinator.syncNativeServiceState(context, false, null, 0)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val stoppedIntent = shadowOf(getApplicationContext<android.app.Application>()).nextStoppedService
        assertTrue(!snapshot.isRunning)
        assertNull(snapshot.statsJson)
        assertEquals(0, snapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, stoppedIntent.component?.className)
        assertEquals(null, stoppedIntent.action)
    }

    @Test
    fun repeated_running_updates_do_not_restart_service_when_instance_exists() {
        val context = getApplicationContext<Context>()
        val application = getApplicationContext<android.app.Application>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(application).clearStartedServices()

        val service = Robolectric.buildService(FtpForegroundService::class.java).get()
        val instanceField = FtpForegroundService::class.java.getDeclaredField("instance")
        instanceField.isAccessible = true
        instanceField.set(null, service)

        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":2}", 3)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val restartedIntent = shadowOf(application).nextStartedService
        assertTrue(snapshot.isRunning)
        assertEquals(3, snapshot.connectedClients)
        assertEquals("{\"files_transferred\":2}", snapshot.statsJson)
        assertNull(restartedIntent)

        instanceField.set(null, null)
    }

    @Test
    fun service_restores_notification_state_from_coordinator_snapshot() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":4,\"filesReceived\":5,\"bytesReceived\":1024,\"lastFile\":null}",
            4,
        )

        val service = Robolectric.buildService(FtpForegroundService::class.java).create().get()
        service.onStartCommand(Intent(context, FtpForegroundService::class.java), 0, 1)

        val restored = AndroidServiceStateCoordinator.getLatestState()
        val restoredStats = readServiceStatsJson(service)
        val notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
        assertTrue(restored.isRunning)
        assertEquals(4, restored.connectedClients)
        assertEquals(4, readConnectedClients(service))
        assertEquals("{\"isRunning\":true,\"connectedClients\":4,\"filesReceived\":5,\"bytesReceived\":1024,\"lastFile\":null}", restoredStats)
        assertNotNull(notification)
        assertTrue(notification.extras.getCharSequence("android.text")!!.contains("1.0 KB"))
    }

    @Test
    fun stale_start_intent_does_not_restart_service_when_snapshot_is_stopped() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val controller = Robolectric.buildService(FtpForegroundService::class.java).create()
        val service = controller.get()

        AndroidServiceStateCoordinator.clearState()

        val result = service.onStartCommand(
            Intent(context, FtpForegroundService::class.java).apply {
                action = FtpForegroundService.ACTION_START
            },
            0,
            1,
        )

        assertEquals(Service.START_NOT_STICKY, result)
        assertEquals(0, readConnectedClients(service))
        assertNull(readServiceStatsJson(service))
    }

    @Test
    fun direct_native_update_refreshes_notification_using_rust_payload_shape() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":1,\"bytesReceived\":512,\"lastFile\":null}",
            1,
        )

        val service = Robolectric.buildService(FtpForegroundService::class.java).create().get()
        service.onStartCommand(Intent(context, FtpForegroundService::class.java), 0, 1)
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":7,\"bytesReceived\":2048,\"lastFile\":null}",
            3,
        )

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
        assertTrue(snapshot.isRunning)
        assertEquals(3, snapshot.connectedClients)
        assertEquals("{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":7,\"bytesReceived\":2048,\"lastFile\":null}", snapshot.statsJson)
        assertEquals(3, readConnectedClients(service))
        assertEquals("{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":7,\"bytesReceived\":2048,\"lastFile\":null}", readServiceStatsJson(service))
        assertNotNull(notification)
        assertTrue(notification.extras.getCharSequence("android.text")!!.contains("2.0 KB"))
    }

    private fun readConnectedClients(service: FtpForegroundService): Int {
        val field = FtpForegroundService::class.java.getDeclaredField("connectedClients")
        field.isAccessible = true
        return field.getInt(service)
    }

    private fun readServiceStatsJson(service: FtpForegroundService): String? {
        val field = FtpForegroundService::class.java.getDeclaredField("serverStats")
        field.isAccessible = true
        return field.get(service)?.toString()
    }
}
