package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts

/**
 * JavaScript Bridge 接口
 * 允许前端JavaScript直接调用Android方法
 */
class SAFPickerBridge(private val activity: MainActivity) {
    
    companion object {
        private const val TAG = "SAFPickerBridge"
    }
    
    private var callbackId: String? = null
    
    /**
     * 打开SAF目录选择器
     * @param initialUri 初始URI（可选）
     * @param callback JavaScript回调函数名
     */
    @JavascriptInterface
    fun openPicker(initialUri: String?, callback: String): Boolean {
        Log.d(TAG, "openPicker called from JavaScript, callback: $callback")
        callbackId = callback
        
        activity.runOnUiThread {
            activity.openSAFPicker(initialUri) { uri ->
                // 调用JavaScript回调
                val jsCode = if (uri != null) {
                    "$callback('$uri')"
                } else {
                    "$callback(null)"
                }
                
                activity.evaluateJavascript(jsCode)
            }
        }
        
        return true
    }
    
    /**
     * 检查是否拥有所有文件访问权限
     */
    @JavascriptInterface
    fun hasAllFilesAccess(): Boolean {
        return StorageHelper.hasManageExternalStoragePermission(activity)
    }
    
    /**
     * 打开权限设置页面
     */
    @JavascriptInterface
    fun openPermissionSettings(): Boolean {
        Log.d(TAG, "openPermissionSettings called from JavaScript")
        
        activity.runOnUiThread {
            StorageHelper.openManageStorageSettings(activity)
        }
        
        return true
    }
    
    /**
     * 打开所有文件访问权限设置页面
     * 直接跳转到系统设置中的权限开关页面
     */
    @JavascriptInterface
    fun openAllFilesAccessSettings(): Boolean {
        Log.d(TAG, "openAllFilesAccessSettings called from JavaScript")
        
        activity.runOnUiThread {
            StorageHelper.openManageStorageSettings(activity)
        }
        
        return true
    }
}

class MainActivity : TauriActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        @JvmStatic
        var currentActivity: MainActivity? = null
    }
    
    private var pickerCallback: ((String?) -> Unit)? = null
    private var safBridge: SAFPickerBridge? = null
    private var webViewRef: WebView? = null

    // SAF 目录选择器回调
    private val safPickerLauncher = registerForActivityResult(
        ActivityResultContracts.OpenDocumentTree()
    ) { uri: Uri? ->
        Log.d(TAG, "SAF Picker result: $uri")
        
        if (uri != null) {
            // 持久化权限
            val success = StorageHelper.persistDirectoryUri(this, uri)
            val uriString = if (success) uri.toString() else null
            pickerCallback?.invoke(uriString)
        } else {
            // 用户取消
            pickerCallback?.invoke(null)
        }
        pickerCallback = null
    }

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        currentActivity = this
        Log.d(TAG, "MainActivity created")
        
        // 初始化Bridge
        safBridge = SAFPickerBridge(this)
    }

    /**
     * WebView创建完成时调用（由WryActivity触发）
     * 这是添加JavaScript Bridge的正确时机
     */
    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        Log.d(TAG, "WebView created, setting up JavaScript Bridge")
        
        // 保存WebView引用
        webViewRef = webView
        
        // 添加JavaScript Bridge - 此时WebView已创建完成
        safBridge?.let { bridge ->
            webView.addJavascriptInterface(bridge, "SAFPickerAndroid")
            Log.d(TAG, "JavaScript Bridge 'SAFPickerAndroid' added to WebView")
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        webViewRef = null
        if (currentActivity == this) {
            currentActivity = null
        }
    }

    /**
     * 启动 SAF 目录选择器
     */
    fun openSAFPicker(initialUri: String?, callback: (String?) -> Unit) {
        Log.d(TAG, "Opening SAF picker, initialUri: $initialUri")
        
        pickerCallback = callback
        
        val uri = initialUri?.let { Uri.parse(it) }
        try {
            safPickerLauncher.launch(uri)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to launch SAF picker", e)
            callback(null)
            pickerCallback = null
        }
    }
    
    /**
     * 在WebView中执行JavaScript
     * 使用保存的WebView引用
     */
    fun evaluateJavascript(jsCode: String) {
        runOnUiThread {
            try {
                webViewRef?.evaluateJavascript(jsCode, null)
                    ?: Log.e(TAG, "WebView reference is null, cannot execute: $jsCode")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to execute JavaScript", e)
            }
        }
    }
}
