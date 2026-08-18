/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.Manifest
import android.app.Activity
import android.provider.Settings
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class PermissionBridgeTest {

    @Test
    fun does_not_request_manage_external_storage() {
        val perms = PermissionBridge.get_required_permissions()
        assertFalse(perms.contains("android.permission.MANAGE_EXTERNAL_STORAGE"))
    }

    @Test
    fun requests_read_media_images() {
        val perms = PermissionBridge.get_required_permissions()
        assertTrue(perms.contains("android.permission.READ_MEDIA_IMAGES"))
    }

    @Test
    fun requests_read_media_visual_user_selected_for_android14_plus() {
        val perms = PermissionBridge.get_required_permissions()
        assertTrue(perms.contains("android.permission.READ_MEDIA_VISUAL_USER_SELECTED"))
    }

    @Test
    fun does_not_request_write_external_storage() {
        val perms = PermissionBridge.get_required_permissions()
        assertFalse(perms.contains("android.permission.WRITE_EXTERNAL_STORAGE"))
    }

    @Test
    fun builds_app_permission_settings_intent() {
        val intent = PermissionBridge.build_app_permission_settings_intent("com.example.app")
        assertEquals(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, intent.action)
        assertEquals("package:com.example.app", intent.dataString)
    }

    @Test
    fun opens_settings_only_for_partial_access() {
        assertTrue(PermissionBridge.should_open_settings_for_storage_request(false, true))
    }

    @Test
    fun does_not_open_settings_for_denied_access() {
        assertFalse(PermissionBridge.should_open_settings_for_storage_request(false, false))
    }

    @Test
    fun does_not_open_settings_when_full_access_exists() {
        assertFalse(PermissionBridge.should_open_settings_for_storage_request(true, false))
        assertFalse(PermissionBridge.should_open_settings_for_storage_request(true, true))
    }

    // ── Storage permission tri-state (full / partial / denied) ──────────
    //
    // checkStoragePermission()/hasPartialStoragePermission() combine the two
    // runtime permissions; the tri-state outcome is observed through the
    // behavior of requestStoragePermission() (no-op / settings / runtime).

    private fun newBridge(): Pair<Activity, PermissionBridge> {
        val activity = Robolectric.buildActivity(Activity::class.java).create().get()
        return activity to PermissionBridge(activity)
    }

    @Test
    fun storage_tri_state_full_grant_reports_granted_and_request_is_noop() {
        val (activity, bridge) = newBridge()
        shadowOf(activity).grantPermissions(
            Manifest.permission.READ_MEDIA_IMAGES,
            Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
        )

        assertTrue(
            "Full READ_MEDIA_IMAGES grant must count as storage granted",
            bridge.checkStoragePermission()
        )

        bridge.requestStoragePermission()

        assertNull(
            "No runtime permission request may be issued when fully granted",
            shadowOf(activity).lastRequestedPermission
        )
        assertNull(
            "No settings screen may be opened when fully granted",
            shadowOf(activity).nextStartedActivity
        )
    }

    @Test
    fun storage_tri_state_partial_grant_reports_not_granted_and_opens_settings() {
        val (activity, bridge) = newBridge()
        shadowOf(activity).grantPermissions(Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)
        shadowOf(activity).denyPermissions(Manifest.permission.READ_MEDIA_IMAGES)

        assertFalse(
            "Partial (selected-photos-only) access must not count as full storage permission",
            bridge.checkStoragePermission()
        )

        bridge.requestStoragePermission()

        val started = shadowOf(activity).nextStartedActivity
        assertNotNull(
            "Partial access must route to the app permission settings screen",
            started
        )
        assertEquals(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, started!!.action)
        assertEquals("package:${activity.packageName}", started.dataString)
        assertNull(
            "No runtime permission request may be issued for partial access",
            shadowOf(activity).lastRequestedPermission
        )
    }

    @Test
    fun storage_tri_state_denied_reports_not_granted_and_requests_runtime_permissions() {
        val (activity, bridge) = newBridge()
        shadowOf(activity).denyPermissions(
            Manifest.permission.READ_MEDIA_IMAGES,
            Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED
        )

        assertFalse(
            "No grants must not count as storage granted",
            bridge.checkStoragePermission()
        )

        bridge.requestStoragePermission()

        val request = shadowOf(activity).lastRequestedPermission
        assertNotNull(
            "Denied access must trigger a runtime permission request",
            request
        )
        assertEquals(
            PermissionBridge.REQUEST_STORAGE_PERMISSIONS,
            request!!.requestCode
        )
        assertArrayEquals(
            PermissionBridge.get_required_permissions().toTypedArray(),
            request.requestedPermissions
        )
        // Robolectric surfaces the runtime request itself as a started
        // activity (the permission controller). The tri-state contract only
        // forbids the *app settings* screen for plain denied access.
        val started = shadowOf(activity).nextStartedActivity
        if (started != null) {
            assertNotEquals(
                "App settings screen is reserved for partial access, not denied access",
                Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                started.action
            )
        }
    }

}
