package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.StorageHelper

class StorageSettingsBridge(activity: MainActivity) : BaseJsBridge(activity) {

    /**
     * 打开"所有文件访问权限"设置页面
     * 直接跳转到系统设置中的权限开关页面
     */
    @JavascriptInterface
    fun openAllFilesAccessSettings(): Boolean {
        runOnUiThread {
            StorageHelper.openManageStorageSettings(activity)
        }

        return true
    }
}
