package com.gjk.cameraftpcompanion

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
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
}

class MainActivity : TauriActivity() {
    
    companion object {
        private const val TAG = "MainActivity"
        @JvmStatic
        var currentActivity: MainActivity? = null
    }
    
    private var pickerCallback: ((String?) -> Unit)? = null
    private var safBridge: SAFPickerBridge? = null

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
        
        // 添加JavaScript Bridge
        safBridge = SAFPickerBridge(this)
        webView?.addJavascriptInterface(safBridge!!, "SAFPickerAndroid")
        Log.d(TAG, "JavaScript Bridge added")
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
     */
    fun evaluateJavascript(jsCode: String) {
        webView?.evaluateJavascript(jsCode, null)
    }
    
    /**
     * 获取WebView实例（供bridge使用）
     */
    val webView: android.webkit.WebView?
        get() {
            // 在TauriActivity中找到WebView
            return findWebView(this)
        }
    
    private fun findWebView(activity: Activity): android.webkit.WebView? {
        val rootView = activity.window.decorView.rootView as? android.view.ViewGroup
        return findWebViewInViewGroup(rootView)
    }
    
    private fun findWebViewInViewGroup(viewGroup: android.view.ViewGroup?): android.webkit.WebView? {
        if (viewGroup == null) return null
        
        for (i in 0 until viewGroup.childCount) {
            val child = viewGroup.getChildAt(i)
            if (child is android.webkit.WebView) {
                return child
            }
            if (child is android.view.ViewGroup) {
                val result = findWebViewInViewGroup(child)
                if (result != null) return result
            }
        }
        return null
    }
}
