/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.app.Activity
import android.util.Log
import com.gjk.cameraftpcompanion.MainActivity
import com.gjk.cameraftpcompanion.galleryv2.MediaPageProvider
import com.gjk.cameraftpcompanion.galleryv2.ThumbJob
import com.gjk.cameraftpcompanion.galleryv2.ThumbResult
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailCacheV2
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailDecoder
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailKeyV2
import com.gjk.cameraftpcompanion.galleryv2.ThumbnailPipelineManager
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap

class GalleryBridgeV2(
    context: Activity,
    private val mediaPageProvider: MediaPageProvider = MediaPageProvider(context),
    private val pipelineManager: ThumbnailPipelineManager = ThumbnailPipelineManager(),
    private val cache: ThumbnailCacheV2 = ThumbnailCacheV2()
) : BaseJsBridge(context) {

    companion object {
        private const val TAG = "GalleryBridgeV2"
    }

    /**
     * Maps listenerId → viewId for listener lifecycle management.
     */
    private val listenerMap = ConcurrentHashMap<String, String>()

    /**
     * Maps viewId → set of listenerIds for bulk invalidation on destroy.
     */
    private val viewListeners = ConcurrentHashMap<String, MutableSet<String>>()

    /**
     * Maps requestId → viewId so we can route results to the correct listeners.
     */
    private val requestViewMap = ConcurrentHashMap<String, String>()

    init {
        cache.initialize(activity)
        cache.cleanup() // Cleanup stale cache entries on startup
        pipelineManager.decoder = ThumbnailDecoder(activity)
        pipelineManager.cacheDir = java.io.File(activity.cacheDir, "thumb/v2")
        pipelineManager.cache = cache
        pipelineManager.onResult = { result -> dispatchResult(result) }
    }

    // ── Media paging ──────────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun listMediaPage(requestJson: String): String {
        Log.d(TAG, "listMediaPage: $requestJson")
        return try {
            val request = JSONObject(requestJson)
            val cursor = request.optString("cursor").takeIf { it.isNotEmpty() }
            val pageSize = request.optInt("pageSize", 50)

            val result = mediaPageProvider.listPage(cursor, pageSize)

            val json = JSONObject().apply {
                put("items", JSONArray().apply {
                    result.items.forEach { item ->
                        put(JSONObject().apply {
                            put("mediaId", item.mediaId)
                            put("uri", item.uri)
                            put("dateModifiedMs", item.dateModifiedMs)
                            put("width", item.width ?: JSONObject.NULL)
                            put("height", item.height ?: JSONObject.NULL)
                            put("mimeType", item.mimeType ?: JSONObject.NULL)
                            put("displayName", item.displayName ?: JSONObject.NULL)
                        })
                    }
                })
                put("nextCursor", result.nextCursor ?: JSONObject.NULL)
                put("revisionToken", result.revisionToken)
            }
            json.toString()
        } catch (e: Exception) {
            Log.e(TAG, "listMediaPage error", e)
            """{"items":[],"nextCursor":null,"revisionToken":"error"}"""
        }
    }

    // ── Thumbnail queue ───────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun enqueueThumbnails(requestsJson: String) {
        try {
            val requests = JSONArray(requestsJson)
            Log.d(TAG, "enqueueThumbnails: ${requests.length()} requests")
            var accepted = 0
            var cacheHits = 0
            for (i in 0 until requests.length()) {
                val req = requests.getJSONObject(i)
                val requestId = req.getString("requestId")
                val mediaId = req.getString("mediaId")
                val dateModifiedMs = req.getLong("dateModifiedMs")
                val sizeBucket = req.getString("sizeBucket")
                val viewId = req.getString("viewId")

                // Check L2 cache first
                val key = ThumbnailKeyV2.of(mediaId, dateModifiedMs, sizeBucket, 0, 0)
                val cachedFile = cache.get(mediaId, key, sizeBucket)
                if (cachedFile != null) {
                    // Cache hit - deliver result immediately without enqueueing
                    cacheHits++
                    pipelineManager.recordCacheHit()
                    val result = ThumbResult(
                        requestId = requestId,
                        mediaId = mediaId,
                        status = "ready",
                        localPath = cachedFile.absolutePath,
                        errorCode = null
                    )
                    requestViewMap[requestId] = viewId
                    dispatchResult(result)
                    continue
                }

                // Cache miss - enqueue for decoding
                val job = ThumbJob(
                    requestId = requestId,
                    mediaId = mediaId,
                    uri = req.getString("uri"),
                    dateModifiedMs = dateModifiedMs,
                    sizeBucket = sizeBucket,
                    priority = req.getString("priority"),
                    viewId = viewId
                )
                if (pipelineManager.enqueue(job)) {
                    requestViewMap[job.requestId] = job.viewId
                    accepted++
                } else {
                    Log.w(TAG, "enqueueThumbnails: rejected job ${job.requestId} mediaId=${job.mediaId}")
                }
            }
            Log.d(TAG, "enqueueThumbnails: accepted=$accepted cacheHits=$cacheHits/${requests.length()}, pending=${pipelineManager.pendingCount()}")
            // Kick off processing for accepted jobs
            repeat(accepted) { pipelineManager.processNext() }
        } catch (e: Exception) {
            Log.e(TAG, "enqueueThumbnails error", e)
        }
    }

    @android.webkit.JavascriptInterface
    fun cancelThumbnailRequests(requestIdsJson: String) {
        Log.d(TAG, "cancelThumbnailRequests")
        try {
            val ids = JSONArray(requestIdsJson)
            for (i in 0 until ids.length()) {
                val id = ids.getString(i)
                pipelineManager.cancel(id)
                requestViewMap.remove(id)
            }
        } catch (e: Exception) {
            Log.e(TAG, "cancelThumbnailRequests error", e)
        }
    }

    @android.webkit.JavascriptInterface
    fun cancelByView(viewId: String) {
        Log.d(TAG, "cancelByView: viewId=$viewId")
        try {
            pipelineManager.cancelByView(viewId)
            val iterator = requestViewMap.entries.iterator()
            while (iterator.hasNext()) {
                if (iterator.next().value == viewId) iterator.remove()
            }
        } catch (e: Exception) {
            Log.e(TAG, "cancelByView error", e)
        }
    }

    // ── Listener lifecycle ────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun registerThumbnailListener(viewId: String, listenerId: String) {
        Log.d(TAG, "registerThumbnailListener: viewId=$viewId, listenerId=$listenerId")
        listenerMap[listenerId] = viewId
        viewListeners.getOrPut(viewId) { ConcurrentHashMap.newKeySet() }.add(listenerId)
    }

    @android.webkit.JavascriptInterface
    fun unregisterThumbnailListener(listenerId: String) {
        Log.d(TAG, "unregisterThumbnailListener: listenerId=$listenerId")
        val viewId = listenerMap.remove(listenerId)
        if (viewId != null) {
            viewListeners[viewId]?.remove(listenerId)
        }
    }

    /**
     * Invalidate all listeners registered under a viewId.
     * Called when the Activity or WebView is destroyed.
     */
    fun invalidateListenersForView(viewId: String) {
        Log.d(TAG, "invalidateListenersForView: viewId=$viewId")
        val ids = viewListeners.remove(viewId) ?: return
        ids.forEach { listenerMap.remove(it) }
        pipelineManager.cancelByView(viewId)
        val iterator = requestViewMap.entries.iterator()
        while (iterator.hasNext()) {
            if (iterator.next().value == viewId) iterator.remove()
        }
    }

    // ── Cache invalidation ────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun invalidateMediaIds(mediaIdsJson: String) {
        Log.d(TAG, "invalidateMediaIds: $mediaIdsJson")
        try {
            val ids = JSONArray(mediaIdsJson)
            val mediaIds = mutableSetOf<String>()
            for (i in 0 until ids.length()) {
                mediaIds.add(ids.getString(i))
            }
            cache.invalidateByMediaId(mediaIds)
        } catch (e: Exception) {
            Log.e(TAG, "invalidateMediaIds error", e)
        }
    }

    // ── Stats ─────────────────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun getQueueStats(): String {
        return try {
            val stats = pipelineManager.queueStats()
            JSONObject().apply {
                put("pending", stats.pending)
                put("running", stats.running)
                put("cacheHitRate", stats.cacheHitRate)
            }.toString()
        } catch (e: Exception) {
            Log.e(TAG, "getQueueStats error", e)
            """{"pending":0,"running":0,"cacheHitRate":0.0}"""
        }
    }

    // ── Internal dispatch ─────────────────────────────────────────────

    private fun dispatchResult(result: ThumbResult) {
        val viewId = requestViewMap.remove(result.requestId)
        if (viewId == null) {
            Log.w(TAG, "dispatchResult: no viewId for requestId=${result.requestId}")
            return
        }
        val listenerIds = viewListeners[viewId]
        if (listenerIds == null || listenerIds.isEmpty()) {
            Log.w(TAG, "dispatchResult: no listeners for viewId=$viewId")
            return
        }

        val payload = JSONObject().apply {
            put("requestId", result.requestId)
            put("mediaId", result.mediaId)
            put("status", result.status)
            put("localPath", result.localPath ?: JSONObject.NULL)
            put("errorCode", result.errorCode ?: JSONObject.NULL)
        }.toString()

        Log.d(TAG, "dispatchResult: mediaId=${result.mediaId} status=${result.status} path=${result.localPath} listeners=${listenerIds.size}")
        for (listenerId in listenerIds) {
            dispatchThumbBatch(listenerId, payload)
        }
    }

    private fun dispatchThumbBatch(listenerId: String, payload: String) {
        val script = "window.__galleryThumbDispatch('$listenerId', '${payload.replace("'", "\\'")}')"
        runOnUiThread {
            (activity as? MainActivity)?.getWebView()?.evaluateJavascript(script, null)
        }
    }
}
