/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class GalleryDeletePermissionTest {

    @Test
    fun request_delete_confirmation_for_security_exception() {
        assertTrue(
            GalleryBridge.shouldRequestDeleteConfirmation(
                isSecurityException = true,
            )
        )
    }

    @Test
    fun do_not_request_delete_confirmation_for_other_failures() {
        assertFalse(
            GalleryBridge.shouldRequestDeleteConfirmation(
                isSecurityException = false,
            )
        )
    }
}
