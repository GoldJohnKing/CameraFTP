package com.gjk.cameraftpcompanion

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import androidx.documentfile.provider.DocumentFile
import java.io.File

/**
 * Android 存储辅助类
 * 封装 SAF (Storage Access Framework) 操作
 */
object StorageHelper {

    private const val PREF_NAME = "storage_prefs"
    private const val KEY_SAVED_URI = "saved_directory_uri"

    /**
     * 获取持久化的存储目录 URI
     */
    fun getPersistedDirectoryUri(context: Context): String? {
        val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        return prefs.getString(KEY_SAVED_URI, null)
    }

    /**
     * 保存目录 URI 并持久化权限
     */
    fun persistDirectoryUri(context: Context, uri: Uri): Boolean {
        return try {
            // 持久化权限
            val takeFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            context.contentResolver.takePersistableUriPermission(uri, takeFlags)

            // 保存 URI 到 SharedPreferences
            val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
            prefs.edit().putString(KEY_SAVED_URI, uri.toString()).apply()

            true
        } catch (e: Exception) {
            e.printStackTrace()
            false
        }
    }

    /**
     * 检查持久化权限是否仍然有效
     */
    fun checkPersistedPermission(context: Context, uriString: String?): Boolean {
        if (uriString == null) return false

        return try {
            val uri = Uri.parse(uriString)
            val persistedUris = context.contentResolver.persistedUriPermissions
            persistedUris.any { it.uri == uri && it.isWritePermission }
        } catch (e: Exception) {
            false
        }
    }

    /**
     * 清除保存的目录 URI
     */
    fun clearPersistedDirectory(context: Context) {
        val savedUri = getPersistedDirectoryUri(context)
        if (savedUri != null) {
            try {
                val uri = Uri.parse(savedUri)
                context.contentResolver.releasePersistableUriPermission(
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                )
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }

        val prefs = context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        prefs.edit().remove(KEY_SAVED_URI).apply()
    }

    /**
     * 获取推荐存储路径（供 Rust 层调用）
     * 优先级：1. 持久化路径 2. /DCIM/CameraFTPCompanion 3. /Pictures/CameraFTPCompanion 4. 应用私有目录
     */
    fun getRecommendedStoragePath(context: Context): String {
        // 1. 检查是否有持久化且有效的权限
        val persistedUri = getPersistedDirectoryUri(context)
        if (persistedUri != null && checkPersistedPermission(context, persistedUri)) {
            return persistedUri
        }

        // 2. 尝试 DCIM 路径（传统方式，Android 10+ 可能受限）
        val dcimPath = File(
            android.os.Environment.getExternalStoragePublicDirectory(
                android.os.Environment.DIRECTORY_DCIM
            ), "CameraFTPCompanion"
        )
        if (dcimPath.exists() || dcimPath.mkdirs()) {
            return dcimPath.absolutePath
        }

        // 3. 尝试 Pictures 路径
        val picturesPath = File(
            android.os.Environment.getExternalStoragePublicDirectory(
                android.os.Environment.DIRECTORY_PICTURES
            ), "CameraFTPCompanion"
        )
        if (picturesPath.exists() || picturesPath.mkdirs()) {
            return picturesPath.absolutePath
        }

        // 4. 回退到应用私有目录
        return File(context.getExternalFilesDir(null), "ftp_uploads").absolutePath
    }

    /**
     * 通过 DocumentFile API 在 SAF 目录下创建文件
     * 用于 FTP 服务器保存文件时
     */
    fun createFileInDirectory(
        context: Context,
        directoryUri: String,
        fileName: String
    ): DocumentFile? {
        return try {
            val uri = Uri.parse(directoryUri)
            val parentDoc = DocumentFile.fromTreeUri(context, uri)
            parentDoc?.createFile("application/octet-stream", fileName)
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }

    /**
     * 获取目录下的文件列表
     */
    fun listFilesInDirectory(context: Context, directoryUri: String): List<String> {
        return try {
            val uri = Uri.parse(directoryUri)
            val docFile = DocumentFile.fromTreeUri(context, uri)
            docFile?.listFiles()?.map { it.name ?: "unknown" } ?: emptyList()
        } catch (e: Exception) {
            emptyList()
        }
    }

    /**
     * 创建目录选择器的 Intent
     */
    fun createDirectoryPickerIntent(): Intent {
        return Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            // 添加标志以获得持久权限
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)

            // 尝试设置初始目录为 DCIM
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val dcimUri = DocumentsContract.buildDocumentUri(
                    "com.android.externalstorage.documents",
                    "primary:DCIM"
                )
                putExtra(DocumentsContract.EXTRA_INITIAL_URI, dcimUri)
            }
        }
    }

    /**
     * 检查是否拥有 MANAGE_EXTERNAL_STORAGE 权限（所有文件访问权限）
     * Android 11+ (API 30+) 需要这个权限才能访问所有文件
     */
    fun hasManageExternalStoragePermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            android.os.Environment.isExternalStorageManager()
        } else {
            // Android 10 及以下不需要这个权限
            true
        }
    }

    /**
     * 获取开启 MANAGE_EXTERNAL_STORAGE 权限的设置页面 Intent
     * 用户需要手动在设置中开启"所有文件访问权限"
     */
    fun getManageStorageSettingsIntent(): Intent {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: 跳转到应用特定的所有文件访问权限设置
            Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:${MainActivity.currentActivity?.packageName}")
            }
        } else {
            // Android 10 及以下: 跳转到应用信息页面
            Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                data = Uri.parse("package:${MainActivity.currentActivity?.packageName}")
            }
        }
    }

    /**
     * 跳转到设置页面开启所有文件访问权限
     */
    fun openManageStorageSettings(activity: Activity) {
        val intent = getManageStorageSettingsIntent()
        activity.startActivity(intent)
    }
}
