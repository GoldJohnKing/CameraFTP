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
type ThumbSizeBucket = 's' | 'm';

/** Priority level for thumbnail request scheduling */
type ThumbPriority = 'visible' | 'nearby';

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
type ThumbStatus = 'ready' | 'failed' | 'cancelled';

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
type ThumbErrorCode =
  | 'io_transient'
  | 'decode_corrupt'
  | 'permission_denied'
  | 'oom_guard'
  | 'cancelled';

// ===== Listener =====

/** Callback type for thumbnail results */
export type ThumbResultListener = (result: ThumbResult) => void;

// ===== Gallery Custom Events =====

/** Custom event for gallery items added (e.g. from FTP upload) */
export interface GalleryItemsAddedEvent extends CustomEvent {
  detail: { items: MediaItemDto[]; timestamp: number };
}

/** Custom event for gallery items deleted (e.g. from ImageViewerActivity) */
export interface GalleryItemsDeletedEvent extends CustomEvent {
  detail: { mediaIds: string[]; timestamp: number };
}
