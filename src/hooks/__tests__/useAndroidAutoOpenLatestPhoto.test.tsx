/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AndroidImageOpenMethod } from '../../types';
import type { MediaItemDto } from '../../types/gallery-v2';
import { useAndroidAutoOpenLatestPhoto } from '../useAndroidAutoOpenLatestPhoto';

const { openImagePreviewMock } = vi.hoisted(() => ({
  openImagePreviewMock: vi.fn(),
}));

vi.mock('../../services/image-open', () => ({
  openImagePreview: openImagePreviewMock,
}));

interface HarnessProps {
  galleryItems: MediaItemDto[];
  openMethod?: AndroidImageOpenMethod;
  autoOpenLatestWhenVisible?: boolean;
}

function Harness(props: HarnessProps) {
  useAndroidAutoOpenLatestPhoto(props);
  return null;
}

function createItem(mediaId: string, uri: string): MediaItemDto {
  return {
    mediaId,
    uri,
    dateModifiedMs: 0,
    width: null,
    height: null,
    mimeType: 'image/jpeg',
    displayName: `${mediaId}.jpg`,
  };
}

describe('useAndroidAutoOpenLatestPhoto', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    openImagePreviewMock.mockReset();
    window.ImageViewerAndroid = undefined;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('auto-opens newest added item when built-in mode, enabled, and app visible', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(true),
    };

    const existing = [
      createItem('existing-1', 'content://existing/1'),
      createItem('existing-2', 'content://existing/2'),
    ];

    const added = [
      createItem('added-1', 'content://added/1'),
      createItem('added-2', 'content://added/2'),
      createItem('added-dup', 'content://existing/2'),
    ];

    await act(async () => {
      root.render(
        <Harness
          galleryItems={existing}
          openMethod="built-in-viewer"
          autoOpenLatestWhenVisible
        />,
      );
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: added, timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).toHaveBeenCalledWith({
      filePath: 'content://existing/2',
      openMethod: 'built-in-viewer',
      allUris: ['content://added/1', 'content://added/2', 'content://existing/2', 'content://existing/1'],
    });
  });

  it('does not auto-open when app is not visible', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(false),
    };

    await act(async () => {
      root.render(
        <Harness
          galleryItems={[createItem('existing-1', 'content://existing/1')]}
          openMethod="built-in-viewer"
          autoOpenLatestWhenVisible
        />,
      );
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: [createItem('added-1', 'content://added/1')], timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).not.toHaveBeenCalled();
  });

  it('does not auto-open in external-app mode', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(true),
    };

    await act(async () => {
      root.render(
        <Harness
          galleryItems={[createItem('existing-1', 'content://existing/1')]}
          openMethod="external-app"
          autoOpenLatestWhenVisible
        />,
      );
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: [createItem('added-1', 'content://added/1')], timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).not.toHaveBeenCalled();
  });

  it('removes listener on unmount and does not open after cleanup', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(true),
    };

    await act(async () => {
      root.render(
        <Harness
          galleryItems={[]}
          openMethod="built-in-viewer"
          autoOpenLatestWhenVisible
        />,
      );
    });

    act(() => {
      root.unmount();
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: [createItem('added-1', 'content://added/1')], timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).not.toHaveBeenCalled();
  });

  it('uses latest rendered gallery items for same-tick rapid events', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(true),
    };

    const firstBatch = [
      createItem('batch1-1', 'content://batch1/1'),
      createItem('batch1-2', 'content://batch1/2'),
    ];
    const secondBatch = [
      createItem('batch2-1', 'content://batch2/1'),
      createItem('batch2-2', 'content://batch2/2'),
    ];

    act(() => {
      flushSync(() => {
        root.render(
          <Harness
            galleryItems={[]}
            openMethod="built-in-viewer"
            autoOpenLatestWhenVisible
          />,
        );
      });
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: firstBatch, timestamp: Date.now() },
        }),
      );

      flushSync(() => {
        root.render(
          <Harness
            galleryItems={firstBatch}
            openMethod="built-in-viewer"
            autoOpenLatestWhenVisible
          />,
        );
      });

      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: secondBatch, timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).toHaveBeenCalledTimes(2);
    expect(openImagePreviewMock).toHaveBeenNthCalledWith(2, {
      filePath: 'content://batch2/2',
      openMethod: 'built-in-viewer',
      allUris: [
        'content://batch2/1',
        'content://batch2/2',
        'content://batch1/1',
        'content://batch1/2',
      ],
    });
  });

  it('accumulates earlier added batch for same-tick events without rerender', async () => {
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      isAppVisible: vi.fn().mockReturnValue(true),
    };

    const firstBatch = [
      createItem('rapid1-1', 'content://rapid1/1'),
      createItem('rapid1-2', 'content://rapid1/2'),
    ];
    const secondBatch = [
      createItem('rapid2-1', 'content://rapid2/1'),
      createItem('rapid2-2', 'content://rapid2/2'),
    ];

    act(() => {
      flushSync(() => {
        root.render(
          <Harness
            galleryItems={[]}
            openMethod="built-in-viewer"
            autoOpenLatestWhenVisible
          />,
        );
      });
    });

    act(() => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: firstBatch, timestamp: Date.now() },
        }),
      );

      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: { items: secondBatch, timestamp: Date.now() },
        }),
      );
    });

    expect(openImagePreviewMock).toHaveBeenCalledTimes(2);
    expect(openImagePreviewMock).toHaveBeenNthCalledWith(2, {
      filePath: 'content://rapid2/2',
      openMethod: 'built-in-viewer',
      allUris: [
        'content://rapid2/1',
        'content://rapid2/2',
        'content://rapid1/1',
        'content://rapid1/2',
      ],
    });
  });
});
