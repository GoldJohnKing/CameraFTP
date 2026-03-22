/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGalleryGrid } from '../useGalleryGrid';

const { convertFileSrcMock } = vi.hoisted(() => ({
  convertFileSrcMock: vi.fn((value: string) => `asset://${value}`),
}));

const { getThumbnailMock, cleanupThumbnailsNotInListMock, removeThumbnailsMock } = vi.hoisted(() => ({
  getThumbnailMock: vi.fn(async (path: string) => `/thumbs/${path.replace('content://', '')}.jpg`),
  cleanupThumbnailsNotInListMock: vi.fn().mockResolvedValue(0),
  removeThumbnailsMock: vi.fn().mockResolvedValue(true),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: convertFileSrcMock,
}));

type HarnessProps = {
  images: Array<{ path: string; filename: string; sortTime: number }>;
  isLoading?: boolean;
  enteringIds?: Set<string>;
  suppressGridAnimations?: boolean;
};

function GalleryGridHarness({
  images,
  isLoading = false,
  enteringIds = new Set<string>(),
  suppressGridAnimations = false,
}: HarnessProps) {
  const { thumbnails, loadingThumbnails, imageRefCallback, removeThumbnailEntries, cleanupDeletedThumbnails } = useGalleryGrid({
    images,
    isLoading,
    enteringIds,
    suppressGridAnimations,
  });

  return (
    <div>
      <span data-testid="thumbnail-count">{thumbnails.size}</span>
      <span data-testid="loading-count">{loadingThumbnails.size}</span>
      <span data-testid="thumbnail-1">{thumbnails.get('content://1') ?? ''}</span>
      <span data-testid="thumbnail-10">{thumbnails.get('content://10') ?? ''}</span>
      <button
        data-testid="remove-content-1"
        onClick={() => removeThumbnailEntries(new Set(['content://1']))}
      >
        remove-content-1
      </button>
      <button
        data-testid="cleanup-content-1"
        onClick={() => {
          void cleanupDeletedThumbnails(new Set(['content://1']));
        }}
      >
        cleanup-content-1
      </button>
      {images.map((image) => (
        <div
          key={image.path}
          data-testid={`tile-${image.path}`}
          data-path={image.path}
          ref={(el) => imageRefCallback(image.path, el)}
        />
      ))}
    </div>
  );
}

class IntersectionObserverMock {
  static instances: IntersectionObserverMock[] = [];

