/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.os.Build
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
    fun delete_refresh_event_name_matches_frontend_listener_chain() {
        assertTrue(ImageViewerActivity.MEDIA_LIBRARY_REFRESH_REQUESTED_EVENT == "media-library-refresh-requested")
    }

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
    fun request_delete_confirmation_on_android_11_plus_for_security_exception() {
        assertTrue(
            ImageViewerActivity.shouldRequestDeleteConfirmation(
                Build.VERSION_CODES.R,
                isSecurityException = true,
                isRecoverableSecurityException = false,
            )
        )
    }

    @Test
    fun request_delete_confirmation_on_android_10_for_recoverable_security_exception() {
        assertTrue(
            ImageViewerActivity.shouldRequestDeleteConfirmation(
                Build.VERSION_CODES.Q,
                isSecurityException = true,
                isRecoverableSecurityException = true,
            )
        )
    }

    @Test
    fun do_not_request_delete_confirmation_for_non_security_failures() {
        assertFalse(
            ImageViewerActivity.shouldRequestDeleteConfirmation(
                Build.VERSION_CODES.R,
                isSecurityException = false,
                isRecoverableSecurityException = false,
            )
        )
    }
}
