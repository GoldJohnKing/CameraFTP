/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { fetchLatestPhotoFile } from '../latest-photo';

const { invokeMock, isGalleryMediaAvailableMock, listGalleryMediaMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isGalleryMediaAvailableMock: vi.fn(),
  listGalleryMediaMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../gallery-media', () => ({
  isGalleryMediaAvailable: isGalleryMediaAvailableMock,
  listGalleryMedia: listGalleryMediaMock,
}));

describe('latest-photo service', () => {
  it('returns null when gallery-media is available but empty', async () => {
    isGalleryMediaAvailableMock.mockReturnValue(true);
    listGalleryMediaMock.mockResolvedValue([]);

    await expect(fetchLatestPhotoFile()).resolves.toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('returns latest mapped image from gallery-media service when available', async () => {
    isGalleryMediaAvailableMock.mockReturnValue(true);
    listGalleryMediaMock.mockResolvedValue([
      { path: 'content://latest', filename: 'latest.jpg', sortTime: 200 },
      { path: 'content://older', filename: 'older.jpg', sortTime: 100 },
    ]);

    await expect(fetchLatestPhotoFile()).resolves.toEqual({
      path: 'content://latest',
      filename: 'latest.jpg',
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('falls back to tauri command when gallery-media is unavailable', async () => {
    isGalleryMediaAvailableMock.mockReturnValue(false);
    invokeMock.mockResolvedValue({
      path: '/tmp/latest.jpg',
      filename: 'latest.jpg',
      size: 123,
    });

    await expect(fetchLatestPhotoFile()).resolves.toEqual({
      path: '/tmp/latest.jpg',
      filename: 'latest.jpg',
      size: 123,
    });
    expect(invokeMock).toHaveBeenCalledWith('get_latest_image');
  });
});
