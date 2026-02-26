package com.gjk.cameraftpcompanion.bridges

import android.app.Activity

abstract class BaseJsBridge(protected val activity: Activity) {
    protected fun runOnUiThread(action: () -> Unit) {
        activity.runOnUiThread(action)
    }
}
