/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGallerySelection } from '../useGallerySelection';

const { toastErrorMock } = vi.hoisted(() => ({
  toastErrorMock: vi.fn(),
}));

vi.mock('sonner', () => ({
  toast: {
    error: toastErrorMock,
  },
}));

type HarnessProps = {
  activeTab?: string;
  onDeleteApplied?: (pathsToAnimate: Set<string>) => void | Promise<void>;
  getUriForId?: (mediaId: string) => string | undefined;
};

function GallerySelectionHarness({ activeTab = 'gallery', onDeleteApplied, getUriForId }: HarnessProps) {
  const {
    isSelectionMode,
    selectedIds,
    showMenu,
    deletingIds,
    menuRef,
    handleTouchStart,
    handleTouchEnd,
    handleSelectionClick,
    handleDelete,
    handleShare,
    handleCancelSelection,
    toggleMenu,
  } = useGallerySelection({
    activeTab,
    onDeleteApplied: onDeleteApplied ?? (() => {}),
    getUriForId: getUriForId ?? ((mediaId) => `content://media/${mediaId}`),
  });

  return (
    <div>
      <div ref={menuRef} data-testid="menu-root">
        menu
      </div>
      <span data-testid="selection-mode">{isSelectionMode ? 'yes' : 'no'}</span>
      <span data-testid="selected-count">{selectedIds.size}</span>
      <span data-testid="show-menu">{showMenu ? 'yes' : 'no'}</span>
      <span data-testid="deleting-count">{deletingIds.size}</span>
      <button
        data-testid="start-selection"
        onClick={() => handleTouchStart('content://1', { preventDefault: () => {} } as React.TouchEvent, false)}
      >
        start-selection
      </button>
      <button data-testid="touch-end" onClick={handleTouchEnd}>touch-end</button>
      <button data-testid="select-toggle" onClick={() => handleSelectionClick('content://1')}>
        select-toggle
      </button>
      <button data-testid="select-toggle-2" onClick={() => handleSelectionClick('content://2')}>
        select-toggle-2
      </button>
      <button data-testid="toggle-menu" onClick={toggleMenu}>toggle-menu</button>
      <button data-testid="delete" onClick={() => void handleDelete()}>delete</button>
      <button data-testid="share" onClick={() => void handleShare()}>share</button>
      <button data-testid="cancel-selection" onClick={handleCancelSelection}>cancel-selection</button>
    </div>
  );
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('useGallerySelection', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    vi.useFakeTimers();
    toastErrorMock.mockReset();

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    window.GalleryAndroid = {
      deleteImages: vi.fn().mockResolvedValue(JSON.stringify({ deleted: [], notFound: [], failed: [] })),
      removeThumbnails: vi.fn().mockResolvedValue(true),
      shareImages: vi.fn().mockResolvedValue(true),
      registerBackPressCallback: vi.fn().mockReturnValue(true),
      unregisterBackPressCallback: vi.fn().mockReturnValue(true),
    } as unknown as typeof window.GalleryAndroid;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('enters selection mode on long press and exits on android back press callback', async () => {
    await act(async () => {
      root.render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');

    expect(container.querySelector('[data-testid="selection-mode"]')?.textContent).toBe('yes');
    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');
    expect(window.GalleryAndroid?.registerBackPressCallback).toHaveBeenCalled();

    await act(async () => {
      (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed?.();
      await flush();
    });

    expect(container.querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(window.GalleryAndroid?.unregisterBackPressCallback).toHaveBeenCalled();
  });

  it('deletes immediately and keeps remaining failed selection after partial delete', async () => {
    const onDeleteApplied = vi.fn();
    (window.GalleryAndroid?.deleteImages as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({ deleted: ['content://1'], notFound: [], failed: ['content://2'] }),
    );

    await act(async () => {
      root.render(<GallerySelectionHarness onDeleteApplied={onDeleteApplied} />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="select-toggle-2"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('2');

    await act(async () => {
      container.querySelector('[data-testid="delete"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await flush();
    });

    expect(window.GalleryAndroid?.deleteImages).toHaveBeenCalledWith(JSON.stringify(['content://1', 'content://2']));
    expect(window.GalleryAndroid?.removeThumbnails).not.toHaveBeenCalled();
    expect(onDeleteApplied).toHaveBeenCalledTimes(1);
    expect(container.querySelector('[data-testid="selection-mode"]')?.textContent).toBe('yes');
    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');
    expect(toastErrorMock).toHaveBeenCalledWith('部分删除失败：1 张图片未删除。');
  });

  it('closes menu on outside click and after share', async () => {
    await act(async () => {
      root.render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      container.querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');

    await act(async () => {
      container.querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      container.querySelector('[data-testid="share"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(window.GalleryAndroid?.shareImages).toHaveBeenCalled();
    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });

  it('resets menu state when canceling selection', async () => {
    await act(async () => {
      root.render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="cancel-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });

  it('clears transient selection ui state when last selected item is toggled off', async () => {
    await act(async () => {
      root.render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      container.querySelector('[data-testid="select-toggle"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="deleting-count"]')?.textContent).toBe('0');
  });

  it('closes menu and calls delete bridge immediately when delete is tapped', async () => {
    await act(async () => {
      root.render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      container.querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      container.querySelector('[data-testid="delete"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(window.GalleryAndroid?.deleteImages).toHaveBeenCalledTimes(1);
    expect(container.querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });
});
