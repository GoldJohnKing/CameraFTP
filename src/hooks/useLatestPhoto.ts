/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useSyncExternalStore } from 'react';
import { listen } from '@tauri-apps/api/event';
import { LATEST_PHOTO_REFRESH_REQUESTED_EVENT } from '../utils/gallery-refresh';
import {
  fetchLatestPhotoFile,
  type LatestPhotoFile,
} from '../services/latest-photo';
import { isGalleryV2Available } from '../services/gallery-media-v2';

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

interface LatestPhotoSnapshot {
  latestPhoto: LatestPhotoFile | null;
}

type StoreListener = () => void;

let snapshot: LatestPhotoSnapshot = {
  latestPhoto: null,
};

const listeners = new Set<StoreListener>();
let isInitialized = false;
let teardownFn: (() => void) | null = null;

/** Encapsulates in-flight refresh state with a dirty flag for follow-up. */
const refreshCoordinator = {
  _inFlight: null as Promise<LatestPhotoFile | null> | null,
  _dirty: false,

  trigger(): Promise<LatestPhotoFile | null> {
    if (!this._inFlight) {
      this._inFlight = runRefreshLatestPhoto().finally(() => {
        this._inFlight = null;
        if (this._dirty) {
          this._dirty = false;
          void this.trigger();
        }
      });
    } else {
      this._dirty = true;
    }
    return this._inFlight;
  },
};

function emit(): void {
  listeners.forEach((listener) => listener());
}

function isSameLatestPhoto(
  left: LatestPhotoFile | null,
  right: LatestPhotoFile | null,
): boolean {
  if (left === right) {
    return true;
  }

  if (!left || !right) {
    return false;
  }

  return left.filename === right.filename && left.path === right.path;
}

function setLatestPhoto(nextLatestPhoto: LatestPhotoFile | null): void {
  if (isSameLatestPhoto(snapshot.latestPhoto, nextLatestPhoto)) {
    return;
  }

  snapshot = {
    latestPhoto: nextLatestPhoto,
  };
  emit();
}

async function runRefreshLatestPhoto(): Promise<LatestPhotoFile | null> {
  try {
    const latest = await fetchLatestPhotoFile();
    setLatestPhoto(latest);
    return latest;
  } catch (err) {
    console.error('[useLatestPhoto] Failed to fetch latest image:', err);
    return null;
  }
}

function refreshLatestPhoto(): Promise<LatestPhotoFile | null> {
  return refreshCoordinator.trigger();
}

function initializeStore(): void {
  if (isInitialized) {
    return;
  }

  isInitialized = true;

  const handleRefreshRequest = () => {
    void refreshLatestPhoto();
  };
  const handleGalleryItemsAdded = () => {
    void refreshLatestPhoto();
  };
  const galleryV2Available = isGalleryV2Available();

  window.addEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, handleRefreshRequest);
  if (galleryV2Available) {
    window.addEventListener('gallery-items-added', handleGalleryItemsAdded);
  }

  const unlistenPromise = listen<FileIndexChangedEvent>('file-index-changed', (event) => {
    if (event.payload.count === 0) {
      setLatestPhoto(null);
      return;
    }

    void refreshLatestPhoto();
  });

  teardownFn = () => {
    window.removeEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, handleRefreshRequest);
    if (galleryV2Available) {
      window.removeEventListener('gallery-items-added', handleGalleryItemsAdded);
    }
    void unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
  };

  void refreshLatestPhoto();
}

function disposeStore(): void {
  if (!isInitialized) {
    return;
  }

  teardownFn?.();
  teardownFn = null;
  isInitialized = false;
}

function subscribe(listener: StoreListener): () => void {
  listeners.add(listener);
  initializeStore();

  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      disposeStore();
    }
  };
}

function getSnapshot(): LatestPhotoSnapshot {
  return snapshot;
}

export function useLatestPhoto() {
  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    latestPhoto: current.latestPhoto,
    refreshLatestPhoto,
  };
}
