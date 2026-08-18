/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Product ruling (#4), native side: the AI edit dialog default must come
 * from dialogPrompt (manualPrompt) only — it never falls back to the
 * auto-edit prompt. autoPrompt (aiEdit.prompt) belongs exclusively to the
 * no-UI automatic path and must never surface as a dialog default.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class ImageViewerAiEditDialogContextTest {

    @Test
    fun dialog_default_uses_dialogPrompt_only_without_auto_fallback() {
        val json = """
            {"dialogPrompt":"","autoPrompt":"auto-edit prompt","model":"m1",
             "autoEdit":true,"hasApiKey":true,"models":[]}
        """.trimIndent()

        val context = parseAiEditDialogContext(json)!!

        // Ruling: empty manualPrompt keeps an empty dialog default — the
        // auto-edit prompt must NOT leak into the dialog.
        assertEquals("", context.dialogPrompt)
        assertEquals("auto-edit prompt", context.autoPrompt)
    }

    @Test
    fun dialog_default_carries_manual_prompt_with_unescaped_newlines() {
        val json = """
            {"dialogPrompt":"line1\\nline2","autoPrompt":"","model":"m1",
             "autoEdit":false,"hasApiKey":true,"models":[]}
        """.trimIndent()

        val context = parseAiEditDialogContext(json)!!

        assertEquals("line1\nline2", context.dialogPrompt)
    }

    @Test
    fun missing_keys_degrade_to_safe_defaults() {
        val context = parseAiEditDialogContext("{}")!!

        assertEquals("", context.dialogPrompt)
        assertEquals("", context.autoPrompt)
        assertEquals("", context.model)
        assertEquals(false, context.autoEdit)
        assertEquals(true, context.hasApiKey)
        assertEquals(emptyList<Pair<String, String>>(), context.models)
    }

    @Test
    fun invalid_json_returns_null() {
        assertNull(parseAiEditDialogContext("not-json"))
    }

    @Test
    fun model_entries_skip_empty_values_and_fallback_labels_to_value() {
        val json = """
            {"dialogPrompt":"p","model":"m1","models":[
               {"value":"","label":"dropped"},
               {"value":"m1","label":""},
               {"value":"m2","label":"Model 2"}
             ]}
        """.trimIndent()

        val context = parseAiEditDialogContext(json)!!

        assertEquals(
            listOf("m1" to "m1", "m2" to "Model 2"),
            context.models,
        )
    }
}
