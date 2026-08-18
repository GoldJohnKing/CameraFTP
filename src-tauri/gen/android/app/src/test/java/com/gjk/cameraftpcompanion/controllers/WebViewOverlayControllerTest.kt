/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.controllers

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The native AI edit dialog must render its model dropdown from the list
 * passed in via the __tauriGetAiEditPrompt callback (single source: Rust
 * SEEDREAM_MODELS, exported to the frontend as bindings/SeedreamModels.ts)
 * — never from a hardcoded catalog.
 */
class WebViewOverlayControllerTest {

    private val customModels = listOf(
        "custom-a" to "Custom Model A",
        "custom-b" to "Custom Model B",
        "custom-c" to "Custom Model C",
    )

    @Test
    fun model_options_come_from_parameter_not_constants() {
        val selection = buildAiEditModelSelection(customModels, "custom-b")

        assertTrue(selection.optionsHtml.contains("""data-value="custom-a""""))
        assertTrue(selection.optionsHtml.contains("Custom Model A"))
        assertTrue(selection.optionsHtml.contains("""data-value="custom-c""""))
        // No hardcoded seedream catalog may leak into the rendered options.
        assertFalse(selection.optionsHtml.contains("doubao-seedream"))
    }

    @Test
    fun renders_one_option_per_supplied_model_with_selection_marker() {
        val selection = buildAiEditModelSelection(customModels, "custom-a")

        assertEquals(3, selection.optionsHtml.split("dropdown-opt").size - 1)
        assertEquals(1, selection.optionsHtml.split("dropdown-opt selected").size - 1)
    }

    @Test
    fun current_model_match_is_selected() {
        val selection = buildAiEditModelSelection(customModels, "custom-c")

        assertEquals("custom-c", selection.selectedModel)
        assertEquals("Custom Model C", selection.selectedLabel)
        assertTrue(
            selection.optionsHtml.contains(
                "<div class=\"dropdown-opt selected\" data-value=\"custom-c\">"
            )
        )
    }

    @Test
    fun unknown_current_model_falls_back_to_first_option() {
        val selection = buildAiEditModelSelection(customModels, "not-in-list")

        assertEquals("custom-a", selection.selectedModel)
        assertEquals("Custom Model A", selection.selectedLabel)
    }

    @Test
    fun empty_model_list_degrades_to_current_model_option() {
        val selection = buildAiEditModelSelection(emptyList(), "solo-model")

        assertEquals("solo-model", selection.selectedModel)
        assertEquals("solo-model", selection.selectedLabel)
        assertEquals(
            """<div class="dropdown-opt selected" data-value="solo-model">solo-model</div>""",
            selection.optionsHtml
        )
    }
}
