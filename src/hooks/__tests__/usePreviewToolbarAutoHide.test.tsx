/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { usePreviewToolbarAutoHide } from '../usePreviewToolbarAutoHide';
import { setupReactRoot } from '../../test-utils/react-root';

function Harness() {
  const toolbar = usePreviewToolbarAutoHide();

  return (
    <div>
      <span data-testid="visible">{toolbar.showToolbar ? 'yes' : 'no'}</span>
      <button data-testid="move" onClick={toolbar.showToolbarOnPointerMove}>move</button>
      <button data-testid="enter" onMouseEnter={toolbar.handleToolbarMouseEnter} onClick={toolbar.handleToolbarMouseEnter}>enter</button>
      <button data-testid="leave" onMouseLeave={toolbar.handleToolbarMouseLeave} onClick={toolbar.handleToolbarMouseLeave}>leave</button>
    </div>
  );
}

const TOOLBAR_HIDE_DELAY_MS = 3000;

describe('usePreviewToolbarAutoHide', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('hides toolbar after idle timeout and keeps visible while hovered', async () => {
    await act(async () => {
      getRoot().render(<Harness />);
    });

    expect(getContainer().querySelector('[data-testid="visible"]')?.textContent).toBe('yes');

    await act(async () => {
      vi.advanceTimersByTime(TOOLBAR_HIDE_DELAY_MS);
    });
    expect(getContainer().querySelector('[data-testid="visible"]')?.textContent).toBe('no');

    await act(async () => {
      getContainer().querySelector('[data-testid="move"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      getContainer().querySelector('[data-testid="enter"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(TOOLBAR_HIDE_DELAY_MS);
    });

    expect(getContainer().querySelector('[data-testid="visible"]')?.textContent).toBe('yes');
  });

  it('restarts idle timer on repeated pointer movement while visible', async () => {
    await act(async () => {
      getRoot().render(<Harness />);
    });

    await act(async () => {
      vi.advanceTimersByTime(2500);
      getContainer().querySelector('[data-testid="move"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      vi.advanceTimersByTime(700);
    });

    expect(getContainer().querySelector('[data-testid="visible"]')?.textContent).toBe('yes');

    await act(async () => {
      vi.advanceTimersByTime(2300);
    });

    expect(getContainer().querySelector('[data-testid="visible"]')?.textContent).toBe('no');
  });
});
