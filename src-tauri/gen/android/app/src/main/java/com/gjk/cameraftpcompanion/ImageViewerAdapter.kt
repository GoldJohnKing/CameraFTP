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

class ImageViewerAdapter(
    private val uris: List<String>,
    private val onTap: (() -> Unit)? = null
) : RecyclerView.Adapter<ImageViewerAdapter.ViewHolder>() {

    class ViewHolder(val imageView: SubsamplingScaleImageView) : RecyclerView.ViewHolder(imageView)

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
        val uri = Uri.parse(uris[position])
        holder.imageView.setImage(ImageSource.uri(uri))
    }

    override fun getItemCount(): Int = uris.size
}
