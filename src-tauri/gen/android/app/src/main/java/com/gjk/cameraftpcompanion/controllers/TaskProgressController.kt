/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.controllers

import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.bridges.ImageViewerBridge
import com.gjk.cameraftpcompanion.bridges.TaskProgressState
import com.gjk.cameraftpcompanion.dpToPx
import java.lang.ref.WeakReference

class TaskProgressController(activity: android.app.Activity) {

    private companion object {
        private const val TAG = "TaskProgressController"
    }

    private val activityRef: WeakReference<android.app.Activity> = WeakReference(activity)

    private lateinit var panel: LinearLayout
    private lateinit var aiEditRow: LinearLayout
    private lateinit var cgRow: LinearLayout
    private lateinit var aiEditCount: TextView
    private lateinit var aiEditFailed: TextView
    private lateinit var aiEditCancel: TextView
    private lateinit var cgCount: TextView
    private lateinit var cgFailed: TextView
    private lateinit var cgCancel: TextView
    private lateinit var footer: TextView

    var isAiEditing = false
    var isColorGrading = false
    private var autoDismissHandler: android.os.Handler? = null
    private var autoDismissRunnable: Runnable? = null

    data class TaskRowRefs(
        val row: LinearLayout,
        val countView: TextView,
        val failedView: TextView,
        val cancelView: TextView,
        val cancelJs: String,
    )

