/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.net.Uri
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ViewGroup
import androidx.recyclerview.widget.RecyclerView
import com.davemorrissey.labs.subscaleview.ImageSource
import com.davemorrissey.labs.subscaleview.SubsamplingScaleImageView
import kotlin.math.abs

class ImageViewerAdapter(
    uris: List<String>,
    private val onTap: (() -> Unit)? = null,
    private val onExifNeeded: ((position: Int, uri: String) -> Unit)? = null,
) : RecyclerView.Adapter<ImageViewerAdapter.ViewHolder>() {

    private val uris: MutableList<String> = uris.toMutableList()

    /** Current visible position, updated by ViewPager2 callback */
    var currentPosition: Int = 0

    /**
     * Position of the initially-opened page. This page loads immediately
     * without waiting for orientation cache, since the TypeScript
     * sendExifToViewer pipeline handles orientation correction asynchronously.
     * Cleared after the first onPageSelected callback.
     */
    var immediateLoadPosition: Int = -1

    /**
     * Position → degrees mapping for RAW file orientation overrides.
     * Keys are present only for RAW files where backend EXIF has been resolved.
     * Values are 0, 90, 180, or 270 (matching SubsamplingScaleImageView constants).
     * Set by ImageViewerActivity.
     */
    var orientationCache: MutableMap<Int, Int> = mutableMapOf()

    class ViewHolder(val imageView: SubsamplingScaleImageView) : RecyclerView.ViewHolder(imageView) {
        /** Track which adapter position this holder is currently bound to */
        var bindPosition: Int = RecyclerView.NO_POSITION
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val imageView = SubsamplingScaleImageView(parent.context).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setMinimumScaleType(SubsamplingScaleImageView.SCALE_TYPE_CENTER_INSIDE)
            setMaxScale(5f)
            setDoubleTapZoomScale(2f)
            setOrientation(SubsamplingScaleImageView.ORIENTATION_USE_EXIF)
            setPanLimit(SubsamplingScaleImageView.PAN_LIMIT_INSIDE)
        }

        onTap?.let { callback ->
            val gestureDetector = GestureDetector(parent.context, object : GestureDetector.SimpleOnGestureListener() {
                override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                    callback()
                    return true
                }
            })
            imageView.setOnTouchListener { _, event -> gestureDetector.onTouchEvent(event) }
        }

        return ViewHolder(imageView)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        holder.bindPosition = position
        val uri = uris[position]

        // Always reset orientation to prevent ViewHolder reuse pollution
        holder.imageView.setOrientation(SubsamplingScaleImageView.ORIENTATION_USE_EXIF)

        val cachedDegrees = orientationCache[position]

        when {
            // Initial page: load immediately. Orientation corrected by sendExifToViewer pipeline.
            position == immediateLoadPosition -> {
                holder.imageView.setImage(ImageSource.uri(Uri.parse(uri)))
            }
            // Cache hit: apply pre-fetched orientation, then load image.
            // The image decodes asynchronously but orientation is already set, so
            // the first rendered frame uses the correct rotation — zero flicker.
            cachedDegrees != null -> {
                if (cachedDegrees != SubsamplingScaleImageView.ORIENTATION_USE_EXIF) {
                    holder.imageView.setOrientation(cachedDegrees)
                }
                holder.imageView.setImage(ImageSource.uri(Uri.parse(uri)))
            }
            // Cache miss: delay setImage until EXIF arrives.
            // We can't detect RAW files from content:// URIs (no file extension),
            // so we prefetch EXIF for ALL cache-miss positions. JPEG EXIF arrives
            // in ~7ms (imperceptible), RAW in ~50ms (page is offscreen anyway).
            else -> {
                onExifNeeded?.invoke(position, uri)
            }
        }
    }

    override fun onViewRecycled(holder: ViewHolder) {
        // Only recycle if distance from current position > 2
        // This keeps images within prefetch range (±2) in memory
        val position = holder.bindingAdapterPosition
        if (position != RecyclerView.NO_POSITION && abs(position - currentPosition) > 2) {
            holder.imageView.recycle()
        }
    }

    override fun getItemCount(): Int = uris.size

    fun replaceUris(newUris: List<String>) {
        uris.clear()
        uris.addAll(newUris)
        notifyDataSetChanged()
    }
}
