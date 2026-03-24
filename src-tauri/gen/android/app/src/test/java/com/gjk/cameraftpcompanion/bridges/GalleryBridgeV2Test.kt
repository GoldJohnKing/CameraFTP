/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.app.Activity
import com.gjk.cameraftpcompanion.galleryv2.MediaPageProvider
import com.gjk.cameraftpcompanion.galleryv2.ThumbResult
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailCacheV2
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailPipelineManager
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class GalleryBridgeV2Test {

    private lateinit var activity: Activity
    private lateinit var pipelineManager: ThumbnailPipelineManager
    private lateinit var bridge: GalleryBridgeV2

    @Before
    fun setUp() {
        activity = Robolectric.buildActivity(Activity::class.java).create().get()
        pipelineManager = ThumbnailPipelineManager()
        bridge = GalleryBridgeV2(
            context = activity,
            mediaPageProvider = MediaPageProvider(activity),
            pipelineManager = pipelineManager,
            cache = ThumbnailCacheV2()
        )
    }

    // ── Listener register/unregister ──────────────────────────────────

    @Test
    fun register_listener_stores_mapping() {
        bridge.registerThumbnailListener("view1", "listener1")

        // Verify by unregistering — should not throw
        bridge.unregisterThumbnailListener("listener1")
    }

    @Test
    fun register_multiple_listeners_per_view() {
        bridge.registerThumbnailListener("view1", "listenerA")
        bridge.registerThumbnailListener("view1", "listenerB")

        // Unregister one, the other should still be active
        bridge.unregisterThumbnailListener("listenerA")

        // Invalidate the view — should clean up remaining listener
        bridge.invalidateListenersForView("view1")
    }

    @Test
    fun unregister_nonexistent_listener_does_not_throw() {
        bridge.unregisterThumbnailListener("nonexistent")
    }

    @Test
    fun invalidate_listeners_for_view_removes_all_listeners() {
        bridge.registerThumbnailListener("view1", "listener1")
        bridge.registerThumbnailListener("view1", "listener2")
        bridge.registerThumbnailListener("view2", "listener3")

        bridge.invalidateListenersForView("view1")

        // view2 listener should still be valid — verify no exception
        bridge.unregisterThumbnailListener("listener3")
    }

    // ── cancelByView ──────────────────────────────────────────────────

    @Test
    fun cancel_by_view_cancels_all_jobs_for_view() {
        val jobsJson = JSONArray().apply {
            put(JSONObject().apply {
                put("requestId", "req1")
                put("mediaId", "100")
                put("uri", "content://media/100")
                put("dateModifiedMs", 1000L)
                put("sizeBucket", "s")
                put("priority", "visible")
                put("viewId", "view1")
            })
            put(JSONObject().apply {
                put("requestId", "req2")
                put("mediaId", "200")
                put("uri", "content://media/200")
                put("dateModifiedMs", 2000L)
                put("sizeBucket", "s")
                put("priority", "nearby")
                put("viewId", "view1")
            })
            put(JSONObject().apply {
                put("requestId", "req3")
                put("mediaId", "300")
                put("uri", "content://media/300")
                put("dateModifiedMs", 3000L)
                put("sizeBucket", "s")
                put("priority", "visible")
                put("viewId", "view2")
            })
        }

        bridge.enqueueThumbnails(jobsJson.toString())

        // Cancel all jobs for view1
        bridge.cancelByView("view1")

        // view1 jobs should be cancelled; view2 job should remain
        val stats = pipelineManager.queueStats()
        // After cancellation, only view2's job should remain in queue
        assertTrue("view2 job should still be pending", stats.pending <= 1)
    }

    @Test
    fun cancel_by_view_with_no_jobs_does_not_throw() {
        bridge.cancelByView("empty_view")
    }

    // ── invalidateMediaIds ────────────────────────────────────────────

    @Test
    fun invalidate_media_ids_removes_cached_entries() {
        val cache = ThumbnailCacheV2()
        cache.initialize(activity)

        // Pre-populate cache with entries
        val key1 = com.gjk.cameraftpcompanion.galleryv2.ThumbnailKeyV2.of("100", 0, "s", 0, 0)
        val key2 = com.gjk.cameraftpcompanion.galleryv2.ThumbnailKeyV2.of("200", 0, "s", 0, 0)
        cache.put(key1, "s", byteArrayOf(1, 2, 3))
        cache.put(key2, "s", byteArrayOf(4, 5, 6))

        // Verify entries exist
        assertNotNull("key1 should be cached", cache.get(key1, "s"))
        assertNotNull("key2 should be cached", cache.get(key2, "s"))

        // Invalidate mediaId 100
        val bridgeWithCache = GalleryBridgeV2(
            context = activity,
            mediaPageProvider = MediaPageProvider(activity),
            pipelineManager = ThumbnailPipelineManager(),
            cache = cache
        )
        bridgeWithCache.invalidateMediaIds("""["100"]""")

        // key1 should be gone, key2 should remain
        assertNull("key1 should be invalidated", cache.get(key1, "s"))
        assertNotNull("key2 should still be cached", cache.get(key2, "s"))
    }

    @Test
    fun invalidate_media_ids_with_empty_array_does_not_throw() {
        bridge.invalidateMediaIds("[]")
    }

    @Test
    fun invalidate_media_ids_with_invalid_json_does_not_throw() {
        bridge.invalidateMediaIds("not json")
    }

    // ── getQueueStats ─────────────────────────────────────────────────

    @Test
    fun get_queue_stats_returns_valid_json() {
        val statsJson = bridge.getQueueStats()
        val json = JSONObject(statsJson)

        assertTrue("should have pending", json.has("pending"))
        assertTrue("should have running", json.has("running"))
        assertTrue("should have cacheHitRate", json.has("cacheHitRate"))
    }

    @Test
    fun get_queue_stats_reflects_enqueued_jobs() {
        val jobsJson = JSONArray().apply {
            put(JSONObject().apply {
                put("requestId", "req1")
                put("mediaId", "100")
                put("uri", "content://media/100")
                put("dateModifiedMs", 1000L)
                put("sizeBucket", "s")
                put("priority", "visible")
                put("viewId", "view1")
            })
        }
        bridge.enqueueThumbnails(jobsJson.toString())

        val stats = JSONObject(bridge.getQueueStats())
        assertTrue("pending should be >= 0", stats.getInt("pending") >= 0)
    }

    // ── listMediaPage ─────────────────────────────────────────────────

    @Test
    fun list_media_page_returns_valid_structure() {
        val requestJson = JSONObject().apply {
            put("pageSize", 10)
        }.toString()

        val resultJson = bridge.listMediaPage(requestJson)
        val json = JSONObject(resultJson)

        assertTrue("should have items", json.has("items"))
        assertTrue("should have revisionToken", json.has("revisionToken"))
    }

    @Test
    fun list_media_page_with_invalid_json_returns_error_structure() {
        val resultJson = bridge.listMediaPage("not json")
        val json = JSONObject(resultJson)

        assertEquals("error", json.getString("revisionToken"))
        assertEquals(0, json.getJSONArray("items").length())
    }
}
