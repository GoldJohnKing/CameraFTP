/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.content.Context
import android.os.Environment
import androidx.core.content.FileProvider
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class FileProviderPathsTest {

    private val authority = "com.gjk.cameraftpcompanion.fileprovider"

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    @Before
    fun resetFileProviderStrategyCache() {
        // Robolectric gives each test method its own tmp-dir environment
        // (external storage, files dirs), but androidx FileProvider caches
        // parsed PathStrategy roots in a static map keyed only by authority.
        // Without clearing it, a strategy parsed in an earlier test method
        // would point at that method's tmp dirs and never match this
        // method's paths ("Failed to find configured root").
        val cacheField = FileProvider::class.java.getDeclaredField("sCache")
        cacheField.isAccessible = true
        (cacheField.get(null) as MutableMap<*, *>).clear()
    }

    @Test
    fun camera_capture_temp_file_is_shareable() {
        // RustWebChromeClient.createImageFile builds its capture temp file in
        // getExternalFilesDir(DIRECTORY_PICTURES) — the FileProvider must
        // resolve exactly that root.
        val dir = context().getExternalFilesDir(Environment.DIRECTORY_PICTURES)!!
        dir.mkdirs()
        val photo = File(dir, "JPEG_20260916_000000.jpg")
        photo.writeBytes(byteArrayOf(0xFF.toByte()))

        val uri = FileProvider.getUriForFile(context(), authority, photo)

        assertNotNull(uri)
        assertEquals("content", uri.scheme)
    }

    @Test
    fun shared_storage_roots_are_not_exposed() {
        // The over-broad <external-path> roots must be gone: a file on shared
        // storage must NOT be convertible into a grantable content URI.
        val outsider = File(Environment.getExternalStorageDirectory(), "DCIM/leak.jpg")
        try {
            FileProvider.getUriForFile(context(), authority, outsider)
            fail("external storage root must not be shareable via FileProvider")
        } catch (expected: IllegalArgumentException) {
            // "Failed to find configured root" — expected after narrowing
        }
    }
}
