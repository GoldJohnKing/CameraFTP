/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { VirtualGalleryGrid } from '../VirtualGalleryGrid';
import type { MediaItemDto } from '../../types';
import { flush } from '../../test-utils/flush';
import { createMockRectObserver } from '../../test-utils/mock-resize-observer';
import { setupReactRoot } from '../../test-utils/react-root';

/** Build a single item with a controllable file path (extension-driven). */
function item(mediaId: string, filePath: string): MediaItemDto {
  return {
    mediaId,
    uri: `content://media/${mediaId}`,
    dateModifiedMs: 0,
    width: 100,
    height: 100,
    mimeType: null,
    displayName: null,
    filePath,
  };
}

const ITEMS: MediaItemDto[] = [
  item('raw-nef', '/DCIM/A.NEF'),
  item('raw-cr3', '/DCIM/B.CR3'),
  item('heif-heic', '/DCIM/C.HEIC'),
  item('heif-hif', '/DCIM/D.HIF'),
  item('jpg-a', '/DCIM/E.JPG'),
  item('png-a', '/DCIM/F.PNG'),
];

describe('VirtualGalleryGrid type label', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    const resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
  });

  async function renderGrid() {
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={ITEMS}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={() => {}}
        />,
      );
      await flush();
    });
  }

  it('shows a RAW label on RAW cells', async () => {
    await renderGrid();

    const rawCell = getContainer().querySelector('[data-media-id="raw-nef"]')!;
    const label = rawCell.querySelector('[data-testid="type-label"]');
    expect(label).toBeTruthy();
    expect(label!.getAttribute('data-type-category')).toBe('raw');
    expect(label!.textContent).toBe('RAW');
  });

  it('shows a HEIF label on HEIF cells (heic and hif)', async () => {
    await renderGrid();

    for (const id of ['heif-heic', 'heif-hif']) {
      const cell = getContainer().querySelector(`[data-media-id="${id}"]`)!;
      const label = cell.querySelector('[data-testid="type-label"]');
      expect(label).toBeTruthy();
      expect(label!.getAttribute('data-type-category')).toBe('heif');
      expect(label!.textContent).toBe('HEIF');
    }
  });

  it('shows no type label on JPEG / 其它 cells', async () => {
    await renderGrid();

    for (const id of ['jpg-a', 'png-a']) {
      const cell = getContainer().querySelector(`[data-media-id="${id}"]`)!;
      expect(cell.querySelector('[data-testid="type-label"]')).toBeNull();
    }
  });

  it('renders the label with the semi-transparent gray rounded-rect styling', async () => {
    await renderGrid();

    const label = getContainer().querySelector('[data-media-id="raw-cr3"] [data-testid="type-label"]')!;
    // Semi-transparent gray fill (bg-gray-500/50) + rounded rect + white text.
    expect(label.className).toMatch(/bg-gray-500\/50/);
    expect(label.className).toMatch(/rounded/);
    expect(label.className).toMatch(/text-white/);
    // Positioned top-right.
    expect(label.className).toMatch(/top-1 right-1/);
  });
});
