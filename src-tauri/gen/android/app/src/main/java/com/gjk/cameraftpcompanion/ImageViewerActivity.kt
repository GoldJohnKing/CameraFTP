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
import android.net.Uri
import android.os.Bundle
import android.provider.MediaStore
import android.util.Log
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
import org.json.JSONArray
import org.json.JSONObject

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
    private lateinit var navIndicator: TextView
    private lateinit var exifOverlay: LinearLayout
    private lateinit var exifParams: TextView
    private lateinit var exifDatetime: TextView
    private lateinit var btnRotate: ImageButton
    private lateinit var btnDelete: ImageButton
    private var uris: MutableList<String> = mutableListOf()
    private var currentIndex: Int = 0
    private var isLandscape = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        hideSystemBars()
        setContentView(R.layout.activity_image_viewer)

        uris = parseUrisFromIntent().toMutableList()
        currentIndex = intent.getIntExtra(EXTRA_TARGET_INDEX, 0)

        viewPager = findViewById(R.id.view_pager)
        navIndicator = findViewById(R.id.nav_indicator)
        exifOverlay = findViewById(R.id.exif_overlay)
        exifParams = findViewById(R.id.exif_params)
        exifDatetime = findViewById(R.id.exif_datetime)
        btnRotate = findViewById(R.id.btn_rotate)
        btnDelete = findViewById(R.id.btn_delete)

        setupViewPager()
        setupToolbar()
        updateNavIndicator()
        loadExifForCurrentImage()
    }

    private fun setupViewPager() {
        viewPager.adapter = ImageViewerAdapter(uris)
        viewPager.setCurrentItem(currentIndex, false)
        viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                updateNavIndicator()
                loadExifForCurrentImage()
            }
        })
    }

    private fun setupToolbar() {
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

    private fun confirmDelete() {
        AlertDialog.Builder(this)
            .setTitle("删除图片")
            .setMessage("确定要删除这张图片吗？")
            .setPositiveButton("删除") { _, _ ->
                deleteCurrentImage()
            }
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

                // Adjust index if needed
                if (currentIndex >= uris.size) {
                    currentIndex = uris.size - 1
                }

                viewPager.adapter?.notifyDataSetChanged()
                viewPager.setCurrentItem(currentIndex, false)
                updateNavIndicator()
                loadExifForCurrentImage()
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

    private fun loadExifForCurrentImage() {
        if (uris.isEmpty() || currentIndex < 0 || currentIndex >= uris.size) {
            exifOverlay.visibility = View.GONE
            return
        }

        val uri = uris[currentIndex]
        val webView = MainActivity.instance?.getWebView() ?: run {
            exifOverlay.visibility = View.GONE
            return
        }

        val escapedUri = uri.replace("\\", "\\\\").replace("'", "\\'")
        val script = """
            (function() {
                window.__TAURI__.core.invoke('get_image_exif', { filePath: '$escapedUri' })
                    .then(function(exif) {
                        window.ImageViewerAndroid.onExifResult(JSON.stringify(exif));
                    })
                    .catch(function() {
                        window.ImageViewerAndroid.onExifResult(null);
                    });
            })();
        """.trimIndent()

        runOnUiThread {
            webView.evaluateJavascript(script, null)
        }
    }

    /**
     * Called from JS bridge when EXIF data is available
     */
    fun onExifResult(exifJson: String?) {
        runOnUiThread {
            if (exifJson == null || exifJson == "null") {
                exifOverlay.visibility = View.GONE
                return@runOnUiThread
            }

            try {
                val exif = JSONObject(exifJson)
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
                    exifOverlay.visibility = View.VISIBLE
                } else {
                    exifOverlay.visibility = View.GONE
                }

                val datetime = exif.optString("datetime")
                if (!datetime.isNullOrEmpty()) {
                    exifDatetime.text = datetime
                    exifDatetime.visibility = View.VISIBLE
                } else {
                    exifDatetime.visibility = View.GONE
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
                exifOverlay.visibility = View.GONE
            }
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

    private fun updateNavIndicator() {
        if (uris.size > 1) {
            navIndicator.text = "${currentIndex + 1} / ${uris.size}"
        } else {
            navIndicator.text = ""
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
