/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.controllers

import android.content.res.Configuration
import android.database.Cursor
import android.graphics.Bitmap
import android.net.Uri
import android.provider.MediaStore
import android.util.Log
import android.view.View
import android.widget.TextView
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import com.gjk.cameraftpcompanion.ImageViewerActivity
import com.gjk.cameraftpcompanion.ImageViewerAdapter
import com.gjk.cameraftpcompanion.R
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import kotlin.math.roundToInt

class ExifController(activity: ImageViewerActivity) {

    private companion object {
        private const val TAG = "ExifController"
    }

    private val activityRef: WeakReference<ImageViewerActivity> = WeakReference(activity)

    val orientationCache = ConcurrentHashMap<Int, Int>()
    private val exifExecutor = Executors.newFixedThreadPool(2)

    fun attachAdapter(adapter: ImageViewerAdapter) {
        adapter.orientationCache = orientationCache
    }

    fun updateFilenameAndExif() {
        val activity = activityRef.get() ?: return
        activity.currentDisplayName = null
        if (activity.uris.isEmpty() || activity.currentIndex < 0 || activity.currentIndex >= activity.uris.size) {
            activity.filenameView.text = ""
            activity.exifParams.visibility = View.GONE
            activity.exifDatetime.visibility = View.GONE
            return
        }

        val uri = Uri.parse(activity.uris[activity.currentIndex])
        queryMediaStoreInfo(activity, uri)
    }

    private fun queryMediaStoreInfo(activity: ImageViewerActivity, uri: Uri) {
        val projection = arrayOf(
            MediaStore.Images.Media.DISPLAY_NAME,
            MediaStore.Images.Media.DATE_TAKEN
        )

        try {
            val cursor: Cursor? = activity.contentResolver.query(uri, projection, null, null, null)
            cursor?.use {
                if (it.moveToFirst()) {
                    val displayName = it.getString(it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME))
                    activity.currentDisplayName = displayName
                    val dateTaken = it.getLong(it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_TAKEN))

                    activity.filenameView.text = displayName ?: uri.lastPathSegment ?: ""

                    if (dateTaken > 0) {
                        val sdf = SimpleDateFormat("yyyy-MM-dd HH:mm:ss", Locale.getDefault())
                        activity.exifDatetime.text = sdf.format(Date(dateTaken))
                        activity.exifDatetime.visibility = View.VISIBLE
                    } else {
                        activity.exifDatetime.visibility = View.GONE
                    }

                    readExifParams(activity, uri)
                    return
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to query MediaStore for $uri", e)
        }

        activity.filenameView.text = uri.lastPathSegment ?: ""
        activity.exifParams.visibility = View.GONE
        activity.exifDatetime.visibility = View.GONE
    }

