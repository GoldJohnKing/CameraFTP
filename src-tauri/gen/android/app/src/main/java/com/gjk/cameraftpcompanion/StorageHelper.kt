/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build

/**
 * Android 存储辅助类
 * 提供所有文件访问权限管理功能
 */
object StorageHelper {

    /**
     * 跳转到设置页面开启所有文件访问权限
     */
    fun openManageStorageSettings(activity: Activity) {
        val packageName = activity.packageName
        val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: 跳转到应用特定的所有文件访问权限设置
            Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:$packageName")
            }
        } else {
            // Android 10 及以下: 跳转到应用信息页面
            Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.parse("package:$packageName")
            }
        }
        activity.startActivity(intent)
    }
}
