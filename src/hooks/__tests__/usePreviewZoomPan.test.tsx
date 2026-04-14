/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { usePreviewZoomPan } from '../usePreviewZoomPan';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { getCurrentWindowMock, onResizedMock } = vi.hoisted(() => ({
  getCurrentWindowMock: vi.fn(),
  onResizedMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: getCurrentWindowMock,
}));

function Harness({ imagePath }: { imagePath: string | null }) {
  const zoomPan = usePreviewZoomPan(imagePath);

  return (
    <div>
      <div ref={zoomPan.containerRef} data-testid="container" onWheel={zoomPan.handleWheel}>
        <img data-testid="image" alt="preview" />
      </div>
      <button data-testid="reset" onClick={zoomPan.resetZoom}>reset</button>
      <span data-testid="scale">{zoomPan.scale}</span>
    </div>
  );
}

describe('usePreviewZoomPan', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    onResizedMock.mockResolvedValue(vi.fn());
    getCurrentWindowMock.mockReturnValue({
      onResized: onResizedMock,
    });
  });

  it('zooms in on wheel and resets when image path changes', async () => {
    await act(async () => {
      getRoot().render(<Harness imagePath="/photos/a.jpg" />);
      await flush();
    });

    const containerEl = getContainer().querySelector('[data-testid="container"]') as HTMLDivElement;
    const imageEl = getContainer().querySelector('[data-testid="image"]') as HTMLImageElement;

    vi.spyOn(containerEl, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      width: 500,
      height: 500,
      right: 500,
      bottom: 500,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });

    vi.spyOn(imageEl, 'getBoundingClientRect').mockReturnValue({
      left: 50,
      top: 50,
      width: 400,
      height: 400,
      right: 450,
      bottom: 450,
      x: 50,
      y: 50,
      toJSON: () => ({}),
    });

    await act(async () => {
      containerEl.dispatchEvent(new WheelEvent('wheel', { bubbles: true, deltaY: -100, clientX: 200, clientY: 200 }));
      await flush();
    });

    expect(Number(getContainer().querySelector('[data-testid="scale"]')?.textContent ?? '1')).toBeGreaterThan(1);

    await act(async () => {
      getRoot().render(<Harness imagePath="/photos/b.jpg" />);
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="scale"]')?.textContent).toBe('1');
  });
});
