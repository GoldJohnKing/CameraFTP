/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import android.content.Intent
import androidx.viewpager2.widget.ViewPager2
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Live-path tests for [ImageViewerActivity.insertImage]: the orientation
 * cache shift applied when an image is inserted, and the revert applied
 * when the adapter rejects the insert. These run a real (Robolectric)
 * activity so `runOnUiThread` executes synchronously on the test thread.
 *
 * Uses the merged app manifest (NOT `Config.NONE`) so the activity gets its
 * manifest-declared AppCompat-descendant theme.
 *
 * The trailing `prefetchOrientations()` of a successful insert dispatches
 * EXIF work into the main WebView via `MainActivity.instance`, which cannot
 * be touched under a JVM run (loading MainActivity initializes WryActivity,
 * whose static initializer loads the Rust native library). That external
 * side effect is not what these tests target, so each test neutralizes it:
 * the ViewPager2 page-change callback is detached (see
 * [detachPageChangeCallbacks]) and the inserted slot is marked as the
 * adapter's `immediateLoadPosition` so the direct prefetch call collects
 * no items.
 *
 * `computeInsertState` (the pure decision helper) is covered by
 * [ImageViewerActivityInsertTest]; this file covers the application of
 * that decision to the live activity state.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ImageViewerInsertImageLiveTest {

    private fun buildActivity(uris: List<String>, targetIndex: Int): ImageViewerActivity {
        val intent = Intent(Intent.ACTION_MAIN).apply {
            putExtra(ImageViewerActivity.EXTRA_URIS, JSONArray(uris).toString())
            putExtra(ImageViewerActivity.EXTRA_TARGET_INDEX, targetIndex)
        }
        val activity = Robolectric.buildActivity(ImageViewerActivity::class.java, intent)
            .create()
            .get()
        detachPageChangeCallbacks(activity)
        return activity
    }

    /**
     * Remove the activity's ViewPager2 page-change callback.
     *
     * `insertImage` ends with `viewPager.setCurrentItem(...)` which triggers
     * `onPageSelected` → `ExifController.prefetchOrientations` →
     * `MainActivity.instance`. Merely touching the MainActivity class
     * initializes WryActivity, whose static initializer loads the Rust
     * native library — impossible under a JVM Robolectric run. The EXIF
     * prefetch side effect is not what these tests target, so the callback
     * is detached to keep the cache-shift/revert logic under test.
     */
    private fun detachPageChangeCallbacks(activity: ImageViewerActivity) {
        val pagerField = ViewPager2::class.java.getDeclaredField("mExternalPageChangeCallbacks")
        pagerField.isAccessible = true
        val composite = pagerField.get(activity.viewPager)
        val listField = composite.javaClass.getDeclaredField("mCallbacks")
        listField.isAccessible = true
        @Suppress("UNCHECKED_CAST")
        val callbacks = listField.get(composite) as MutableList<Any>
        callbacks.clear()
    }

    @Test
    fun insert_image_shifts_cache_positions_at_or_after_insert_index() {
        val activity = buildActivity(listOf("uri_a", "uri_b", "uri_c", "uri_d"), targetIndex = 1)
        val cache = activity.exifController.orientationCache
        cache[0] = 0
        cache[1] = 90
        cache[3] = 270
        // Prefetch-neutralizing: positions around the (unchanged) current
        // index must all be either cached or the inserted slot, so the
        // trailing prefetchOrientations() becomes a no-op (see class KDoc).
        (activity.viewPager.adapter as ImageViewerAdapter).immediateLoadPosition = 2

        activity.insertImage("uri_new", insertIndex = 2)

        assertEquals(
            listOf("uri_a", "uri_b", "uri_new", "uri_c", "uri_d"),
            activity.uris
        )
        assertEquals(
            "Insert after the current position must not shift currentIndex",
            1,
            activity.currentIndex
        )
        assertEquals(
            "Entries at positions >= insertIndex must shift by +1; others stay",
            mapOf(0 to 0, 1 to 90, 4 to 270),
            cache.toMap()
        )
    }

    @Test
    fun insert_image_at_or_before_current_index_shifts_current_index() {
        val activity = buildActivity(listOf("uri_a", "uri_b", "uri_c"), targetIndex = 1)
        val cache = activity.exifController.orientationCache
        cache[1] = 90
        cache[2] = 180
        // Prefetch-neutralizing: after the insert, prefetch runs around the
        // shifted currentIndex (2) over positions 1..3. Position 1 is the
        // inserted slot (skipped via immediateLoadPosition); positions 2 and
        // 3 are covered by the shifted cache entries.
        (activity.viewPager.adapter as ImageViewerAdapter).immediateLoadPosition = 1

        activity.insertImage("uri_new", insertIndex = 1)

        assertEquals(listOf("uri_a", "uri_new", "uri_b", "uri_c"), activity.uris)
        assertEquals(
            "Insert at the current position must shift currentIndex to keep the same image",
            2,
            activity.currentIndex
        )
        assertEquals("Entries >= insertIndex shift by +1", mapOf(2 to 90, 3 to 180), cache.toMap())
    }

    @Test
    fun insert_image_duplicate_uri_is_ignored_entirely() {
        val activity = buildActivity(listOf("uri_a", "uri_b"), targetIndex = 0)
        val cache = activity.exifController.orientationCache
        cache[0] = 90
        cache[1] = 180

        activity.insertImage("uri_b", insertIndex = 0)

        assertEquals(listOf("uri_a", "uri_b"), activity.uris)
        assertEquals(0, activity.currentIndex)
        assertEquals("Cache must be untouched for a rejected duplicate", mapOf(0 to 90, 1 to 180), cache.toMap())
    }

    @Test
    fun insert_image_reverts_uris_and_cache_when_adapter_rejects() {
        val activity = buildActivity(listOf("uri_a", "uri_b", "uri_c"), targetIndex = 1)
        val cache = activity.exifController.orientationCache
        cache[0] = 0
        cache[1] = 90
        cache[2] = 180

        // Force the adapter to reject the insert: the adapter's list already
        // contains the uri while the activity's list does not.
        val adapter = activity.viewPager.adapter as ImageViewerAdapter
        adapter.replaceUris(listOf("uri_a", "uri_new", "uri_b", "uri_c"))

        activity.insertImage("uri_new", insertIndex = 1)

        assertEquals(
            "Activity uri list must be reverted after the adapter rejects",
            listOf("uri_a", "uri_b", "uri_c"),
            activity.uris
        )
        assertEquals(1, activity.currentIndex)
        assertEquals(
            "Orientation cache must be restored to the pre-shift mapping",
            mapOf(0 to 0, 1 to 90, 2 to 180),
            cache.toMap()
        )
    }
}
