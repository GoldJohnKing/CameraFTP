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
  ThumbRequest,
  ThumbResult,
  ThumbResultListener,
} from '../types';

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
  const bridge = getBridge();
  const json = JSON.stringify(reqs);
  await bridge.enqueueThumbnails(json);
}

/**
 * Cancel specific thumbnail requests by request ID
 */
export async function cancelThumbnailRequests(requestIds: string[]): Promise<void> {
  const bridge = getBridge();
  await bridge.cancelThumbnailRequests(JSON.stringify(requestIds));
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
  window.__galleryThumbDispatch = dispatchThumbnailResult;
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
 * Dispatch a thumbnail result to the registered listener.
 * Called by the Android bridge via a global callback.
 */
function dispatchThumbnailResult(listenerId: string, resultJson: string): void {
  const listener = listeners.get(listenerId);
  if (listener) {
    try {
      const result = JSON.parse(resultJson) as ThumbResult;
      listener(result);
    } catch {
      // Ignore malformed result JSON
    }
  }
}
