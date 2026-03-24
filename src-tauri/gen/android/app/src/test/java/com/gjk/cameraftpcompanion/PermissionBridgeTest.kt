/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.provider.Settings
import org.junit.Test
import org.junit.Assert.*

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
}
