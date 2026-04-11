/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ImageViewerDeletePermissionTest {

    @Test
    fun treat_delete_as_success_when_rows_deleted() {
        assertTrue(
            ImageViewerActivity.shouldTreatDeleteAsSuccess(
                rowsDeleted = 1,
                stillExists = true,
            )
        )
    }

    @Test
    fun treat_delete_as_success_when_item_already_missing() {
        assertTrue(
            ImageViewerActivity.shouldTreatDeleteAsSuccess(
                rowsDeleted = 0,
                stillExists = false,
            )
        )
    }

    @Test
    fun treat_delete_as_failure_when_rows_not_deleted_and_still_exists() {
        assertFalse(
            ImageViewerActivity.shouldTreatDeleteAsSuccess(
                rowsDeleted = 0,
                stillExists = true,
            )
        )
    }

    @Test
    fun request_delete_confirmation_for_security_exception() {
        assertTrue(
            ImageViewerActivity.shouldRequestDeleteConfirmation(
                isSecurityException = true,
            )
        )
    }

    @Test
    fun do_not_request_delete_confirmation_for_non_security_failures() {
        assertFalse(
            ImageViewerActivity.shouldRequestDeleteConfirmation(
                isSecurityException = false,
            )
        )
    }
}
