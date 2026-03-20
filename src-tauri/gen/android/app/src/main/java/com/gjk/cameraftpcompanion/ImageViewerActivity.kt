/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.content.ContentUris
import android.content.Context
import android.content.Intent
import android.content.pm.ActivityInfo
import android.database.Cursor
import android.graphics.Bitmap
import android.net.Uri
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
import android.view.MotionEvent
import android.view.View
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.viewpager2.widget.ViewPager2
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import org.json.JSONArray
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class ImageViewerActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ImageViewerActivity"
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"

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
    private lateinit var navIndicator: TextView
    private lateinit var btnRotate: ImageButton
    private lateinit var btnDelete: ImageButton
    private var uris: MutableList<String> = mutableListOf()
    private var currentIndex: Int = 0
    private var isLandscape = false
    private var isBottomBarVisible = true

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
        navIndicator = findViewById(R.id.nav_indicator)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        setupViewPager()
        setupButtons()
        updateUI()
    }

    private fun setupViewPager() {
        viewPager.adapter = ImageViewerAdapter(uris)
        viewPager.setCurrentItem(currentIndex, false)
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                updateUI()
            }
        })

        // Tap to toggle bottom bar
        viewPager.getChildAt(0)?.setOnTouchListener { _, event ->
            if (event.action == MotionEvent.ACTION_UP) {
                toggleBottomBar()
            }
            false
        }
    }

    private fun toggleBottomBar() {
        isBottomBarVisible = !isBottomBarVisible
        bottomBar.visibility = if (isBottomBarVisible) View.VISIBLE else View.GONE
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
                confirmDelete()
            }
        }
    }

    private fun updateUI() {
        updateFilenameAndExif()
        updateNavIndicator()
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
            MediaStore.Images.Media.DATE_TAKEN,
            MediaStore.Images.Media.WIDTH,
            MediaStore.Images.Media.HEIGHT
        )

        try {
            val cursor: Cursor? = contentResolver.query(uri, projection, null, null, null)
            cursor?.use {
                if (it.moveToFirst()) {
                    val displayName = it.getString(it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME))
                    val dateTaken = it.getLong(it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_TAKEN))
                    val width = it.getInt(it.getColumnIndexOrThrow(MediaStore.Images.Media.WIDTH))
                    val height = it.getInt(it.getColumnIndexOrThrow(MediaStore.Images.Media.HEIGHT))

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

                    // Dimensions as EXIF params fallback
                    if (width > 0 && height > 0) {
                        exifParams.text = "${width} × ${height}"
                        exifParams.visibility = View.VISIBLE
                    } else {
                        exifParams.visibility = View.GONE
                    }

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
                    exifParams.text = parts.joinToString("  ·  ")
                    exifParams.visibility = View.VISIBLE
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
            }
        }
    }

    private fun updateNavIndicator() {
        if (uris.size > 1) {
            navIndicator.text = "${currentIndex + 1} / ${uris.size}"
            navIndicator.visibility = View.VISIBLE
        } else {
            navIndicator.visibility = View.GONE
        }
    }

    private fun confirmDelete() {
        AlertDialog.Builder(this)
            .setTitle("删除图片")
            .setMessage("确定要删除这张图片吗？")
            .setPositiveButton("删除") { _, _ -> deleteCurrentImage() }
            .setNegativeButton("取消", null)
            .show()
    }

    private fun deleteCurrentImage() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) return

        val uriString = uris[currentIndex]
        val uri = Uri.parse(uriString)

        try {
            val rowsDeleted = contentResolver.delete(uri, null, null)
            if (rowsDeleted > 0) {
                Log.d(TAG, "Deleted image: $uriString")
                uris.removeAt(currentIndex)

                if (uris.isEmpty()) {
                    Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
                    finish()
                    return
                }

                if (currentIndex >= uris.size) {
                    currentIndex = uris.size - 1
                }

                viewPager.adapter?.notifyDataSetChanged()
                viewPager.setCurrentItem(currentIndex, false)
                updateUI()
                Toast.makeText(this, "图片已删除", Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(this, "删除失败：文件不存在", Toast.LENGTH_SHORT).show()
            }
        } catch (e: SecurityException) {
            Log.e(TAG, "No permission to delete image", e)
            Toast.makeText(this, "删除失败：无权限", Toast.LENGTH_SHORT).show()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to delete image", e)
            Toast.makeText(this, "删除失败", Toast.LENGTH_SHORT).show()
        }
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

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        finish()
    }
}