    fun bindViews(root: View) {
        panel = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_progress_panel)
        aiEditRow = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_row_ai_edit)
        cgRow = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_row_color_grading)
        aiEditCount = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_ai_edit_count)
        aiEditFailed = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_ai_edit_failed)
        aiEditCancel = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_ai_edit_cancel)
        cgCount = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_cg_count)
        cgFailed = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_cg_failed)
        cgCancel = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_cg_cancel)
        footer = root.findViewById(com.gjk.cameraftpcompanion.R.id.task_panel_footer)
    }

    fun aiEditRefs() = TaskRowRefs(aiEditRow, aiEditCount, aiEditFailed, aiEditCancel, "__tauriCancelAiEdit")
    fun cgRefs() = TaskRowRefs(cgRow, cgCount, cgFailed, cgCancel, "__tauriCancelColorGrading")

    fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
        isAiEditing = true
        updateTaskRowProgress(aiEditRefs(), current, total, failedCount)
    }

    fun updateColorGradingProgress(current: Int, total: Int, failedCount: Int) {
        isColorGrading = true
        updateTaskRowProgress(cgRefs(), current, total, failedCount)
    }

    fun onAiEditComplete(cancelled: Boolean) {
        isAiEditing = false
        val state = ImageViewerBridge.aiEditState
        onTaskRowComplete(aiEditRefs(), state, cancelled)
    }

    fun onColorGradingComplete(cancelled: Boolean) {
        isColorGrading = false
        val state = ImageViewerBridge.colorGradingState
        onTaskRowComplete(cgRefs(), state, cancelled)
    }

    fun syncAiEditProgress() {
        syncTaskRowFromWebView(
            ImageViewerBridge.aiEditState,
            aiEditRefs(),
            { isAiEditing = true },
            { c, t, f -> updateAiEditProgress(c, t, f) },
        )
    }

    fun syncColorGradingProgress() {
        syncTaskRowFromWebView(
            ImageViewerBridge.colorGradingState,
            cgRefs(),
            { isColorGrading = true },
            { c, t, f -> updateColorGradingProgress(c, t, f) },
        )
    }

    fun cancelAutoDismiss() {
        autoDismissRunnable?.let { autoDismissHandler?.removeCallbacks(it) }
        autoDismissRunnable = null
    }

    fun updatePosition(isBottomBarVisible: Boolean, bottomBar: View) {
        val lp = panel.layoutParams as? FrameLayout.LayoutParams ?: return
        lp.gravity = Gravity.BOTTOM or Gravity.START
        lp.marginStart = 12.dpToPx(panel.context)
        lp.bottomMargin = if (isBottomBarVisible && bottomBar.visibility != View.GONE) {
            val barHeight = bottomBar.height
            if (barHeight > 0) {
                barHeight + ((bottomBar.layoutParams as? FrameLayout.LayoutParams)
                    ?.bottomMargin?.let { if (it > 0) it else 8.dpToPx(panel.context) }
                    ?: 8.dpToPx(panel.context)) + 12.dpToPx(panel.context)
            } else {
                92.dpToPx(panel.context)
            }
        } else {
            16.dpToPx(panel.context)
        }
        panel.layoutParams = lp
    }

    val isVisible: Boolean get() = panel.visibility == View.VISIBLE

    fun dismissAll() {
        aiEditRow.visibility = View.GONE
        cgRow.visibility = View.GONE
        panel.visibility = View.GONE
        resetState()
    }

    fun destroy() {
        cancelAutoDismiss()
    }

    private fun updateTaskRowProgress(refs: TaskRowRefs, current: Int, total: Int, failedCount: Int) {
        panel.visibility = View.VISIBLE
        refs.row.visibility = View.VISIBLE
        refs.countView.text = "$current / $total"

        if (failedCount > 0) {
            refs.failedView.visibility = View.VISIBLE
            refs.failedView.text = "(失败 $failedCount)"
        } else {
            refs.failedView.visibility = View.GONE
        }

        refs.cancelView.visibility = View.VISIBLE
        refs.cancelView.setOnClickListener {
            val mainActivity = MainActivity.instance
            mainActivity?.runOnUiThread {
                mainActivity.getWebView()?.evaluateJavascript(
                    "(function(){try{window.${refs.cancelJs}?.()}catch(e){}})();", null
                )
            }
        }

        val bottomBar = panel.rootView.findViewById<LinearLayout>(com.gjk.cameraftpcompanion.R.id.bottom_bar)
        val barVisible = bottomBar.visibility != View.GONE
        updatePosition(barVisible, bottomBar)
        updateFooter()
    }

    private fun onTaskRowComplete(refs: TaskRowRefs, state: TaskProgressState?, cancelled: Boolean) {
        if (cancelled) {
            refs.row.visibility = View.GONE
            updateVisibility()
            return
        }

        refs.cancelView.visibility = View.GONE
        if (state is TaskProgressState.Done && state.total > 0) {
            refs.countView.text = "${state.total} / ${state.total}"
        }

        updateFooter()
        checkAutoDismiss()
    }

    private fun syncTaskRowFromWebView(
        state: TaskProgressState?,
        refs: TaskRowRefs,
        setActive: () -> Unit,
        updateProgress: (Int, Int, Int) -> Unit,
    ) {
        if (state is TaskProgressState.InProgress) {
            setActive()
            updateProgress(state.current, state.total, state.failedCount)
        } else if (state is TaskProgressState.Done) {
            refs.row.visibility = View.VISIBLE
            refs.cancelView.visibility = View.GONE
            if (state.total > 0) {
                refs.countView.text = "${state.total} / ${state.total}"
                if (state.failedCount > 0) {
                    refs.failedView.visibility = View.VISIBLE
                    refs.failedView.text = "(失败 ${state.failedCount})"
                }
            }
            panel.visibility = View.VISIBLE
            updateFooter()
        }
    }

    private fun updateFooter() {
        val aiEditActive = aiEditRow.visibility == View.VISIBLE
        val cgActive = cgRow.visibility == View.VISIBLE
        val aiEditDone = !aiEditActive || !isAiEditing
        val cgDone = !cgActive || !isColorGrading
        val hasVisibleRow = aiEditActive || cgActive

        if (hasVisibleRow && aiEditDone && cgDone) {
            footer.text = "已完成"
            footer.setTextColor(0xFF4ADE80.toInt())
            footer.setOnClickListener(null)
        } else {
            footer.text = "全部取消"
            footer.setTextColor(0x66FFFFFF.toInt())
            footer.setOnClickListener {
                if (isAiEditing) {
                    val mainActivity = MainActivity.instance
                    mainActivity?.runOnUiThread {
                        mainActivity.getWebView()?.evaluateJavascript(
                            "(function(){try{window.__tauriCancelAiEdit?.()}catch(e){}})();", null
                        )
                    }
                }
                if (isColorGrading) {
                    val mainActivity = MainActivity.instance
                    mainActivity?.runOnUiThread {
                        mainActivity.getWebView()?.evaluateJavascript(
                            "(function(){try{window.__tauriCancelColorGrading?.()}catch(e){}})();", null
                        )
                    }
                }
            }
        }
    }

    private fun updateVisibility() {
        val hasVisibleRow = aiEditRow.visibility == View.VISIBLE || cgRow.visibility == View.VISIBLE
        if (!hasVisibleRow) {
            panel.visibility = View.GONE
        }
        updateFooter()
    }

    private fun checkAutoDismiss() {
        val aiEditActive = aiEditRow.visibility == View.VISIBLE
        val cgActive = cgRow.visibility == View.VISIBLE
        val aiEditDone = !aiEditActive || (!isAiEditing && aiEditCancel.visibility == View.GONE)
        val cgDone = !cgActive || (!isColorGrading && cgCancel.visibility == View.GONE)
        val allDone = aiEditDone && cgDone && (aiEditActive || cgActive)

        if (allDone) {
            cancelAutoDismiss()
            if (autoDismissHandler == null) {
                autoDismissHandler = android.os.Handler(android.os.Looper.getMainLooper())
            }
            autoDismissRunnable = Runnable {
                aiEditRow.visibility = View.GONE
                cgRow.visibility = View.GONE
                panel.visibility = View.GONE
                resetState()
                ImageViewerBridge.clearProgress()
                ImageViewerBridge.clearColorGradingProgress()
            }
            autoDismissHandler?.postDelayed(autoDismissRunnable!!, 3000)
        }
    }

    private fun resetState() {
        footer.text = "全部取消"
        footer.setTextColor(0x66FFFFFF.toInt())
        aiEditFailed.visibility = View.GONE
        cgFailed.visibility = View.GONE
    }
}
