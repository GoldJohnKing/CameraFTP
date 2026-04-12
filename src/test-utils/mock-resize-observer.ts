/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * Creates a mock ResizeObserver with a `triggerResize` helper for testing.
 */
export function createMockRectObserver() {
  const callbacks: Map<Element, ResizeObserverCallback> = new Map();

  class MockResizeObserver {
    private _cb: ResizeObserverCallback;
    private _el: Element | null = null;

    constructor(cb: ResizeObserverCallback) {
      this._cb = cb;
    }

    observe(el: Element) {
      this._el = el;
      callbacks.set(el, this._cb);
    }

    unobserve(el: Element) {
      callbacks.delete(el);
    }

    disconnect() {
      if (this._el) callbacks.delete(this._el);
    }
  }

  const triggerResize = (el: Element, height: number) => {
    const cb = callbacks.get(el);
    if (cb) {
      cb(
        [{ contentRect: { height } } as ResizeObserverEntry],
        {} as ResizeObserver,
      );
    }
  };

  return { MockResizeObserver, triggerResize };
}
