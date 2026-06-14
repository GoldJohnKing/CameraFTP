/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ColorGradingJniBridgeTest {

    // Tests use JniResultParser directly to avoid triggering
    // ColorGradingJniBridge's System.loadLibrary static init.

    @Test
    fun parseResult_okTrue_returnsSuccess() {
        val result = JniResultParser.parseResult("""{"ok":true}""")
        assertTrue(result.isSuccess)
    }

    @Test
    fun parseResult_okFalse_returnsFailureWithMessage() {
        val result = JniResultParser.parseResult("""{"ok":false,"error":"something went wrong"}""")
        assertTrue(result.isFailure)
        assertEquals("something went wrong", result.exceptionOrNull()?.message)
    }

    @Test
    fun parseResult_missingOk_returnsFailure() {
        val result = JniResultParser.parseResult("""{"error":"no ok field"}""")
        assertTrue(result.isFailure)
    }

    @Test(expected = org.json.JSONException::class)
    fun parseResult_malformedJson_throwsJSONException() {
        JniResultParser.parseResult("""not json""")
    }

    @Test
    fun parseResult_okFalseEmptyError_returnsDefaultMessage() {
        val result = JniResultParser.parseResult("""{"ok":false}""")
        assertTrue(result.isFailure)
        assertEquals("Unknown error", result.exceptionOrNull()?.message)
    }

    @Test
    fun parseResultWithBuffer_validBytes_returnsSameBytes() {
        val bytes = byteArrayOf(0x01, 0x02, 0x03)
        val result = JniResultParser.parseResultWithBuffer(bytes)
        assertTrue(result.isSuccess)
        assertArrayEquals(bytes, result.getOrNull())
    }

    @Test
    fun parseResultWithBuffer_emptyBytes_returnsFailure() {
        val result = JniResultParser.parseResultWithBuffer(byteArrayOf())
        assertTrue(result.isFailure)
    }

    @Test
    fun parseResultWithBuffer_nullBytes_returnsFailure() {
        val result = JniResultParser.parseResultWithBuffer(null)
        assertTrue(result.isFailure)
    }
}
