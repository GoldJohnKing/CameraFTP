/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import {
  GALLERY_REFRESH_REQUESTED_EVENT,
  LATEST_PHOTO_REFRESH_REQUESTED_EVENT,
  requestMediaLibraryRefresh,
} from '../gallery-refresh';

describe('gallery-refresh', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('dispatches gallery and latest-photo refresh events together', () => {
    const galleryHandler = vi.fn();
    const latestHandler = vi.fn();

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, galleryHandler);
    window.addEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, latestHandler);

    requestMediaLibraryRefresh({ reason: 'manual' });

    expect(galleryHandler).toHaveBeenCalledTimes(1);
    expect(latestHandler).toHaveBeenCalledTimes(1);
    expect(galleryHandler.mock.calls[0]?.[0]).toMatchObject({
      detail: { reason: 'manual' },
    });
    expect(latestHandler.mock.calls[0]?.[0]).toMatchObject({
      detail: { reason: 'manual' },
    });
  });

  it('dispatches explicit delete refresh events to gallery and latest-photo listeners', () => {
    const galleryHandler = vi.fn();
    const latestHandler = vi.fn();

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, galleryHandler);
    window.addEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, latestHandler);

    requestMediaLibraryRefresh({ reason: 'delete', timestamp: 123 });

    expect(galleryHandler).toHaveBeenCalledTimes(1);
    expect(latestHandler).toHaveBeenCalledTimes(1);
    expect(galleryHandler.mock.calls[0]?.[0]).toMatchObject({
      detail: { reason: 'delete', timestamp: 123 },
    });
    expect(latestHandler.mock.calls[0]?.[0]).toMatchObject({
      detail: { reason: 'delete', timestamp: 123 },
    });
  });

  it('emits runtime refresh output only for active refresh events, not media-store-ready', () => {
    const runtimeRefreshOutput: Array<string> = [];
    const dispatchRuntimeRefresh = (event: Event): void => {
      const detail = (event as CustomEvent<{ reason: string }>).detail;
      runtimeRefreshOutput.push(detail.reason);
    };

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, dispatchRuntimeRefresh);

    window.dispatchEvent(new CustomEvent('media-store-ready', {
      detail: { reason: 'upload', uri: 'content://media/1' },
    }));
    expect(runtimeRefreshOutput).toEqual([]);

    requestMediaLibraryRefresh({ reason: 'manual' });
    expect(runtimeRefreshOutput).toEqual(['manual']);
  });
});
