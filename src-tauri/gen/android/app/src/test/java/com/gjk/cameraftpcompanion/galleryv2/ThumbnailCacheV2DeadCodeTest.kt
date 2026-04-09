package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ThumbnailCacheV2DeadCodeTest {
    @Test
    fun `deleteDiskEntries is removed`() {
        val sourceFile = resolveSourceFile(
            "src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt"
        )
        val source = sourceFile.readText()
        assertFalse(
            "deleteDiskEntries should be removed — it is never called",
            source.contains("fun deleteDiskEntries")
        )
    }

    private fun resolveSourceFile(relativePath: String): File {
        val candidates = listOf(File(relativePath), File("app/$relativePath"))
        return candidates.first { it.exists() }
    }
}
