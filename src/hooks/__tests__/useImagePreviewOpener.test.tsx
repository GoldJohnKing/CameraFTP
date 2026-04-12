/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useImagePreviewOpener } from '../useImagePreviewOpener';
import { flush } from '../../test-utils/flush';

const { useDraftConfigMock, openImagePreviewMock } = vi.hoisted(() => ({
  useDraftConfigMock: vi.fn(),
  openImagePreviewMock: vi.fn(),
}));

vi.mock('../../stores/configStore', () => ({
  useDraftConfig: useDraftConfigMock,
}));

vi.mock('../../services/image-open', () => ({
  openImagePreview: openImagePreviewMock,
}));

interface HarnessProps {
  filePath: string;
  allUris?: string[];
}

function Harness({ filePath, allUris }: HarnessProps) {
  const openPreview = useImagePreviewOpener();

  return (
    <button
      data-testid="open"
      onClick={() => {
        void openPreview({ filePath, allUris });
      }}
    >
      open
    </button>
  );
}

describe('useImagePreviewOpener', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    useDraftConfigMock.mockReset();
    openImagePreviewMock.mockReset();
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('passes configured Android open method to image-open service', async () => {
    useDraftConfigMock.mockReturnValue({
      androidImageViewer: {
        openMethod: 'built-in-viewer',
        autoOpenLatestWhenVisible: true,
      },
    });

    await act(async () => {
      root.render(<Harness filePath="content://media/1" allUris={['content://media/1', 'content://media/2']} />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="open"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(openImagePreviewMock).toHaveBeenCalledWith({
      filePath: 'content://media/1',
      allUris: ['content://media/1', 'content://media/2'],
      getAllUris: undefined,
      openMethod: 'built-in-viewer',
    });
  });

  it('opens without Android method when draft is unavailable', async () => {
    useDraftConfigMock.mockReturnValue(null);

    await act(async () => {
      root.render(<Harness filePath="/tmp/image.jpg" />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="open"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(openImagePreviewMock).toHaveBeenCalledWith({
      filePath: '/tmp/image.jpg',
      allUris: undefined,
      getAllUris: undefined,
      openMethod: undefined,
    });
  });
});
