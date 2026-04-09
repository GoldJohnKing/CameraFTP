/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ImageViewerBridgeDeadCodeTest {
    @Test
    fun `openViewer delegate is removed`() {
        val sourceFile = resolveSourceFile(
            "src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt"
        )
        val source = sourceFile.readText()
        assertFalse(
            "openViewer() delegate should be removed — use openOrNavigateTo directly",
            source.contains("fun openViewer(uri: String, allUrisJson: String)")
        )
    }

    private fun resolveSourceFile(relativePath: String): File {
        val candidates = listOf(File(relativePath), File("app/$relativePath"))
        return candidates.first { it.exists() }
    }
}
