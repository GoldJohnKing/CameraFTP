/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import com.gjk.cameraftpcompanion.MainActivity

/**
 * Server State JavaScript Bridge
 * Forwards server state changes to the foreground service
 */
class ServerStateBridge(private val mainActivity: MainActivity) : BaseJsBridge(mainActivity) {

    /**
     * Called from JavaScript when server state changes
     * @param isRunning Whether FTP server is running
     * @param statsJson JSON string with stats (files_transferred, bytes_transferred)
     * @param connectedClients Number of connected clients
     */
    @JavascriptInterface
    fun onServerStateChanged(isRunning: Boolean, statsJson: String?, connectedClients: Int) {
        mainActivity.updateServiceState(isRunning, statsJson, connectedClients)
    }
}
