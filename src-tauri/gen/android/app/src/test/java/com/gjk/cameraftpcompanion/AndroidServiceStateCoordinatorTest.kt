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
    fun update_service_state_persists_snapshot_before_service_instance_exists() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":1}", 2)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        assertTrue(snapshot.isRunning)
        assertEquals(2, snapshot.connectedClients)
        assertEquals("{\"files_transferred\":1}", snapshot.statsJson)
    }

    @Test
    fun update_service_state_starts_foreground_service_when_running() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":1}", 2)

        val startedIntent = shadowOf(getApplicationContext<android.app.Application>()).nextStartedService
        assertEquals(FtpForegroundService::class.java.name, startedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_START, startedIntent.action)
    }

    @Test
    fun update_service_state_stops_foreground_service_when_stopped() {
        val context = getApplicationContext<Context>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(getApplicationContext<android.app.Application>()).clearStartedServices()

        AndroidServiceStateCoordinator.updateServiceState(context, false, null, 0)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val stoppedIntent = shadowOf(getApplicationContext<android.app.Application>()).nextStoppedService
        assertTrue(!snapshot.isRunning)
        assertNull(snapshot.statsJson)
        assertEquals(0, snapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, stoppedIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_STOP, stoppedIntent.action)
    }

    @Test
    fun repeated_running_updates_do_not_restart_service_when_instance_exists() {
        val context = getApplicationContext<Context>()
        val application = getApplicationContext<android.app.Application>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(application).clearStartedServices()

        val service = Robolectric.buildService(FtpForegroundService::class.java).get()
        val instanceField = FtpForegroundService::class.java.getDeclaredField("instance")
        instanceField.isAccessible = true
        instanceField.set(null, service)

        AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":2}", 3)

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
        AndroidServiceStateCoordinator.updateServiceState(
            context,
            true,
            "{\"files_transferred\":5,\"bytes_transferred\":1024}",
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
        assertEquals("{\"files_transferred\":5,\"bytes_transferred\":1024}", restoredStats)
        assertNotNull(notification)
        assertTrue(notification.extras.getCharSequence("android.text")!!.contains("1.0 KB"))
    }

    @Test
    fun stale_start_intent_does_not_restart_service_when_snapshot_is_stopped() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
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
        assertNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
    }

    @Test
    fun direct_service_update_refreshes_notification_using_coordinator_backed_state() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.updateServiceState(
            context,
            true,
            "{\"files_transferred\":1,\"bytes_transferred\":512}",
            1,
        )

        val service = Robolectric.buildService(FtpForegroundService::class.java).create().get()
        service.onStartCommand(Intent(context, FtpForegroundService::class.java), 0, 1)
        service.updateServerState("{\"files_transferred\":7,\"bytes_transferred\":2048}", 3)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
        assertTrue(snapshot.isRunning)
        assertEquals(3, snapshot.connectedClients)
        assertEquals("{\"files_transferred\":7,\"bytes_transferred\":2048}", snapshot.statsJson)
        assertEquals(3, readConnectedClients(service))
        assertEquals("{\"files_transferred\":7,\"bytes_transferred\":2048}", readServiceStatsJson(service))
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
