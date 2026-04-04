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
    const openViewer = vi.fn().mockReturnValue(true);
    const onExifResult = vi.fn();

    window.ImageViewerAndroid = {
      openViewer,
      openOrNavigateTo: vi.fn().mockReturnValue(false),
      isAppVisible: vi.fn().mockReturnValue(true),
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

  it('uses filePath URI when URI list provider is not provided', async () => {
    const openViewer = vi.fn().mockReturnValue(true);
    window.ImageViewerAndroid = {
      openViewer,
      openOrNavigateTo: vi.fn().mockReturnValue(false),
      isAppVisible: vi.fn().mockReturnValue(true),
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('content://media/3'),
    };

    vi.mocked(invoke).mockResolvedValueOnce(null);

    await openImagePreview({
      filePath: 'content://media/3',
      openMethod: 'built-in-viewer',
    });

    expect(openViewer).toHaveBeenCalledWith('content://media/3', JSON.stringify(['content://media/3']));
  });

  it('uses openOrNavigateTo when preferReuse is true', async () => {
    const openViewer = vi.fn().mockReturnValue(true);
    const openOrNavigateTo = vi.fn().mockReturnValue(true);

    window.ImageViewerAndroid = {
      openViewer,
      openOrNavigateTo,
      isAppVisible: vi.fn().mockReturnValue(true),
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('/real/path.jpg'),
    };

    vi.mocked(invoke).mockResolvedValueOnce(null);

    await openImagePreview({
      filePath: 'content://media/1',
      openMethod: 'built-in-viewer',
      allUris: ['content://media/1', 'content://media/2'],
      preferReuse: true,
    });

    expect(openOrNavigateTo).toHaveBeenCalledWith(
      'content://media/1',
      JSON.stringify(['content://media/1', 'content://media/2']),
    );
    expect(openViewer).not.toHaveBeenCalled();
  });

  it('uses getAllUris provider to construct URI list', async () => {
    const openViewer = vi.fn().mockReturnValue(true);
    window.ImageViewerAndroid = {
      openViewer,
      openOrNavigateTo: vi.fn().mockReturnValue(false),
      isAppVisible: vi.fn().mockReturnValue(true),
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('/real/path.jpg'),
    };

    await openImagePreview({
      filePath: 'content://media/5',
      openMethod: 'built-in-viewer',
      getAllUris: async () => ['content://media/5', 'content://media/4'],
    });

    expect(openViewer).toHaveBeenCalledWith(
      'content://media/5',
      JSON.stringify(['content://media/5', 'content://media/4']),
    );
  });

  it('falls back to openViewer when openOrNavigateTo returns false', async () => {
    const openViewer = vi.fn().mockReturnValue(true);
    const openOrNavigateTo = vi.fn().mockReturnValue(false);

    window.ImageViewerAndroid = {
      openViewer,
      openOrNavigateTo,
      isAppVisible: vi.fn().mockReturnValue(true),
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('/real/path.jpg'),
    };

    await openImagePreview({
      filePath: 'content://media/1',
      openMethod: 'built-in-viewer',
      allUris: ['content://media/1', 'content://media/2'],
      preferReuse: true,
    });

    expect(openOrNavigateTo).toHaveBeenCalledTimes(1);
    expect(openViewer).toHaveBeenCalledWith(
      'content://media/1',
      JSON.stringify(['content://media/1', 'content://media/2']),
    );
  });

  it('falls back to chooser when built-in viewer bridge call fails', async () => {
    const openImageWithChooser = vi.fn();
    window.PermissionAndroid = {
      openImageWithChooser,
    } as unknown as Window['PermissionAndroid'];

    window.ImageViewerAndroid = {
      openViewer: vi.fn().mockImplementation(() => {
        throw new Error('bridge failed');
      }),
      openOrNavigateTo: vi.fn().mockReturnValue(false),
      isAppVisible: vi.fn().mockReturnValue(true),
      closeViewer: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn().mockReturnValue('/real/path.jpg'),
    };

    await expect(
      openImagePreview({
        filePath: '/tmp/pic.jpg',
        openMethod: 'built-in-viewer',
      }),
    ).resolves.toBeUndefined();

    expect(openImageWithChooser).toHaveBeenCalledWith('/tmp/pic.jpg');
  });

  it('falls back to Android chooser when built-in viewer is unavailable', async () => {
    const openImageWithChooser = vi.fn().mockReturnValue(JSON.stringify({ success: true }));
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

  it('falls back to preview window when chooser returns legacy boolean true', async () => {
    const openImageWithChooser = vi.fn().mockReturnValue(true);
    window.PermissionAndroid = {
      openImageWithChooser,
    } as unknown as Window['PermissionAndroid'];
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
      openMethod: 'built-in-viewer',
    });

    expect(openImageWithChooser).toHaveBeenCalledWith('/tmp/pic.jpg');
    expect(invoke).toHaveBeenCalledWith('open_preview_window', { filePath: '/tmp/pic.jpg' });
  });

  it('falls back to preview window when chooser reports failure', async () => {
    const openImageWithChooser = vi.fn().mockReturnValue(JSON.stringify({ success: false }));
    window.PermissionAndroid = {
      openImageWithChooser,
    } as unknown as Window['PermissionAndroid'];
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
      openMethod: 'built-in-viewer',
    });

    expect(openImageWithChooser).toHaveBeenCalledWith('/tmp/pic.jpg');
    expect(invoke).toHaveBeenCalledWith('open_preview_window', { filePath: '/tmp/pic.jpg' });
  });

  it('falls back to preview window when chooser throws', async () => {
    const openImageWithChooser = vi.fn().mockImplementation(() => {
      throw new Error('chooser failed');
    });
    window.PermissionAndroid = {
      openImageWithChooser,
    } as unknown as Window['PermissionAndroid'];
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
      openMethod: 'built-in-viewer',
    });

    expect(openImageWithChooser).toHaveBeenCalledWith('/tmp/pic.jpg');
    expect(invoke).toHaveBeenCalledWith('open_preview_window', { filePath: '/tmp/pic.jpg' });
  });

  it('falls back to preview window off Android', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await openImagePreview({
      filePath: '/tmp/pic.jpg',
    });

    expect(invoke).toHaveBeenCalledWith('open_preview_window', { filePath: '/tmp/pic.jpg' });
  });
});
