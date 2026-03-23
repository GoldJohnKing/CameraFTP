/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { fetchLatestPhotoFile } from '../latest-photo';

const { invokeMock, isGalleryV2AvailableMock, listMediaPageV2Mock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isGalleryV2AvailableMock: vi.fn(),
  listMediaPageV2Mock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../gallery-media-v2', () => ({
  isGalleryV2Available: isGalleryV2AvailableMock,
  listMediaPageV2: listMediaPageV2Mock,
}));

describe('latest-photo service', () => {
  it('returns null when V2 bridge is available but empty', async () => {
    isGalleryV2AvailableMock.mockReturnValue(true);
    listMediaPageV2Mock.mockResolvedValue({ items: [], nextCursor: null, revisionToken: '' });

    await expect(fetchLatestPhotoFile()).resolves.toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('returns latest mapped image from V2 bridge when available', async () => {
    isGalleryV2AvailableMock.mockReturnValue(true);
    listMediaPageV2Mock.mockResolvedValue({
      items: [
        { mediaId: '2', uri: 'content://latest/latest.jpg', dateModifiedMs: 200, width: 1920, height: 1080, mimeType: 'image/jpeg' },
      ],
      nextCursor: null,
      revisionToken: 'tok',
    });

    await expect(fetchLatestPhotoFile()).resolves.toEqual({
      path: 'content://latest/latest.jpg',
      filename: 'latest.jpg',
    });
    expect(listMediaPageV2Mock).toHaveBeenCalledWith({ cursor: null, pageSize: 1, sort: 'dateDesc' });
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('falls back to tauri command when V2 bridge is unavailable', async () => {
    isGalleryV2AvailableMock.mockReturnValue(false);
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
