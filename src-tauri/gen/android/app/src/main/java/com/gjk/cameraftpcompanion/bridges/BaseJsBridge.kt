/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.app.Activity
import android.util.Log

abstract class BaseJsBridge(protected val activity: Activity) {
    protected fun runOnUiThread(action: () -> Unit) {
        activity.runOnUiThread(action)
    }

    /**
     * Standard wrapper for @JavascriptInterface method bodies: runs [block],
     * and on any exception logs it (tagged [TAG], contextualized by [name])
     * and returns [fallback] instead. Callers must keep their original
     * fallback value and thread semantics — [block] runs synchronously on
     * the calling (JavaBridge) thread, exactly like the code it replaces.
     */
    protected fun <T> jsCall(name: String, fallback: T, block: () -> T): T {
        return try {
            block()
        } catch (e: Exception) {
            Log.e(TAG, name, e)
            fallback
        }
    }

    companion object {
        private const val TAG = "BaseJsBridge"
    }
}
