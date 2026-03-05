/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.app.Activity

abstract class BaseJsBridge(protected val activity: Activity) {
    protected fun runOnUiThread(action: () -> Unit) {
        activity.runOnUiThread(action)
    }
}
