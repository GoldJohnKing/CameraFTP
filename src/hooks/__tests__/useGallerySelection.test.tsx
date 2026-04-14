/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGallerySelection } from '../useGallerySelection';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

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
  touchCount?: number;
  isScrollingOnStart?: boolean;
};

function createTouchStartEvent(touchCount = 1, clientX = 0, clientY = 0): React.TouchEvent {
  return {
    touches: Array.from({ length: touchCount }, () => ({ clientX, clientY })),
    preventDefault: () => {},
  } as unknown as React.TouchEvent;
}

function GallerySelectionHarness({
  activeTab = 'gallery',
  onDeleteApplied,
  getUriForId,
  touchCount = 1,
  isScrollingOnStart = false,
}: HarnessProps) {
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
        onClick={() => handleTouchStart('content://1', createTouchStartEvent(touchCount), isScrollingOnStart)}
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

describe('useGallerySelection', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    vi.useFakeTimers();
    toastErrorMock.mockReset();

    window.GalleryAndroid = {
      deleteImages: vi.fn().mockResolvedValue(JSON.stringify({ deleted: [], notFound: [], failed: [] })),
      shareImages: vi.fn().mockResolvedValue(true),
      registerBackPressCallback: vi.fn().mockReturnValue(true),
      unregisterBackPressCallback: vi.fn().mockReturnValue(true),
    } as unknown as typeof window.GalleryAndroid;
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('enters selection mode on long press and exits on android back press callback', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('yes');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');
    expect(window.GalleryAndroid?.registerBackPressCallback).toHaveBeenCalled();

    await act(async () => {
      (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed?.();
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(window.GalleryAndroid?.unregisterBackPressCallback).toHaveBeenCalled();
  });

  it('does not enter selection mode for multi-touch long press', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness touchCount={2} />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
  });

  it('does not enter selection mode when touch starts while scrolling', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness isScrollingOnStart />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
  });

  it('deletes immediately and keeps remaining failed selection after partial delete', async () => {
    const onDeleteApplied = vi.fn();
    (window.GalleryAndroid?.deleteImages as ReturnType<typeof vi.fn>).mockResolvedValue(
      JSON.stringify({
        deleted: ['content://media/content://1'],
        notFound: [],
        failed: ['content://media/content://2'],
      }),
    );

    await act(async () => {
      getRoot().render(<GallerySelectionHarness onDeleteApplied={onDeleteApplied} />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="select-toggle-2"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('2');

    await act(async () => {
      getContainer().querySelector('[data-testid="delete"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
      await flush();
    });

    expect(window.GalleryAndroid?.deleteImages).toHaveBeenCalledWith(
      JSON.stringify(['content://media/content://1', 'content://media/content://2']),
    );
    expect((window.GalleryAndroid as Record<string, unknown> | undefined)?.removeThumbnails).toBeUndefined();
    expect(onDeleteApplied).toHaveBeenCalledTimes(1);
    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('yes');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('1');
    expect(toastErrorMock).toHaveBeenCalledWith('部分删除失败：1 张图片未删除。');
  });

  it('closes menu on outside click and after share', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      getContainer().querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');

    await act(async () => {
      getContainer().querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      getContainer().querySelector('[data-testid="share"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(window.GalleryAndroid?.shareImages).toHaveBeenCalled();
    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });

  it('resets menu state when canceling selection', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="cancel-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });

  it('clears transient selection ui state when last selected item is toggled off', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      getContainer().querySelector('[data-testid="select-toggle"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="selection-mode"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="selected-count"]')?.textContent).toBe('0');
    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="deleting-count"]')?.textContent).toBe('0');
  });

  it('closes menu and calls delete bridge immediately when delete is tapped', async () => {
    await act(async () => {
      getRoot().render(<GallerySelectionHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="start-selection"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(400);
      getContainer().querySelector('[data-testid="toggle-menu"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('yes');

    await act(async () => {
      getContainer().querySelector('[data-testid="delete"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(window.GalleryAndroid?.deleteImages).toHaveBeenCalledTimes(1);
    expect(getContainer().querySelector('[data-testid="show-menu"]')?.textContent).toBe('no');
  });
});
