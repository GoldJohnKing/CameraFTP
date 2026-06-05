/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ColorGradingActivitySaveTest {

    private fun createActivity(): ColorGradingActivity {
        val intent = android.content.Intent().apply {
            putExtra("filePath", "/test/photo.NEF")
        }
        return org.robolectric.Robolectric.buildActivity(ColorGradingActivity::class.java, intent).get()
    }

    @Test
    fun scanOutputFile_doesNotThrowForValidPath() {
        val activity = createActivity()
        val path = "/storage/emulated/0/DCIM/ColorGrading/test_provia_20260605_120000.jpg"
        activity.scanOutputFile(path)
    }

    @Test
    fun scanOutputFile_doesNotThrowForEmptyPath() {
        val activity = createActivity()
        activity.scanOutputFile("")
    }
}
