/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * V2 Gallery Media Adapter
 *
 * Wraps the GalleryAndroidV2 JS Bridge into clean Promise-based TypeScript functions.
 * All bridge methods return JSON strings; this layer handles parsing and type safety.
 */

import type {
  MediaPageRequest,
  MediaPageResponse,
  QueueStats,
  ThumbRequest,
  ThumbResult,
  ThumbResultListener,
} from '../types/gallery-v2';

/** Internal listener registry keyed by listenerId */
const listeners = new Map<string, ThumbResultListener>();

/**
 * Check if the V2 gallery bridge is available on this platform
 */
export function isGalleryV2Available(): boolean {
  return typeof window !== 'undefined' && !!window.GalleryAndroidV2;
}

/**
 * Get the raw bridge, throwing if unavailable
 */
function getBridge(): NonNullable<typeof window.GalleryAndroidV2> {
  const bridge = window.GalleryAndroidV2;
  if (!bridge) {
    throw new Error('GalleryAndroidV2 bridge is not available');
  }
  return bridge;
}

/**
 * List a page of media items from MediaStore
 */
export async function listMediaPage(req: MediaPageRequest): Promise<MediaPageResponse> {
  const bridge = getBridge();
  const json = await bridge.listMediaPage(JSON.stringify(req));
  try {
    return JSON.parse(json) as MediaPageResponse;
  } catch (e) {
    throw new Error(`Failed to parse listMediaPage response: ${(e as Error).message}`);
  }
}

/**
 * Enqueue thumbnail generation requests
 */
export async function enqueueThumbnails(reqs: ThumbRequest[]): Promise<void> {
  console.log(`[GalleryV2] enqueueThumbnails: ${reqs.length} requests, bridge=${!!window.GalleryAndroidV2}`);
  const bridge = getBridge();
  const json = JSON.stringify(reqs);
  console.log(`[GalleryV2] enqueueThumbnails: json length=${json.length}`);
  await bridge.enqueueThumbnails(json);
  console.log(`[GalleryV2] enqueueThumbnails: done`);
}

/**
 * Cancel specific thumbnail requests by request ID
 */
export async function cancelThumbnailRequests(requestIds: string[]): Promise<void> {
  const bridge = getBridge();
  await bridge.cancelThumbnailRequests(JSON.stringify(requestIds));
}

/**
 * Cancel all thumbnail requests associated with a view
 */
export async function cancelByView(viewId: string): Promise<void> {
  const bridge = getBridge();
  await bridge.cancelByView(viewId);
}

/**
 * Register a listener for thumbnail results
 *
 * The bridge delivers results via a global callback mechanism.
 * This function registers both with the bridge and sets up the local callback dispatch.
 */
export async function registerThumbnailListener(
  viewId: string,
  listenerId: string,
  listener: ThumbResultListener,
): Promise<void> {
  const bridge = getBridge();
  await bridge.registerThumbnailListener(viewId, listenerId);
  listeners.set(listenerId, listener);
}

/**
 * Unregister a thumbnail result listener
 */
export async function unregisterThumbnailListener(listenerId: string): Promise<void> {
  const bridge = getBridge();
  listeners.delete(listenerId);
  await bridge.unregisterThumbnailListener(listenerId);
}

/**
 * Invalidate cached thumbnails for specific media IDs
 */
export async function invalidateMediaIds(mediaIds: string[]): Promise<void> {
  const bridge = getBridge();
  await bridge.invalidateMediaIds(JSON.stringify(mediaIds));
}

/**
 * Get current thumbnail queue statistics
 */
export async function getQueueStats(): Promise<QueueStats> {
  const bridge = getBridge();
  const json = await bridge.getQueueStats();
  try {
    return JSON.parse(json) as QueueStats;
  } catch (e) {
    throw new Error(`Failed to parse getQueueStats response: ${(e as Error).message}`);
  }
}

/**
 * Dispatch a thumbnail result to the registered listener.
 * Called by the Android bridge via a global callback.
 */
export function dispatchThumbnailResult(listenerId: string, resultJson: string): void {
  console.log(`[GalleryV2] dispatchThumbnailResult: listenerId=${listenerId} json=${resultJson.substring(0, 100)}`);
  const listener = listeners.get(listenerId);
  if (listener) {
    try {
      const result = JSON.parse(resultJson) as ThumbResult;
      console.log(`[GalleryV2] dispatchThumbnailResult: calling listener with status=${result.status} path=${result.localPath}`);
      listener(result);
    } catch (e) {
      console.error(`[GalleryV2] dispatchThumbnailResult parse error:`, e);
    }
  } else {
    console.warn(`[GalleryV2] dispatchThumbnailResult: no listener for ${listenerId}, registered: ${[...listeners.keys()]}`);
  }
}

/**
 * List a page of media items using V2 bridge (simplified API).
 * Returns empty response if bridge is unavailable.
 */
export async function listMediaPageV2(req: MediaPageRequest): Promise<MediaPageResponse> {
  if (!isGalleryV2Available()) {
    return { items: [], nextCursor: null, revisionToken: '' };
  }
  return listMediaPage(req);
}

// ===== V2 Adapter Functions =====
// Thin wrappers that match the spec contract for useThumbnailScheduler.

/**
 * Enqueue thumbnail generation requests (V2 API).
 * Delegates to the bridge's enqueueThumbnails method.
 */
export async function enqueueThumbnailsV2(reqs: ThumbRequest[]): Promise<void> {
  return enqueueThumbnails(reqs);
}

/**
 * Cancel specific thumbnail requests by request ID (V2 API).
 * Delegates to the bridge's cancelThumbnailRequests method.
 */
export async function cancelThumbnailRequestsV2(requestIds: string[]): Promise<void> {
  return cancelThumbnailRequests(requestIds);
}

/**
 * Register a thumbnail result listener (V2 API).
 * Sets up the global dispatch callback so the Android bridge can deliver results.
 */
export async function registerThumbnailListenerV2(
  viewId: string,
  listenerId: string,
  listener: ThumbResultListener,
): Promise<void> {
  await registerThumbnailListener(viewId, listenerId, listener);
  window.__galleryThumbDispatch = dispatchThumbnailResult;
}

/**
 * Unregister a thumbnail result listener (V2 API).
 */
export async function unregisterThumbnailListenerV2(listenerId: string): Promise<void> {
  await unregisterThumbnailListener(listenerId);
}
