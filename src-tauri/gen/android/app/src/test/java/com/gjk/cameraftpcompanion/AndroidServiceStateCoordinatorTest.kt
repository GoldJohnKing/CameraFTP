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
import javax.xml.parsers.DocumentBuilderFactory
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
class AndroidServiceStateCoordinatorTest {

    @Test
    fun manifest_declares_connected_device_foreground_service_type() {
        val manifestPath = Paths.get("app/src/main/AndroidManifest.xml")
        val manifest = Files.readString(manifestPath)
        val androidNamespace = "http://schemas.android.com/apk/res/android"
        val document = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }
            .newDocumentBuilder()
            .parse(manifestPath.toFile())
        val serviceNodes = document.getElementsByTagName("service")
        val ftpServiceNode = (0 until serviceNodes.length).map { index -> serviceNodes.item(index) }.firstOrNull { node ->
            node.attributes?.getNamedItemNS(androidNamespace, "name")?.nodeValue == ".FtpForegroundService"
        }

        assertTrue(manifest.contains("android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE"))
        assertTrue(manifest.contains("android.permission.CHANGE_WIFI_STATE"))
        assertTrue(manifest.contains("android.permission.CHANGE_NETWORK_STATE"))
        assertFalse(manifest.contains("android.permission.FOREGROUND_SERVICE_DATA_SYNC"))
        assertNotNull(ftpServiceNode)
        assertEquals(
            "connectedDevice",
            ftpServiceNode?.attributes?.getNamedItemNS(androidNamespace, "foregroundServiceType")?.nodeValue,
        )
    }

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
        val application = getApplicationContext<android.app.Application>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(application).clearStartedServices()

        AndroidServiceStateCoordinator.syncNativeServiceState(context, false, null, 0)

        val snapshot = AndroidServiceStateCoordinator.getLatestState()
        val stopIntent = shadowOf(application).nextStartedService
        assertFalse(snapshot.isRunning)
        assertNull(snapshot.statsJson)
        assertEquals(0, snapshot.connectedClients)
        assertEquals(FtpForegroundService::class.java.name, stopIntent.component?.className)
        assertEquals(FtpForegroundService.ACTION_STOP, stopIntent.action)
    }

    @Test
    fun repeated_running_updates_do_not_restart_service_when_instance_exists() {
        val context = getApplicationContext<Context>()
        val application = getApplicationContext<android.app.Application>()

        AndroidServiceStateCoordinator.clearState()
        AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":1}", 2)
        shadowOf(application).clearStartedServices()

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
            withCompanionInstance(service) {
                AndroidServiceStateCoordinator.syncNativeServiceState(context, true, "{\"files_transferred\":2}", 3)

                val snapshot = AndroidServiceStateCoordinator.getLatestState()
                val restartedIntent = shadowOf(application).nextStartedService
                assertTrue(snapshot.isRunning)
                assertEquals(3, snapshot.connectedClients)
                assertEquals("{\"files_transferred\":2}", snapshot.statsJson)
                assertNull(restartedIntent)
            }
        } finally {
            serviceController.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
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

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
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
        } finally {
            serviceController.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    @Test
    fun stale_start_intent_does_not_restart_service_when_snapshot_is_stopped() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val controller = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
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
            assertFalse(readIsInForeground(service))
            assertNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
        } finally {
            controller.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    @Test
    fun explicit_stop_action_stops_service_and_clears_state() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val notificationManager =
            context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val controller = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = controller.get()

            AndroidServiceStateCoordinator.syncNativeServiceState(
                context,
                true,
                "{\"filesReceived\":2,\"bytesReceived\":2048}",
                3,
            )
            service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_START
                },
                0,
                1,
            )

            val result = service.onStartCommand(
                Intent(context, FtpForegroundService::class.java).apply {
                    action = FtpForegroundService.ACTION_STOP
                },
                0,
                2,
            )

            assertEquals(Service.START_NOT_STICKY, result)
            assertEquals(0, readConnectedClients(service))
            assertNull(readServiceStatsJson(service))
            assertFalse(readIsInForeground(service))
            assertNull(shadowOf(notificationManager).getNotification(FtpForegroundService.NOTIFICATION_ID))
        } finally {
            controller.destroy()
            AndroidServiceStateCoordinator.clearState()
        }
    }

    @Test
    fun service_source_uses_connected_device_foreground_runtime_type() {
        val sourcePath = Paths.get("src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt")
        val source = Files.readString(sourcePath)

        assertTrue(source.contains("const val ACTION_STOP = \"com.gjk.cameraftpcompanion.STOP_SERVICE\""))
        assertTrue(source.contains("ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE"))
        assertFalse(source.contains("ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC"))
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

        val serviceController = Robolectric.buildService(FtpForegroundService::class.java).create()
        try {
            val service = serviceController.get()
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

    private fun <T> withAccessibleField(
        targetClass: Class<*>,
        fieldName: String,
        block: (java.lang.reflect.Field) -> T,
    ): T {
        val field = targetClass.getDeclaredField(fieldName)
        val wasAccessible = field.isAccessible
        field.isAccessible = true
        return try {
            block(field)
        } finally {
            field.isAccessible = wasAccessible
        }
    }

    private fun withCompanionInstance(service: FtpForegroundService, block: () -> Unit) {
        withAccessibleField(FtpForegroundService::class.java, "instance") { field ->
            field.set(null, service)
            try {
                block()
            } finally {
                field.set(null, null)
            }
        }
    }

    private fun <T> withAccessibleField(
        target: Any,
        fieldName: String,
        block: (java.lang.reflect.Field) -> T,
    ): T {
        return withAccessibleField(target.javaClass, fieldName, block)
    }
}
