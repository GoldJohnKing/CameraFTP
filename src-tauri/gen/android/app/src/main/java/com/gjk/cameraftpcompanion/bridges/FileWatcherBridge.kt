package com.gjk.cameraftpcompanion.bridges

import android.os.FileObserver
import android.util.Log
import android.webkit.JavascriptInterface
import java.io.File

/**
 * 文件系统监听 Bridge（Android 平台）
 *
 * 使用 FileObserver 监听存储目录的文件变化
 * 事件通过 JS Bridge 传递给前端，再调用 Rust 命令同步索引
 */
class FileWatcherBridge(activity: android.app.Activity) : BaseJsBridge(activity) {

    companion object {
        private const val TAG = "FileWatcherBridge"

        // 支持的图片扩展名
        private val SUPPORTED_EXTENSIONS = setOf("jpg", "jpeg", "heif", "hif", "heic")
    }

    private var fileObserver: RecursiveFileObserver? = null
    private var isWatching = false

    /**
     * 开始监听指定路径
     *
     * @param path 要监听的目录路径
     * @return 是否成功启动监听
     */
    @JavascriptInterface
    fun startWatching(path: String): Boolean {
        Log.d(TAG, "Starting file watcher for: $path")

        if (isWatching) {
            Log.w(TAG, "Already watching, stopping previous watcher")
            stopWatching()
        }

        val directory = File(path)
        if (!directory.exists() || !directory.isDirectory) {
            Log.e(TAG, "Invalid watch path: $path")
            return false
        }

        return try {
            fileObserver = RecursiveFileObserver(path) { event, filePath ->
                handleFileEvent(event, filePath)
            }
            fileObserver?.startWatching()
            isWatching = true
            Log.i(TAG, "File watcher started successfully")
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start file watcher", e)
            false
        }
    }

    /**
     * 停止监听
     */
    @JavascriptInterface
    fun stopWatching() {
        Log.d(TAG, "Stopping file watcher")
        fileObserver?.stopWatching()
        fileObserver = null
        isWatching = false
        Log.i(TAG, "File watcher stopped")
    }

    /**
     * 检查是否正在监听
     */
    @JavascriptInterface
    fun isWatching(): Boolean {
        return isWatching
    }

    /**
     * 处理文件系统事件
     */
    private fun handleFileEvent(event: Int, path: String?) {
        if (path == null) return

        // 只处理支持的图片格式
        if (!isSupportedImage(path)) {
            return
        }

        when (event) {
            FileObserver.CREATE, FileObserver.MODIFY -> {
                Log.d(TAG, "File created/modified: $path")
                notifyRust("created", path)
            }
            FileObserver.DELETE, FileObserver.MOVED_FROM -> {
                Log.d(TAG, "File deleted/moved out: $path")
                notifyRust("deleted", path)
            }
            FileObserver.MOVED_TO -> {
                Log.d(TAG, "File moved in: $path")
                notifyRust("created", path)
            }
            FileObserver.CLOSE_WRITE -> {
                // 文件写入完成，可能需要重新读取
                Log.d(TAG, "File write completed: $path")
                notifyRust("modified", path)
            }
        }
    }

    /**
     * 通知 Rust 后端文件事件
     */
    private fun notifyRust(eventType: String, filePath: String) {
        runOnUiThread {
            try {
                // 通过 JS 调用 Rust 命令
                val js = """
                    (async () => {
                        try {
                            if (window.__TAURI__?.core?.invoke) {
                                await window.__TAURI__.core.invoke('handle_file_system_event', {
                                    eventType: '$eventType',
                                    path: '$filePath'
                                });
                            }
                        } catch (e) {
                            console.error('Failed to notify Rust of file event:', e);
                        }
                    })();
                """.trimIndent()

                // 尝试从 activity 获取 WebView
                val mainActivity = activity as? com.gjk.cameraftpcompanion.MainActivity
                val webView = mainActivity?.getWebView()
                    ?: findWebViewInActivity()

                webView?.evaluateJavascript(js, null)
                    ?: Log.w(TAG, "Cannot evaluate JS: WebView not found")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to notify Rust", e)
            }
        }
    }

    /**
     * 在 activity 中查找 WebView
     */
    private fun findWebViewInActivity(): android.webkit.WebView? {
        return try {
            // 尝试通过反射或其他方式获取 WebView
            // 这里简化处理，直接尝试从 decorView 查找
            val decorView = activity.window?.decorView
            findWebViewRecursive(decorView as? android.view.ViewGroup)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to find WebView", e)
            null
        }
    }

    /**
     * 递归查找 WebView
     */
    private fun findWebViewRecursive(parent: android.view.ViewGroup?): android.webkit.WebView? {
        if (parent == null) return null

        for (i in 0 until parent.childCount) {
            val child = parent.getChildAt(i)
            if (child is android.webkit.WebView) {
                return child
            }
            if (child is android.view.ViewGroup) {
                val found = findWebViewRecursive(child)
                if (found != null) return found
            }
        }
        return null
    }

    /**
     * 检查是否是支持的图片格式
     */
    private fun isSupportedImage(path: String): Boolean {
        val extension = path.substringAfterLast('.', "").lowercase()
        return extension in SUPPORTED_EXTENSIONS
    }

    /**
     * 递归文件监听器
     * 
     * 注意：FileObserver 本身不递归，需要为每个子目录创建观察者
     */
    private class RecursiveFileObserver(
        private val rootPath: String,
        private val callback: (Int, String) -> Unit
    ) : FileObserver(rootPath, MASK) {

        companion object {
            // 监听的事件类型
            private const val MASK = CREATE or DELETE or MODIFY or MOVED_FROM or MOVED_TO or CLOSE_WRITE
        }

        private val observers = mutableListOf<FileObserver>()

        override fun onEvent(event: Int, path: String?) {
            if (path != null) {
                val fullPath = "$rootPath/$path"
                callback(event, fullPath)
            }
        }

        override fun startWatching() {
            super.startWatching()
            // 递归监听子目录
            setupRecursiveWatchers(File(rootPath))
        }

        override fun stopWatching() {
            super.stopWatching()
            observers.forEach { it.stopWatching() }
            observers.clear()
        }

        private fun setupRecursiveWatchers(directory: File) {
            directory.listFiles()?.forEach { file ->
                if (file.isDirectory) {
                    try {
                        val observer = object : FileObserver(file.absolutePath, MASK) {
                            override fun onEvent(event: Int, path: String?) {
                                if (path != null) {
                                    val fullPath = "${file.absolutePath}/$path"
                                    callback(event, fullPath)
                                }
                            }
                        }
                        observer.startWatching()
                        observers.add(observer)

                        // 继续递归
                        setupRecursiveWatchers(file)
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to watch directory: ${file.absolutePath}", e)
                    }
                }
            }
        }
    }
}