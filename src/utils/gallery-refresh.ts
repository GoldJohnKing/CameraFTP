/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export const GALLERY_REFRESH_REQUESTED_EVENT = 'gallery-refresh-requested';
export const LATEST_PHOTO_REFRESH_REQUESTED_EVENT = 'latest-photo-refresh-requested';

export type MediaLibraryRefreshReason =
  | 'manual'
  | 'upload'
  | 'delete'
  | 'permission-granted'
  | 'media-store-ready';

export interface MediaLibraryRefreshDetail {
  reason: MediaLibraryRefreshReason;
  uri?: string;
  displayName?: string;
  timestamp?: number;
}

let trailingRefreshTimer: ReturnType<typeof setTimeout> | null = null;
let trailingRefreshDetail: MediaLibraryRefreshDetail | null = null;

function dispatchRefreshEvent(eventName: string, detail: MediaLibraryRefreshDetail): void {
  window.dispatchEvent(new CustomEvent<MediaLibraryRefreshDetail>(eventName, { detail }));
}

export function requestLatestPhotoRefresh(detail: MediaLibraryRefreshDetail): void {
  dispatchRefreshEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, detail);
}

export function requestMediaLibraryRefresh(detail: MediaLibraryRefreshDetail): void {
  dispatchRefreshEvent(GALLERY_REFRESH_REQUESTED_EVENT, detail);
  dispatchRefreshEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, detail);
}

export function scheduleMediaLibraryRefresh(
  detail: MediaLibraryRefreshDetail,
  debounceMs = 300,
): void {
  if (trailingRefreshTimer === null) {
    requestMediaLibraryRefresh(detail);
    trailingRefreshTimer = setTimeout(() => {
      trailingRefreshTimer = null;
      if (trailingRefreshDetail) {
        const nextDetail = trailingRefreshDetail;
        trailingRefreshDetail = null;
        requestMediaLibraryRefresh(nextDetail);
      }
    }, debounceMs);
    return;
  }

  trailingRefreshDetail = detail;
}
