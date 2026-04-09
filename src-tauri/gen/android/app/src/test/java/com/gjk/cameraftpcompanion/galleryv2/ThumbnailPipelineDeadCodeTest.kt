package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ThumbnailPipelineDeadCodeTest {
    @Test
    fun `write-only counters are removed`() {
        val sourceFile = resolveSourceFile(
            "src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt"
        )
        val source = sourceFile.readText()
        assertFalse(
            "totalRequests write-only counter should be removed",
            source.contains("private var totalRequests")
        )
        assertFalse(
            "cacheHits write-only counter should be removed",
            source.contains("private var cacheHits")
        )
    }

    private fun resolveSourceFile(relativePath: String): File {
        val candidates = listOf(File(relativePath), File("app/$relativePath"))
        return candidates.first { it.exists() }
    }
}
