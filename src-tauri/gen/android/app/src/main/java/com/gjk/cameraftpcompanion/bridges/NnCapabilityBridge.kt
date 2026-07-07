/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.os.Build
import android.util.Log
import android.webkit.JavascriptInterface
import org.json.JSONObject

/**
 * Detects whether the device's SoC is on the Qualcomm Hexagon v73+
 * whitelist eligible for FP16 NN demosaic on HTP.
 *
 * Exposed to the WebView as `window.NnCapability`; the frontend reads
 * [getNnEnabled] and forwards the result to the Rust NN service, which
 * uses it to gate the QNN code path.
 */
class NnCapabilityBridge {
    companion object {
        private const val TAG = "NnCapabilityBridge"

        // Hexagon v73+ (SD 8 Gen 2 and newer) — FP16-on-HTP supported.
        // Shared with MainActivity so the QNN/ORT native-library load gate
        // and the JS-facing probe always agree on the same device set.
        val HEXAGON_V73_PLUS: Set<String> = setOf(
            "SM8550", "SM8650", "SM8750", "SM8845", "SM8850"
        )

        /**
         * True iff [Build.SOC_MODEL] is on the NN-capable whitelist.
         * Reads only a static system property; safe to call off the main thread.
         */
        fun isNnCapable(): Boolean = Build.SOC_MODEL in HEXAGON_V73_PLUS
    }

    /**
     * JS-facing capability probe.
     *
     * @return JSON `{"enabled":Boolean,"socModel":String}` parsed by the frontend.
     */
    @JavascriptInterface
    fun getNnEnabled(): String {
        // Build.SOC_MODEL is a platform type; fall back defensively so
        // JSONObject.put never receives a null value (which would throw).
        val socModel = Build.SOC_MODEL ?: "unknown"
        val enabled = socModel in HEXAGON_V73_PLUS
        Log.d(TAG, "SoC=$socModel, NN enabled=$enabled")
        val json = JSONObject()
        json.put("enabled", enabled)
        json.put("socModel", socModel)
        return json.toString()
    }
}
