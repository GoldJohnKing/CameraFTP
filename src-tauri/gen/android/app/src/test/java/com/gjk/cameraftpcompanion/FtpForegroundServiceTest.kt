/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.PowerManager
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class FtpForegroundServiceTest {
    @Test
    fun start_update_and_stop_flow_uses_direct_native_payload_and_real_stop_path() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val application = context as android.app.Application
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":2,\"bytesReceived\":1024,\"lastFile\":null}",
            1,
        )

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
            service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_START
                },
                0,
                1,
            )

            AndroidServiceStateCoordinator.syncNativeServiceState(
                context,
                true,
                "{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":4,\"bytesReceived\":2048,\"lastFile\":null}",
                3,
            )

            var notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
            assertNotNull(notification)
            assertTrue(notification.extras.getCharSequence("android.text")!!.contains("2.0 KB"))
            assertEquals(3, readConnectedClients(service))
            assertEquals(
                "{\"isRunning\":true,\"connectedClients\":3,\"filesReceived\":4,\"bytesReceived\":2048,\"lastFile\":null}",
                readServiceStatsJson(service),
            )

            shadowOf(application).clearStartedServices()
            AndroidServiceStateCoordinator.syncNativeServiceState(context, false, null, 0)

            val snapshot = AndroidServiceStateCoordinator.getLatestState()
            val stoppedIntent = shadowOf(application).nextStartedService
            val stopResult = service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_STOP
                },
                0,
                2,
            )
            notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
            assertEquals(Service.START_NOT_STICKY, stopResult)
            assertFalse(snapshot.isRunning)
            assertEquals(0, snapshot.connectedClients)
            assertEquals(FtpForegroundService::class.java.name, stoppedIntent.component?.className)
            assertEquals(FtpForegroundService.ACTION_STOP, stoppedIntent.action)
            assertNull(notification)
            assertFalse(readIsInForeground(service))
        } finally {
            serviceController.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    @Test
    fun onTimeout_stops_service_now_and_clears_state() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":2,\"bytesReceived\":1024,\"lastFile\":null}",
            1,
        )

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
            service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_START
                },
                0,
                1,
            )

            service.onTimeout(2)
            assertFalse(AndroidServiceStateCoordinator.getLatestState().isRunning)
            assertEquals(0, readConnectedClients(service))
            assertNull(readServiceStatsJson(service))
            assertFalse(readIsInForeground(service))
            assertNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
        } finally {
            serviceController.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    @Test
    @Config(shadows = [ThrowingWakeLockPowerManagerShadow::class])
    fun start_with_wake_lock_creation_failure_does_not_crash_and_timeout_cleanup_still_works() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(
            context,
            true,
            "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":2,\"bytesReceived\":1024,\"lastFile\":null}",
            1,
        )

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
            val startResult = runCatching {
                service.onStartCommand(
                    Intent(context, FtpForegroundService::class.java).apply {
                        action = FtpForegroundService.ACTION_START
                    },
                    0,
                    1,
                )
            }

            assertTrue(startResult.isSuccess)
            assertEquals(Service.START_NOT_STICKY, startResult.getOrThrow())
            assertTrue(readIsInForeground(service))
            assertNotNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
            assertNull(readWakeLock(service))

            service.onTimeout(2)
            assertFalse(AndroidServiceStateCoordinator.getLatestState().isRunning)
            assertEquals(0, readConnectedClients(service))
            assertNull(readServiceStatsJson(service))
            assertFalse(readIsInForeground(service))
            assertNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
        } finally {
            serviceController.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    private fun readConnectedClients(service: FtpForegroundService): Int {
        return withAccessibleField(service, "connectedClients") { field ->
            field.getInt(service)
        }
    }

    private fun readServiceStatsJson(service: FtpForegroundService): String? {
        return withAccessibleField(service, "serverStats") { field ->
            field.get(service)?.toString()
        }
    }

    private fun readIsInForeground(service: FtpForegroundService): Boolean {
        return withAccessibleField(service, "isInForeground") { field ->
            field.getBoolean(service)
        }
    }

    private fun readWakeLock(service: FtpForegroundService): PowerManager.WakeLock? {
        return withAccessibleField(service, "wakeLock") { field ->
            field.get(service) as PowerManager.WakeLock?
        }
    }

    private fun <T> withAccessibleField(
        target: Any,
        fieldName: String,
        block: (java.lang.reflect.Field) -> T,
    ): T {
        val field = target.javaClass.getDeclaredField(fieldName)
        val wasAccessible = field.isAccessible
        field.isAccessible = true
        return try {
            block(field)
        } finally {
            field.isAccessible = wasAccessible
        }
    }

    @Implements(PowerManager::class)
    class ThrowingWakeLockPowerManagerShadow {
        @Implementation
        fun newWakeLock(_levelAndFlags: Int, _tag: String?): PowerManager.WakeLock {
            throw IllegalStateException("wake lock creation failed for test")
        }
    }

}