  callback: IntersectionObserverCallback;
  observed = new Set<Element>();

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback;
    IntersectionObserverMock.instances.push(this);
  }

  observe = (element: Element) => {
    this.observed.add(element);
  };

  unobserve = (element: Element) => {
    this.observed.delete(element);
  };

  disconnect = () => {
    this.observed.clear();
  };

  trigger(entries: IntersectionObserverEntry[]) {
    this.callback(entries, this as unknown as IntersectionObserver);
  }
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('useGalleryGrid', () => {
  let container: HTMLDivElement;
  let root: Root;
  let animationFrameId = 0;
  let scheduledAnimationFrames = new Map<number, FrameRequestCallback>();

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    vi.useFakeTimers();
    convertFileSrcMock.mockClear();
    IntersectionObserverMock.instances = [];

    animationFrameId = 0;
    scheduledAnimationFrames = new Map();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      animationFrameId += 1;
      scheduledAnimationFrames.set(animationFrameId, callback);
      return animationFrameId;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => {
      scheduledAnimationFrames.delete(id);
    });
    vi.stubGlobal('IntersectionObserver', IntersectionObserverMock as unknown as typeof IntersectionObserver);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    Object.defineProperty(window, 'GalleryAndroid', {
      configurable: true,
      writable: true,
      value: {
        getThumbnail: getThumbnailMock,
        cleanupThumbnailsNotInList: cleanupThumbnailsNotInListMock,
        removeThumbnails: removeThumbnailsMock,
      } as unknown as typeof window.GalleryAndroid,
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
    getThumbnailMock.mockClear();
    cleanupThumbnailsNotInListMock.mockClear();
    removeThumbnailsMock.mockClear();
  });

  it('preloads thumbnails for the first rows only', async () => {
    const images = Array.from({ length: 12 }, (_, index) => ({
      path: `content://${index + 1}`,
      filename: `${index + 1}.jpg`,
      sortTime: index + 1,
    }));

    await act(async () => {
      root.render(<GalleryGridHarness images={images} />);
      await flush();
    });

    await act(async () => {
      scheduledAnimationFrames.forEach((callback) => callback(0));
      scheduledAnimationFrames.clear();
      vi.runAllTimers();
      await flush();
    });

    expect(getThumbnailMock).toHaveBeenCalledTimes(9);
    expect(container.querySelector('[data-testid="thumbnail-count"]')?.textContent).toBe('9');
    expect(cleanupThumbnailsNotInListMock).toHaveBeenCalledWith(JSON.stringify(images.map((image) => image.path)));
  });

  it('loads thumbnail for intersecting tile and cleans up removed images', async () => {
    const images = [
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://10', filename: '10.jpg', sortTime: 10 },
      { path: 'content://11', filename: '11.jpg', sortTime: 11 },
      { path: 'content://12', filename: '12.jpg', sortTime: 12 },
      { path: 'content://13', filename: '13.jpg', sortTime: 13 },
      { path: 'content://14', filename: '14.jpg', sortTime: 14 },
      { path: 'content://15', filename: '15.jpg', sortTime: 15 },
      { path: 'content://16', filename: '16.jpg', sortTime: 16 },
      { path: 'content://17', filename: '17.jpg', sortTime: 17 },
      { path: 'content://18', filename: '18.jpg', sortTime: 18 },
    ];

    await act(async () => {
      root.render(<GalleryGridHarness images={images} />);
      await flush();
    });

    await act(async () => {
      scheduledAnimationFrames.forEach((callback) => callback(0));
      scheduledAnimationFrames.clear();
      vi.runAllTimers();
      await flush();
    });

    expect(getThumbnailMock).toHaveBeenCalledTimes(9);

    const observer = IntersectionObserverMock.instances[0];
    const tile = container.querySelector('[data-testid="tile-content://18"]') as Element;

    await act(async () => {
      observer.trigger([
        {
          target: tile,
          isIntersecting: true,
        } as IntersectionObserverEntry,
      ]);
      vi.advanceTimersByTime(60);
      await flush();
    });

    expect(getThumbnailMock).toHaveBeenCalledTimes(10);
    expect(container.querySelector('[data-testid="thumbnail-10"]')?.textContent).toBe('asset:///thumbs/10.jpg');

    await act(async () => {
      root.render(<GalleryGridHarness images={images.filter(image => image.path !== 'content://1')} />);
      await flush();
    });

    expect(container.querySelector('[data-testid="thumbnail-1"]')?.textContent).toBe('');
  });

  it('removes thumbnail entries explicitly for deleted images', async () => {
    const images = [
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://2', filename: '2.jpg', sortTime: 2 },
    ];

    await act(async () => {
      root.render(<GalleryGridHarness images={images} />);
      await flush();
    });

    await act(async () => {
      scheduledAnimationFrames.forEach((callback) => callback(0));
      scheduledAnimationFrames.clear();
      vi.runAllTimers();
      await flush();
    });

    expect(container.querySelector('[data-testid="thumbnail-1"]')?.textContent).toBe('asset:///thumbs/1.jpg');

    await act(async () => {
      container.querySelector('[data-testid="remove-content-1"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="thumbnail-1"]')?.textContent).toBe('');
  });

  it('coordinates deleted thumbnail cache cleanup in grid hook', async () => {
    const images = [
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://2', filename: '2.jpg', sortTime: 2 },
    ];

    await act(async () => {
      root.render(<GalleryGridHarness images={images} />);
      await flush();
    });

    await act(async () => {
      scheduledAnimationFrames.forEach((callback) => callback(0));
      scheduledAnimationFrames.clear();
      vi.runAllTimers();
      await flush();
    });

    expect(container.querySelector('[data-testid="thumbnail-1"]')?.textContent).toBe('asset:///thumbs/1.jpg');

    await act(async () => {
      container.querySelector('[data-testid="cleanup-content-1"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="thumbnail-1"]')?.textContent).toBe('');
    expect(removeThumbnailsMock).toHaveBeenCalledWith(JSON.stringify(['content://1']));
  });

  it('handles synchronous orphan cleanup bridge return values', async () => {
    cleanupThumbnailsNotInListMock.mockReset();
    cleanupThumbnailsNotInListMock.mockReturnValue(0);

    const images = [
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
    ];

    await act(async () => {
      root.render(<GalleryGridHarness images={images} />);
      await flush();
    });

    await act(async () => {
      vi.runAllTimers();
      await flush();
    });

    expect(cleanupThumbnailsNotInListMock).toHaveBeenCalledWith(JSON.stringify(['content://1']));
    expect(container.querySelector('[data-testid="thumbnail-count"]')?.textContent).toBe('0');
    expect(getThumbnailMock).not.toHaveBeenCalled();
  });

  it('clears scheduled preload timers when image list changes before they fire', async () => {
    const initialImages = Array.from({ length: 9 }, (_, index) => ({
      path: `content://${index + 1}`,
      filename: `${index + 1}.jpg`,
      sortTime: index + 1,
    }));

    await act(async () => {
      root.render(<GalleryGridHarness images={initialImages} />);
      await flush();
    });

    await act(async () => {
      root.render(<GalleryGridHarness images={[]} />);
      await flush();
    });

    await act(async () => {
      scheduledAnimationFrames.forEach((callback) => callback(0));
      scheduledAnimationFrames.clear();
      vi.runAllTimers();
      await flush();
    });

    expect(getThumbnailMock).not.toHaveBeenCalled();
  });
});
