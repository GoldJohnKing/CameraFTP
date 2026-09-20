/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowToast
import android.os.Looper
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/**
 * Covers NativeColorGradingPreviewBridge.save() after the enqueueBatch
 * rework: success means "task accepted" (Rust validates the LUT id, then
 * performs the actual enqueue — including mpsc backpressure waits — on a
 * dedicated thread), and neither the last-used config persistence nor the
 * preview teardown may run on the WebView JavaBridge (calling) thread.
 *
 * The bridge is constructed with injected JNI seams so these JVM tests never
 * trigger ColorGradingJniBridge's System.loadLibrary static init (same
 * strategy as ColorGradingJniBridgeTest).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], manifest = Config.NONE)
class ColorGradingActivitySaveTest {

    private fun newActivity(): ColorGradingActivity =
        // attach() only — no onCreate (it needs a filePath intent + WebView);
    // runOnUiThread/finish/isFinishing all work on an attached activity.
        Robolectric.buildActivity(ColorGradingActivity::class.java).get()

    private fun idleMainLooper() {
        shadowOf(Looper.getMainLooper()).idle()
    }

    @Test
    fun save_enqueueAccepted_finishesActivityWithArgsAndKeepsSavingLatch() {
        val activity = newActivity()
        val recordedPath = AtomicReference<String?>(null)
        val recordedEnqueue = AtomicReference<Triple<String, String, Float>?>(null)

        val bridge = NativeColorGradingPreviewBridge(
            activity,
            "/sdcard/DCIM/photo.nef",
            endPreviewOp = {},
            saveLastUsedOp = { _, _, _ -> Result.success(Unit) },
            enqueueBatchOp = { path, lutId, meteringMode, evOffset ->
                recordedPath.set(path)
                recordedEnqueue.set(Triple(lutId, meteringMode, evOffset))
                Result.success(Unit)
            },
        )

        bridge.save("fujifilm-classic-neg", "matrix", 0.5f)
        idleMainLooper()

        assertEquals("/sdcard/DCIM/photo.nef", recordedPath.get())
        val args = recordedEnqueue.get()
        assertNotNull("enqueueBatch must be invoked", args)
        assertEquals("fujifilm-classic-neg", args!!.first)
        assertEquals("matrix", args.second)
        assertEquals(0.5f, args.third)
        assertTrue(
            "accepted save must finish the activity immediately",
            activity.isFinishing,
        )
        assertTrue(
            "isSaving must stay latched so onDestroy skips cleanup",
            activity.isSaving,
        )
    }

    @Test
    fun save_enqueueRejected_showsToast_doesNotFinish_reopensSavingLatch() {
        val activity = newActivity()

        val bridge = NativeColorGradingPreviewBridge(
            activity,
            "/sdcard/DCIM/photo.nef",
            endPreviewOp = {},
            saveLastUsedOp = { _, _, _ -> Result.success(Unit) },
            enqueueBatchOp = { _, _, _, _ ->
                Result.failure(IllegalStateException("bad lut"))
            },
        )

        bridge.save("fujifilm-classic-neg", "matrix", 0f)
        idleMainLooper()

        assertFalse("rejected save must not finish the activity", activity.isFinishing)
        assertFalse(
            "isSaving must be reset so onDestroy can clean up",
            activity.isSaving,
        )
        assertEquals("bad lut", ShadowToast.getTextOfLatestToast())
    }

    @Test
    fun save_saveLastUsedRunsOffCallerThread_andDoesNotBlockSave() {
        val activity = newActivity()
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val completed = CountDownLatch(1)
        val persistThreadId = AtomicReference<Long?>(null)
        val persistArgs = AtomicReference<Triple<String, String, Float>?>(null)

        val bridge = NativeColorGradingPreviewBridge(
            activity,
            "/sdcard/DCIM/photo.nef",
            endPreviewOp = {},
            saveLastUsedOp = { lutId, meteringMode, evOffset ->
                persistThreadId.set(Thread.currentThread().id)
                persistArgs.set(Triple(lutId, meteringMode, evOffset))
                entered.countDown()
                // Simulate slow config persistence (disk I/O on the JNI side).
                release.await(5, TimeUnit.SECONDS)
                completed.countDown()
                Result.success(Unit)
            },
            enqueueBatchOp = { _, _, _, _ -> Result.success(Unit) },
        )

        // Simulate the WebView JavaBridge thread calling into save(). If
        // save() waited for the persistence to finish, the future would hit
        // the 5s timeout and fail the test — exactly the freeze this fixes.
        val returned = CompletableFuture<Long>()
        val bridgeThreadId = AtomicReference<Long?>(null)
        val bridgeThread = Thread {
            bridgeThreadId.set(Thread.currentThread().id)
            bridge.save("fujifilm-classic-neg", "matrix", -0.7f)
            returned.complete(Thread.currentThread().id)
        }
        bridgeThread.start()

        val callerId = returned.get(5, TimeUnit.SECONDS)
        assertTrue("persist op must have started", entered.await(5, TimeUnit.SECONDS))

        // save() returned on the caller thread…
        assertEquals(callerId, bridgeThreadId.get())
        // …while persistence ran (and is still parked) on a different thread.
        assertNotEquals(
            "config persistence must leave the JavaBridge thread",
            callerId,
            persistThreadId.get()!!,
        )
        // CountDownLatch(1) semantics: count is 1 while still latched (parked),
        // 0 after countDown(). "Still parked" therefore asserts 1, not 0.
        assertEquals(1L, completed.count)
        assertEquals("fujifilm-classic-neg", persistArgs.get()!!.first)
        assertEquals("matrix", persistArgs.get()!!.second)
        assertEquals(-0.7f, persistArgs.get()!!.third)

        release.countDown()
        assertTrue(
            "persisted op should finish after release",
            completed.await(5, TimeUnit.SECONDS),
        )
        idleMainLooper()
        assertTrue(activity.isFinishing)
    }

    @Test
    fun save_endPreviewRunsOffCallerThread_andClearsSessionFlagSynchronously() {
        val activity = newActivity()
        activity.isSessionActive = true
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val endPreviewThreadId = AtomicReference<Long?>(null)

        val bridge = NativeColorGradingPreviewBridge(
            activity,
            "/sdcard/DCIM/photo.nef",
            endPreviewOp = {
                endPreviewThreadId.set(Thread.currentThread().id)
                entered.countDown()
                release.await(5, TimeUnit.SECONDS)
            },
            saveLastUsedOp = { _, _, _ -> Result.success(Unit) },
            enqueueBatchOp = { _, _, _, _ -> Result.success(Unit) },
        )

        val callerId = Thread.currentThread().id
        bridge.save("fujifilm-classic-neg", "matrix", 0f)

        // The session flag must be cleared synchronously on the caller
        // thread (onDestroy's back-press path depends on it).
        assertFalse("session must be marked inactive synchronously", activity.isSessionActive)

        assertTrue("endPreview must be invoked", entered.await(5, TimeUnit.SECONDS))
        assertNotEquals(
            "preview teardown must leave the JavaBridge thread",
            callerId,
            endPreviewThreadId.get()!!,
        )

        release.countDown()
        idleMainLooper()
    }
}
