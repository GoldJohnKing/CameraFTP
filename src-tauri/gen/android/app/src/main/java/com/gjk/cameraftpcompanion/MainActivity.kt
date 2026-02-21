package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.ActivityResult
import androidx.activity.result.contract.ActivityResultContracts

class MainActivity : TauriActivity() {

    companion object {
        // 存储选择的回调，供 Rust 调用
        @JvmStatic
        var directoryPickerCallback: ((String?) -> Unit)? = null

        // 静态引用当前 Activity
        @JvmStatic
        var currentActivity: MainActivity? = null
    }

    // SAF 目录选择器回调
    private val safPickerLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        if (uri != null) {
            // 持久化权限和 URI
            val success = StorageHelper.persistDirectoryUri(this, uri)
            val uriString = if (success) uri.toString() else null

            // 发送结果给前端/JavaScript
            emitSAFPickerResult(uriString)

            // 同时回调传统方式
            directoryPickerCallback?.invoke(uriString)
        } else {
            // 用户取消了选择
            emitSAFPickerResult(null)
            directoryPickerCallback?.invoke(null)
        }
    }

    // 传统方式的 SAF 选择器
    private val directoryPickerLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result: ActivityResult ->
        if (result.resultCode == Activity.RESULT_OK) {
            val uri = result.data?.data
            if (uri != null) {
                val success = StorageHelper.persistDirectoryUri(this, uri)
                if (success) {
                    directoryPickerCallback?.invoke(uri.toString())
                } else {
                    directoryPickerCallback?.invoke(null)
                }
            } else {
                directoryPickerCallback?.invoke(null)
            }
        } else {
            directoryPickerCallback?.invoke(null)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        currentActivity = this
    }

    override fun onDestroy() {
        super.onDestroy()
        if (currentActivity == this) {
            currentActivity = null
        }
    }

    /**
     * 启动 SAF 目录选择器
     */
    fun openSAFPicker(initialUri: String? = null) {
        val uri = initialUri?.let { Uri.parse(it) }
        safPickerLauncher.launch(uri)
    }

    /**
     * 启动 SAF 目录选择器（传统方式）
     */
    fun openDirectoryPicker(callback: (String?) -> Unit) {
        directoryPickerCallback = callback
        val intent = StorageHelper.createDirectoryPickerIntent()
        directoryPickerLauncher.launch(intent)
    }

    /**
     * 发送 SAF 选择器结果给前端
     * 通过 Tauri 事件桥
     */
    private fun emitSAFPickerResult(uri: String?) {
        // 使用 Tauri 的事件系统发送结果给前端
        // 前端需要监听 "saf-picker-result" 事件
        val intent = Intent("saf-picker-result").apply {
            putExtra("uri", uri)
        }
        sendBroadcast(intent)
    }

    /**
     * 检查是否有所有文件访问权限
     */
    fun hasAllFilesAccessPermission(): Boolean {
        return StorageHelper.hasManageExternalStoragePermission(this)
    }

    /**
     * 跳转到设置页面开启所有文件访问权限
     */
    fun openAllFilesAccessSettings() {
        StorageHelper.openManageStorageSettings(this)
    }
}
