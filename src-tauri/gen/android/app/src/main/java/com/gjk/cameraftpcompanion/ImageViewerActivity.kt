/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.app.Activity
import android.app.RecoverableSecurityException
import android.content.Context
import android.content.Intent
import android.content.IntentSender
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.database.Cursor
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.os.Build
import android.provider.MediaStore
import android.util.Log
import android.view.View
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.exifinterface.media.ExifInterface
import com.gjk.cameraftpcompanion.bridges.GalleryBridge
import androidx.viewpager2.widget.ViewPager2
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import org.json.JSONArray
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

class ImageViewerActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ImageViewerActivity"
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"
        const val MEDIA_LIBRARY_REFRESH_REQUESTED_EVENT = "media-library-refresh-requested"

        /** Active instance, set by onResume/cleared by onDestroy for bridge access */
        var instance: ImageViewerActivity? = null
            private set

        @JvmStatic
        fun shouldRequestDeleteConfirmation(
            apiLevel: Int,
            isSecurityException: Boolean,
            isRecoverableSecurityException: Boolean,
        ): Boolean {
            return GalleryBridge.shouldRequestDeleteConfirmation(
                apiLevel = apiLevel,
                isSecurityException = isSecurityException,
                isRecoverableSecurityException = isRecoverableSecurityException,
            )
        }

        @JvmStatic
        fun shouldTreatDeleteAsSuccess(rowsDeleted: Int, stillExists: Boolean): Boolean {
            return rowsDeleted > 0 || !stillExists
        }

        fun start(context: Context, uris: List<String>, targetIndex: Int) {
            val intent = Intent(context, ImageViewerActivity::class.java).apply {
                putExtra(EXTRA_URIS, JSONArray(uris).toString())
                putExtra(EXTRA_TARGET_INDEX, targetIndex)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
        }
    }

    private lateinit var viewPager: ViewPager2
    private lateinit var bottomBar: LinearLayout
    private lateinit var filenameView: TextView
    private lateinit var exifParams: TextView
    private lateinit var exifDatetime: TextView
    private lateinit var btnRotate: ImageButton
    private lateinit var btnDelete: ImageButton
    private var uris: MutableList<String> = mutableListOf()
    private var currentIndex: Int = 0
    private var isLandscape = false
    private var isBottomBarVisible = true
    private var pendingDeleteUri: String? = null

    private val deleteRequestLauncher = registerForActivityResult(
        ActivityResultContracts.StartIntentSenderForResult(),
    ) { result ->
        val uriString = pendingDeleteUri ?: return@registerForActivityResult
        pendingDeleteUri = null

        if (result.resultCode == Activity.RESULT_OK) {
            finalizeDeleteAfterConfirmation(uriString)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        hideSystemBars()
        setContentView(R.layout.activity_image_viewer)

        SubsamplingScaleImageView.setPreferredBitmapConfig(Bitmap.Config.ARGB_8888)

        uris = parseUrisFromIntent().toMutableList()
        currentIndex = intent.getIntExtra(EXTRA_TARGET_INDEX, 0)

        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        filenameView = findViewById(R.id.filename)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        setupViewPager()
        setupButtons()
        updateUI()
    }

    private fun setupViewPager() {
        viewPager.adapter = ImageViewerAdapter(uris) { toggleBottomBar() }
        viewPager.setCurrentItem(currentIndex, false)
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                updateUI()
            }
        })
    }

    private fun toggleBottomBar() {
        isBottomBarVisible = !isBottomBarVisible
        if (isBottomBarVisible) {
            bottomBar.alpha = 0f
            bottomBar.visibility = View.VISIBLE
            bottomBar.animate().alpha(1f).setDuration(100).start()
        } else {
            bottomBar.animate()
                .alpha(0f)
                .setDuration(100)
                .withEndAction { bottomBar.visibility = View.GONE }
                .start()
        }
    }

    private fun setupButtons() {
        btnRotate.setOnClickListener {
            isLandscape = !isLandscape
            requestedOrientation = if (isLandscape) {
                ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            } else {
                ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            }
        }

        btnDelete.setOnClickListener {
            if (uris.isNotEmpty()) {
                deleteCurrentImage()
            }
        }
    }

    private fun updateUI() {
        updateFilenameAndExif()
    }

    private fun updateFilenameAndExif() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) {
            filenameView.text = ""
            exifParams.visibility = View.GONE
            exifDatetime.visibility = View.GONE
            return
        }

        val uri = Uri.parse(uris[currentIndex])
        queryMediaStoreInfo(uri)
    }

    private fun queryMediaStoreInfo(uri: Uri) {
        val projection = arrayOf(
            MediaStore.Images.Media.DISPLAY_NAME,
            MediaStore.Images.Media.DATE_TAKEN
        )

        try {
            val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
            cursor?.use {
                if (it.moveToFirst()) {
                    val displayName = it.getString(it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME))
                    val dateTaken = it.getLong(it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_TAKEN))

                    // Filename
                    filenameView.text = displayName ?: uri.lastPathSegment ?: ""

                    // Date taken
                    if (dateTaken > 0) {
                        val sdf = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.getDefault())
                        exifDatetime.text = sdf.format(Date(dateTaken))
                        exifDatetime.visibility = View.VISIBLE
                    } else {
                        exifDatetime.visibility = View.GONE
                    }

                    // Read EXIF params natively
                    readExifParams(uri)

                    return
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to query MediaStore for ${uri}", e)
        }

        // Fallback
        filenameView.text = uri.lastPathSegment ?: ""
        exifParams.visibility = View.GONE
        exifDatetime.visibility = View.GONE
    }

    private fun readExifParams(uri: Uri) {
        try {
            contentResolver.openInputStream(uri)?.use { stream ->
                val exif = ExifInterface(stream)
                val parts = mutableListOf<String>()

                exif.getAttributeInt(ExifInterface.TAG_PHOTOGRAPHIC_SENSITIVITY, -1).takeIf { it >= 0 }?.let {
                    parts.add("ISO $it")
                }

                exif.getAttributeDouble(ExifInterface.TAG_F_NUMBER, 0.0).takeIf { it > 0 }?.let {
                    parts.add("f/${"%.1f".format(it)}")
                }

                exif.getAttributeDouble(ExifInterface.TAG_EXPOSURE_TIME, 0.0).takeIf { it > 0 }?.let {
                    if (it < 1.0) {
                        val denom = (1.0 / it).roundToInt()
                        parts.add("1/${denom}s")
                    } else {
                        parts.add("%.1fs".format(it))
                    }
                }

                // Prefer 35mm equivalent focal length; fall back to native focal length
                val focalLength = exif.getAttributeInt(ExifInterface.TAG_FOCAL_LENGTH_IN_35MM_FILM, 0).takeIf { it > 0 }
                    ?: exif.getAttributeDouble(ExifInterface.TAG_FOCAL_LENGTH, 0.0).takeIf { it > 0 }?.roundToInt()
                focalLength?.let {
                    parts.add("${it}mm")
                }

                if (parts.isNotEmpty()) {
                    exifParams.text = parts.joinToString(" • ")
                    exifParams.visibility = View.VISIBLE
                } else {
                    exifParams.visibility = View.GONE
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to read EXIF for $uri", e)
            exifParams.visibility = View.GONE
        }
    }

    /**
     * Called from JS bridge when EXIF data is available (enriches existing info)
     */
    fun onExifResult(exifJson: String?) {
        runOnUiThread {
            if (exifJson == null || exifJson == "null") return@runOnUiThread

            try {
                val exif = org.json.JSONObject(exifJson)
                val parts = mutableListOf<String>()

                exif.optInt("iso", -1).takeIf { it >= 0 }?.let {
                    parts.add("ISO $it")
                }
                exif.optString("aperture").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }
                exif.optString("shutterSpeed").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }
                exif.optString("focalLength").takeIf { !it.isNullOrEmpty() }?.let {
                    parts.add(it)
                }

                if (parts.isNotEmpty()) {
                    exifParams.text = parts.joinToString(" • ")
                    exifParams.visibility = View.VISIBLE
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
            }
        }
    }

    private fun deleteCurrentImage() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        deleteCurrentImage(uris[currentIndex], allowDeleteConfirmation = true)
    }

    private fun deleteCurrentImage(uriString: String, allowDeleteConfirmation: Boolean) {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(uri)
            if (shouldTreatDeleteAsSuccess(rowsDeleted, stillExists)) {
                applyDeleteSuccess(uriString)
            } else {
                Toast.makeText(this, "删除失败：文件不存在", Toast.LENGTH_SHORT).show()
            }
        } catch (e: Exception) {
            if (allowDeleteConfirmation) {
                when {
                    Build.VERSION.SDK_INT == Build.VERSION_CODES.Q && e is RecoverableSecurityException -> {
                        requestDeleteConfirmation(uriString, e.userAction.actionIntent.intentSender)
                        return
                    }

                    shouldRequestDeleteConfirmation(
                        apiLevel = Build.VERSION.SDK_INT,
                        isSecurityException = e is SecurityException,
                        isRecoverableSecurityException = e is RecoverableSecurityException,
                    ) -> {
                        val pendingIntent = MediaStore.createDeleteRequest(contentResolver, listOf(uri))
                        requestDeleteConfirmation(uriString, pendingIntent.intentSender)
                        return
                    }
                }
            }

            if (e is SecurityException) {
                if (!uriStillExists(uri)) {
                    applyDeleteSuccess(uriString)
                    return
                }

                Log.e(TAG, "No permission to delete image", e)
                Toast.makeText(this, "删除失败：无权限", Toast.LENGTH_SHORT).show()
                return
            }

            Log.e(TAG, "Failed to delete image", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    private fun finalizeDeleteAfterConfirmation(uriString: String) {
        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = contentResolver.delete(uri, null, null)
            val stillExists = uriStillExists(uri)
            if (shouldTreatDeleteAsSuccess(rowsDeleted, stillExists)) {
                applyDeleteSuccess(uriString)
                return
            }
        } catch (e: SecurityException) {
            if (!uriStillExists(uri)) {
                applyDeleteSuccess(uriString)
                return
            }

            Log.e(TAG, "Delete still blocked after confirmation", e)
            Toast.makeText(this, "删除失败：无权限", Toast.LENGTH_SHORT).show()
            return
        } catch (e: Exception) {
            Log.e(TAG, "Failed to finalize delete after confirmation", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
            return
        }

        Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
    }

    private fun applyDeleteSuccess(uriString: String) {
        val removedIndex = uris.indexOf(uriString)

        if (removedIndex >= 0) {
            uris.removeAt(removedIndex)

            if (removedIndex < currentIndex) {
                currentIndex -= 1
            } else if (currentIndex >= uris.size && uris.isNotEmpty()) {
                currentIndex = uris.size - 1
            }
        }

        notifyMediaLibraryDeleted()

        if (uris.isEmpty()) {
            Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
            finish()
            return
        }

        viewPager.adapter?.notifyDataSetChanged()
        viewPager.setCurrentItem(currentIndex, false)
        updateUI()
        Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
    }

    private fun uriStillExists(uri: Uri): Boolean {
        return try {
            val cursor = contentResolver.query(uri, arrayOf(MediaStore.Images.Media._ID), null, null, null)
            cursor?.use { it.moveToFirst() } ?: false
        } catch (_: Exception) {
            false
        }
    }

    private fun requestDeleteConfirmation(uriString: String, intentSender: IntentSender) {
        pendingDeleteUri = uriString

        try {
            val request = IntentSenderRequest.Builder(intentSender).build()
            deleteRequestLauncher.launch(request)
        } catch (e: Exception) {
            pendingDeleteUri = null
            Log.e(TAG, "Failed to launch delete confirmation", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
        }
    }

    private fun notifyMediaLibraryDeleted() {
        val mainActivity = MainActivity.instance ?: return
        val refreshPayload = "{\"reason\":\"delete\",\"timestamp\":${System.currentTimeMillis()}}"

        mainActivity.emitTauriEvent("file-index-changed", "{\"count\":1,\"latestFilename\":null}")
        mainActivity.emitTauriEvent(MEDIA_LIBRARY_REFRESH_REQUESTED_EVENT, "{}")
        mainActivity.emitWindowEvent("gallery-refresh-requested", refreshPayload)
        mainActivity.emitWindowEvent("latest-photo-refresh-requested", refreshPayload)
    }

    private fun parseUrisFromIntent(): List<String> {
        val urisJson = intent.getStringExtra(EXTRA_URIS) ?: return emptyList()
        return try {
            val jsonArray = JSONArray(urisJson)
            (0 until jsonArray.length()).map { jsonArray.getString(it) }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse URIs from intent", e)
            emptyList()
        }
    }

    private fun hideSystemBars() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        setContentView(R.layout.activity_image_viewer)

        viewPager = findViewById(R.id.view_pager)
        bottomBar = findViewById(R.id.bottom_bar)
        filenameView = findViewById(R.id.filename)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        setupViewPager()
        setupButtons()
        updateUI()
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        finish()
    }

    override fun onResume() {
        super.onResume()
        instance = this
    }

    override fun onDestroy() {
        if (instance == this) instance = null
        super.onDestroy()
    }
}
