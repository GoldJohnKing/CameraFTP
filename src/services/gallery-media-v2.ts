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
  return JSON.parse(json) as MediaPageResponse;
}

/**
 * Enqueue thumbnail generation requests
 */
export async function enqueueThumbnails(reqs: ThumbRequest[]): Promise<void> {
  const bridge = getBridge();
  await bridge.enqueueThumbnails(JSON.stringify(reqs));
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
  listeners.set(listenerId, listener);
  await bridge.registerThumbnailListener(viewId, listenerId);
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
  return JSON.parse(json) as QueueStats;
}

/**
 * Dispatch a thumbnail result to the registered listener.
 * Called by the Android bridge via a global callback.
 */
export function dispatchThumbnailResult(listenerId: string, resultJson: string): void {
  const listener = listeners.get(listenerId);
  if (listener) {
    const result = JSON.parse(resultJson) as ThumbResult;
    listener(result);
  }
}
