/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { openImagePreview } from '../image-open';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('image-open service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.GalleryAndroid = undefined;
    window.PermissionAndroid = undefined;
    window.ImageViewerAndroid = undefined;
  });

  it('opens built-in viewer with provided URI list and requests EXIF', async () => {
    const openViewer = vi.fn();
    const onExifResult = vi.fn();

    window.ImageViewerAndroid = {
      openViewer,
      closeViewer: vi.fn(),
      onExifResult,
      resolveFilePath: vi.fn().mockReturnValue('/real/path.jpg'),
    };

    vi.mocked(invoke).mockResolvedValueOnce({ iso: 100 });

    await openImagePreview({
      filePath: 'content://media/1',
      openMethod: 'built-in-viewer',
      allUris: ['content://media/1', 'content://media/2'],
    });

    expect(openViewer).toHaveBeenCalledWith('content://media/1', JSON.stringify(['content://media/1', 'content://media/2']));
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith('get_image_exif', { filePath: '/real/path.jpg' });
    expect(onExifResult).toHaveBeenCalledWith(JSON.stringify({ iso: 100 }));
  });

  it('loads MediaStore URIs for built-in viewer when list is not provided', async () => {
    const openViewer = vi.fn();
    window.ImageViewerAndroid = {
      openViewer,
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('content://media/3'),
    };

    window.GalleryAndroid = {
      listMediaStoreImages: vi.fn().mockResolvedValue(JSON.stringify([
        { uri: 'content://media/3', displayName: 'c.jpg', dateModified: 3 },
        { uri: 'content://media/2', displayName: 'b.jpg', dateModified: 2 },
      ])),
    } as unknown as Window['GalleryAndroid'];

    vi.mocked(invoke).mockResolvedValueOnce(null);

    await openImagePreview({
      filePath: 'content://media/3',
      openMethod: 'built-in-viewer',
    });

    expect(openViewer).toHaveBeenCalledWith('content://media/3', JSON.stringify(['content://media/3', 'content://media/2']));
  });

  it('falls back to Android chooser when built-in viewer is unavailable', async () => {
    const openImageWithChooser = vi.fn();
    window.PermissionAndroid = {
      openImageWithChooser,
    } as unknown as Window['PermissionAndroid'];

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
      openMethod: 'built-in-viewer',
    });

    expect(openImageWithChooser).toHaveBeenCalledWith('/tmp/pic.jpg');
    expect(invoke).not.toHaveBeenCalledWith('open_preview_window', expect.anything());
  });

  it('falls back to preview window off Android', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
    });

    expect(invoke).toHaveBeenCalledWith('open_preview_window', { filePath: '/tmp/pic.jpg' });
  });
});