    private fun readExifParams(activity: ImageViewerActivity, uri: Uri) {
        exifExecutor.execute {
            try {
                val parts = mutableListOf<String>()
                activity.contentResolver.openInputStream(uri)?.use { stream ->
                    val exif = androidx.exifinterface.media.ExifInterface(stream)

                    exif.getAttributeInt(androidx.exifinterface.media.ExifInterface.TAG_PHOTOGRAPHIC_SENSITIVITY, -1)
                        .takeIf { it >= 0 }?.let { parts.add("ISO $it") }

                    exif.getAttributeDouble(androidx.exifinterface.media.ExifInterface.TAG_F_NUMBER, 0.0)
                        .takeIf { it > 0 }?.let { parts.add("f/${"%.1f".format(it)}") }

                    exif.getAttributeDouble(androidx.exifinterface.media.ExifInterface.TAG_EXPOSURE_TIME, 0.0)
                        .takeIf { it > 0 }?.let {
                            if (it < 1.0) {
                                val denom = (1.0 / it).roundToInt()
                                parts.add("1/${denom}s")
                            } else {
                                parts.add("%.1fs".format(it))
                            }
                        }

                    val focalLength = exif.getAttributeInt(
                        androidx.exifinterface.media.ExifInterface.TAG_FOCAL_LENGTH_IN_35MM_FILM, 0
                    ).takeIf { it > 0 }
                        ?: exif.getAttributeDouble(
                            androidx.exifinterface.media.ExifInterface.TAG_FOCAL_LENGTH, 0.0
                        ).takeIf { it > 0 }?.roundToInt()
                    focalLength?.let { parts.add("${it}mm") }
                }

                activity.runOnUiThread {
                    if (activity.isFinishing || activity.isDestroyed) return@runOnUiThread
                    if (parts.isNotEmpty()) {
                        activity.exifParams.text = parts.joinToString(" • ")
                        activity.exifParams.visibility = View.VISIBLE
                    } else {
                        activity.exifParams.visibility = View.GONE
                    }
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to read EXIF for $uri", e)
                activity.runOnUiThread {
                    if (!activity.isFinishing && !activity.isDestroyed) {
                        activity.exifParams.visibility = View.GONE
                    }
                }
            }
        }
    }

    fun prefetchOrientations(around: Int) {
        val activity = activityRef.get() ?: return
        val adapter = activity.viewPager.adapter as? ImageViewerAdapter ?: return
        val items = mutableListOf<Pair<Int, String>>()
        for (pos in maxOf(0, around - 1)..minOf(activity.uris.lastIndex, around + 1)) {
            if (orientationCache.containsKey(pos)) continue
            if (pos == adapter.immediateLoadPosition) continue
            items.add(pos to activity.uris[pos])
        }
        if (items.isEmpty()) return
        val jsonArray = JSONArray().apply {
            for ((pos, uri) in items) {
                put(JSONObject().apply {
                    put("position", pos)
                    put("uri", uri)
                })
            }
        }
        activity.requestExifPrefetch(jsonArray.toString())
    }

    fun requestSingleExif(position: Int, uri: String) {
        if (orientationCache.containsKey(position)) return
        val jsonArray = JSONArray().apply {
            put(JSONObject().apply {
                put("position", position)
                put("uri", uri)
            })
        }
        val activity = activityRef.get() ?: return
        activity.requestExifPrefetch(jsonArray.toString())
    }

    fun onExifResult(exifJson: String?) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            if (activity.isFinishing || activity.isDestroyed) return@runOnUiThread
            if (exifJson == null || exifJson == "null") return@runOnUiThread

            try {
                val exif = JSONObject(exifJson)
                val parts = mutableListOf<String>()

                exif.optInt("iso", -1).takeIf { it >= 0 }?.let { parts.add("ISO $it") }
                exif.optString("aperture").takeIf { !it.isNullOrEmpty() }?.let { parts.add(it) }
                exif.optString("shutterSpeed").takeIf { !it.isNullOrEmpty() }?.let { parts.add(it) }
                exif.optString("focalLength").takeIf { !it.isNullOrEmpty() }?.let { parts.add(it) }

                if (parts.isNotEmpty()) {
                    activity.exifParams.text = parts.joinToString(" • ")
                    activity.exifParams.visibility = View.VISIBLE
                }

                if (activity.isRawFileByExtension(activity.currentDisplayName)) {
                    val orientation = exif.optInt("orientation", 0)
                    val degrees = ImageViewerActivity.exifOrientationToDegrees(orientation)
                    orientationCache[activity.currentIndex] = degrees
                    applyOrientationFromExif(activity, exif)
                }
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF result", e)
            }
        }
    }

    fun onExifResultForPosition(position: Int, exifJson: String?) {
        val activity = activityRef.get() ?: return
        activity.runOnUiThread {
            if (activity.isFinishing || activity.isDestroyed) return@runOnUiThread
            if (position !in activity.uris.indices) return@runOnUiThread

            if (exifJson == null || exifJson == "null") {
                orientationCache[position] = SubsamplingScaleImageView.ORIENTATION_USE_EXIF
                applyOrientationIfLoaded(activity, position)
                return@runOnUiThread
            }

            try {
                val exif = JSONObject(exifJson)
                val orientation = exif.optInt("orientation", 0)
                val degrees = ImageViewerActivity.exifOrientationToDegrees(orientation)
                orientationCache[position] = degrees
                applyOrientationToHolder(activity, position, degrees)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to parse EXIF for position $position", e)
                orientationCache[position] = SubsamplingScaleImageView.ORIENTATION_USE_EXIF
                applyOrientationIfLoaded(activity, position)
            }
        }
    }

    private fun applyOrientationFromExif(activity: ImageViewerActivity, exif: JSONObject) {
        val orientation = exif.optInt("orientation", 0)
        if (orientation <= 1) return

        val degrees = ImageViewerActivity.exifOrientationToDegrees(orientation)
        if (degrees == 0) return

        val rv = activity.viewPager.getChildAt(0) as? androidx.recyclerview.widget.RecyclerView ?: return
        val holder = rv.findViewHolderForAdapterPosition(activity.currentIndex) as? ImageViewerAdapter.ViewHolder ?: return

        Log.d(TAG, "Applying backend orientation $orientation ($degrees°) to current image")
        holder.imageView.setOrientation(degrees)
    }

    private fun applyOrientationToHolder(activity: ImageViewerActivity, position: Int, degrees: Int) {
        val rv = activity.viewPager.getChildAt(0) as? androidx.recyclerview.widget.RecyclerView ?: return
        val holder = rv.findViewHolderForAdapterPosition(position) as? ImageViewerAdapter.ViewHolder ?: return
        if (holder.bindPosition != position) return
        if (degrees != SubsamplingScaleImageView.ORIENTATION_USE_EXIF) {
            Log.d(TAG, "Applying prefetched orientation $degrees° to position $position")
            holder.imageView.setOrientation(degrees)
        }
    }

    private fun applyOrientationIfLoaded(activity: ImageViewerActivity, position: Int) {
        val degrees = orientationCache[position] ?: return
        applyOrientationToHolder(activity, position, degrees)
    }

    fun destroy() {
        exifExecutor.shutdownNow()
    }
}
