/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export const GALLERY_REFRESH_REQUESTED_EVENT = 'gallery-refresh-requested';
export const LATEST_PHOTO_REFRESH_REQUESTED_EVENT = 'latest-photo-refresh-requested';

type MediaLibraryRefreshReason =
  | 'manual'
  | 'upload'
  | 'delete'
  | 'permission-granted'
  | 'activity-resume';

interface MediaLibraryRefreshDetail {
  reason: MediaLibraryRefreshReason;
  uri?: string;
  displayName?: string;
  timestamp?: number;
}

function dispatchRefreshEvent(eventName: string, detail: MediaLibraryRefreshDetail): void {
  window.dispatchEvent(new CustomEvent<MediaLibraryRefreshDetail>(eventName, { detail }));
}

export function requestMediaLibraryRefresh(detail: MediaLibraryRefreshDetail): void {
  dispatchRefreshEvent(GALLERY_REFRESH_REQUESTED_EVENT, detail);
  dispatchRefreshEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, detail);
}
