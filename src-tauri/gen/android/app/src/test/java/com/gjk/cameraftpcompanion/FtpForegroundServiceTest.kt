/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import androidx.test.core.app.ApplicationProvider
import java.io.File
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
            notification = shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID)
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
    @Config(sdk = [36], manifest = Config.NONE)
    fun onTimeout_overloads_stop_service_now() {
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

            AndroidServiceStateCoordinator.syncNativeServiceState(
                context,
                true,
                "{\"isRunning\":true,\"connectedClients\":1,\"filesReceived\":2,\"bytesReceived\":1024,\"lastFile\":null}",
                1,
            )
            service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_START
                },
                0,
                3,
            )

            service.onTimeout(4, 0)
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
    fun service_source_handles_timeout_and_explicit_foreground_stop() {
        val source = readServiceSource()

        assertTrue(source.contains("override fun onTimeout(startId: Int, fgsType: Int)"))
        assertTrue(source.contains("override fun onTimeout(startId: Int)"))
        assertTrue(source.contains("Log.w(TAG, \"onTimeout(startId): startId=${'$'}startId\")"))
        assertTrue(source.contains("AndroidServiceStateCoordinator.clearState()"))
        assertTrue(source.contains("stopForegroundServiceNow(\"fgs timeout startId=${'$'}startId\")"))
        assertTrue(source.contains("Log.w(TAG, \"onTimeout(startId, fgsType): startId=${'$'}startId, fgsType=${'$'}fgsType\")"))
        assertTrue(source.contains("stopForegroundServiceNow(\"fgs timeout startId=${'$'}startId type=${'$'}fgsType\")"))
        assertTrue(source.contains("if (intent?.action == ACTION_STOP) {"))
        assertTrue(source.contains("stopForegroundServiceNow(\"explicit stop action\")"))
        assertTrue(source.contains("if (isInForeground) {"))
        assertTrue(source.contains("ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)"))
    }

    @Test
    fun service_source_wraps_lock_acquire_and_release_defensively() {
        val source = readServiceSource()

        assertTrue(source.contains("runCatching {"))
        assertTrue(source.contains("val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager"))
        assertTrue(source.contains("wakeLock = nextWakeLock"))
        assertTrue(source.contains("nextWakeLock.acquire()"))
        assertTrue(source.contains("Failed to acquire wake lock"))
        assertTrue(source.contains("val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager"))
        assertTrue(source.contains("wifiLock = nextWifiLock"))
        assertTrue(source.contains("nextWifiLock.acquire()"))
        assertTrue(source.contains("Failed to acquire wifi lock"))
        assertTrue(source.contains("if (it.isHeld)"))
        assertTrue(source.contains("it.release()"))
        assertTrue(source.contains("Failed to release wake lock"))
        assertTrue(source.contains("Failed to release wifi lock"))
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

    private fun readServiceSource(): String {
        return File(
            "src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt"
        ).readText()
    }
}
