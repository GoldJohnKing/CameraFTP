/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.content.Context
import android.content.pm.ApplicationInfo
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
        // Pin the exact root hit: if a broader root (e.g. <external-path>)
        // were re-added and matched first, the URI would still resolve but
        // under a different root name.
        assertEquals("external_files", uri.pathSegments.firstOrNull())
        assertEquals("content", uri.scheme)
    }

    @Test
    fun files_dir_file_is_shareable() {
        // Positive pin: the <files-path> root must keep resolving so
        // internal app files remain shareable.
        val file = File(context().filesDir, "x.bin")
        file.writeBytes(byteArrayOf(0x00))

        val uri = FileProvider.getUriForFile(context(), authority, file)

        assertNotNull(uri)
        assertEquals("app_files", uri.pathSegments.firstOrNull())
        assertEquals("content", uri.scheme)
    }

    @Test
    fun cache_dir_file_is_shareable() {
        // Positive pin: the <cache-path> root must keep resolving.
        val file = File(context().cacheDir, "x.bin")
        file.writeBytes(byteArrayOf(0x00))

        val uri = FileProvider.getUriForFile(context(), authority, file)

        assertNotNull(uri)
        assertEquals("cache", uri.pathSegments.firstOrNull())
        assertEquals("content", uri.scheme)
    }

    @Test
    fun manifest_disallows_backup() {
        // Robolectric reads the merged manifest: the APK must not opt into
        // auto-backup of app data (including any cached transfer state).
        // ApplicationInfo.FLAG_ALLOW_BACKUP is a constant since API 1.
        assertEquals(
            0,
            context().applicationInfo.flags and ApplicationInfo.FLAG_ALLOW_BACKUP,
        )
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
