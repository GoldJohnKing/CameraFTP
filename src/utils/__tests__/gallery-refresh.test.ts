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
  scheduleMediaLibraryRefresh,
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

  it('coalesces burst upload refreshes into immediate and trailing refreshes', () => {
    const galleryHandler = vi.fn();

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, galleryHandler);

    scheduleMediaLibraryRefresh({ reason: 'upload' }, 250);
    scheduleMediaLibraryRefresh({ reason: 'upload' }, 250);
    scheduleMediaLibraryRefresh({ reason: 'upload' }, 250);

    expect(galleryHandler).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(249);
    expect(galleryHandler).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(1);
    expect(galleryHandler).toHaveBeenCalledTimes(2);
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
});
