/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * V2 Gallery Media Types
 *
 * DTOs for the async batched thumbnail pipeline.
 */

// ===== Paging =====

/** Cursor token for media pagination (opaque string or null for first page) */
export type MediaCursor = string | null;

/** Request to list a page of media items from MediaStore */
export interface MediaPageRequest {
  cursor: MediaCursor;
  pageSize: number;
  sort: 'dateDesc';
}

/** Single media item returned from a page query */
export interface MediaItemDto {
  mediaId: string;
  uri: string;
  dateModifiedMs: number;
  width: number | null;
  height: number | null;
  mimeType: string | null;
  displayName: string | null;
}

/** Response from a media page query */
export interface MediaPageResponse {
  items: MediaItemDto[];
  nextCursor: MediaCursor;
  revisionToken: string;
  totalCount: number;
}

// ===== Thumbnails =====

/** Size bucket for thumbnail generation */
export type ThumbSizeBucket = 's' | 'm';

/** Priority level for thumbnail request scheduling */
export type ThumbPriority = 'visible' | 'nearby' | 'prefetch';

/** Request to enqueue a single thumbnail for generation */
export interface ThumbRequest {
  requestId: string;
  mediaId: string;
  uri: string;
  dateModifiedMs: number;
  sizeBucket: ThumbSizeBucket;
  priority: ThumbPriority;
  viewId: string;
}

/** Status of a completed thumbnail result */
export type ThumbStatus = 'ready' | 'failed' | 'cancelled';

/** Result delivered via listener when a thumbnail is ready or failed */
export interface ThumbResult {
  requestId: string;
  mediaId: string;
  status: ThumbStatus;
  localPath?: string;
  errorCode?: ThumbErrorCode;
}

// ===== Error Codes =====

/** Known thumbnail pipeline error codes */
export type ThumbErrorCode =
  | 'io_transient'
  | 'decode_corrupt'
  | 'permission_denied'
  | 'oom_guard'
  | 'cancelled';

// ===== Queue Stats =====

/** Current state of the thumbnail processing queue */
export interface QueueStats {
  pending: number;
  running: number;
  cacheHitRate: number;
}

// ===== Listener =====

/** Callback type for thumbnail results */
export type ThumbResultListener = (result: ThumbResult) => void;
