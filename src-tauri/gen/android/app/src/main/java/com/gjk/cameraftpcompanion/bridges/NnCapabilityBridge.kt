/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.os.Build

/**
 * Detects whether the device's SoC is on the Qualcomm Hexagon v73+
 * whitelist eligible for FP16 NN demosaic on HTP.
 *
 * Not exposed to the WebView; MainActivity gates the QNN/ORT native-library
 * preload on [Companion.isNnCapable].
 */
class NnCapabilityBridge {
    companion object {
        // Hexagon v73+ (SD 8 Gen 2 and newer) — FP16-on-HTP supported.
        // Shared with MainActivity so the QNN/ORT native-library load gate
        // always agrees on the same device set.
        val HEXAGON_V73_PLUS: Set<String> = setOf(
            "SM8550", "SM8650", "SM8750", "SM8845", "SM8850"
        )

        /**
         * True iff [Build.SOC_MODEL] is on the NN-capable whitelist.
         * Reads only a static system property; safe to call off the main thread.
         */
        fun isNnCapable(): Boolean = Build.SOC_MODEL in HEXAGON_V73_PLUS
    }
}
