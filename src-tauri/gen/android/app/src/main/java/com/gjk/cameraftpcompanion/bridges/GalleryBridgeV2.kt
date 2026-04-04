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

    @Volatile
    private var destroyed = false
    private val stateLock = Any()

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
                put("totalCount", result.totalCount)
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
        synchronized(stateLock) {
            if (destroyed) {
                Log.w(TAG, "enqueueThumbnails: bridge already destroyed")
                return
            }

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

                    val key = ThumbnailKeyV2.of(mediaId, dateModifiedMs, sizeBucket, 0, 0)
                    val cachedFile = cache.get(mediaId, key, sizeBucket)
                    if (cachedFile != null) {
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
                repeat(accepted) { pipelineManager.processNext() }
            } catch (e: Exception) {
                Log.e(TAG, "enqueueThumbnails error", e)
            }
        }
    }

    @android.webkit.JavascriptInterface
    fun cancelThumbnailRequests(requestIdsJson: String) {
        synchronized(stateLock) {
            if (destroyed) {
                return
            }

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
    }

    // ── Listener lifecycle ────────────────────────────────────────────

    @android.webkit.JavascriptInterface
    fun registerThumbnailListener(viewId: String, listenerId: String) {
        synchronized(stateLock) {
            if (destroyed) {
                Log.w(TAG, "registerThumbnailListener: bridge already destroyed")
                return
            }

            Log.d(TAG, "registerThumbnailListener: viewId=$viewId, listenerId=$listenerId")
            listenerMap[listenerId] = viewId
            viewListeners.getOrPut(viewId) { ConcurrentHashMap.newKeySet() }.add(listenerId)
        }
    }

    @android.webkit.JavascriptInterface
    fun unregisterThumbnailListener(listenerId: String) {
        synchronized(stateLock) {
            if (destroyed) {
                return
            }

            Log.d(TAG, "unregisterThumbnailListener: listenerId=$listenerId")
            val viewId = listenerMap.remove(listenerId)
            if (viewId != null) {
                viewListeners[viewId]?.remove(listenerId)
            }
        }
    }

    fun destroy() {
        synchronized(stateLock) {
            if (destroyed) {
                return
            }

            destroyed = true
            val viewIds = viewListeners.keys.toList() + requestViewMap.values.toList()
            viewIds.distinct().forEach { viewId ->
                pipelineManager.cancelByView(viewId)
            }
            listenerMap.clear()
            viewListeners.clear()
            requestViewMap.clear()
        }
        pipelineManager.shutdown()
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

    // ── Internal dispatch ─────────────────────────────────────────────

    private fun dispatchResult(result: ThumbResult) {
        synchronized(stateLock) {
            if (destroyed) {
                return
            }

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
    }

    private fun dispatchThumbBatch(listenerId: String, payload: String) {
        if (destroyed) {
            return
        }

        val script = "window.__galleryThumbDispatch('$listenerId', '${payload.replace("'", "\\'")}')"
        runOnUiThread {
            if (destroyed) {
                return@runOnUiThread
            }
            (activity as? MainActivity)?.getWebView()?.evaluateJavascript(script, null)
        }
    }
}
