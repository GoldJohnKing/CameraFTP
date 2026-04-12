/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePreviewExif } from '../usePreviewExif';
import { flush } from '../../test-utils/flush';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

function Harness({ imagePath }: { imagePath: string | null }) {
  const exifInfo = usePreviewExif(imagePath);

  return (
    <div>
      <span data-testid="iso">{exifInfo?.iso ?? ''}</span>
    </div>
  );
}

describe('usePreviewExif', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    invokeMock.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('loads exif for image path and clears on failure', async () => {
    invokeMock.mockResolvedValueOnce({ iso: 640 });
    invokeMock.mockRejectedValueOnce(new Error('no exif'));

    await act(async () => {
      root.render(<Harness imagePath="/photos/a.jpg" />);
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('get_image_exif', { filePath: '/photos/a.jpg' });
    expect(container.querySelector('[data-testid="iso"]')?.textContent).toBe('640');

    await act(async () => {
      root.render(<Harness imagePath="/photos/b.jpg" />);
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('get_image_exif', { filePath: '/photos/b.jpg' });
    expect(container.querySelector('[data-testid="iso"]')?.textContent).toBe('');
  });

  it('clears stale exif immediately when switching between non-null images', async () => {
    let resolveSecond: ((value: { iso: number } | null) => void) | null = null;
    const secondExifPromise = new Promise<{ iso: number } | null>(resolve => {
      resolveSecond = resolve;
    });

    invokeMock.mockImplementation((command: string, args?: { filePath?: string }) => {
      if (command !== 'get_image_exif') {
        return Promise.resolve(null);
      }

      if (args?.filePath === '/photos/a.jpg') {
        return Promise.resolve({ iso: 320 });
      }

      if (args?.filePath === '/photos/b.jpg') {
        return secondExifPromise;
      }

      return Promise.resolve(null);
    });

    await act(async () => {
      root.render(<Harness imagePath="/photos/a.jpg" />);
      await flush();
    });

    expect(container.querySelector('[data-testid="iso"]')?.textContent).toBe('320');

    await act(async () => {
      root.render(<Harness imagePath="/photos/b.jpg" />);
      await flush();
    });

    expect(container.querySelector('[data-testid="iso"]')?.textContent).toBe('');

    await act(async () => {
      resolveSecond?.({ iso: 800 });
      await flush();
    });

    expect(container.querySelector('[data-testid="iso"]')?.textContent).toBe('800');
  });
});
