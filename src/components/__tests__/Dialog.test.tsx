/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';
import { Dialog } from '../ui/Dialog';

describe('Dialog', () => {
  it('lets its content scroll within a maxHeight flex column (min-h-0)', () => {
    // Regression: when a Dialog has maxHeight, its content flex child must be
    // able to shrink so overflow-y-auto engages and a scrollbar appears.
    // Without min-h-0 the default min-height:auto keeps the child at its full
    // content height, so long lists (e.g. the date-jump picker) overflow the
    // card and the bottom rows get cut off.
    render(
      <Dialog
        isOpen
        onClose={() => {}}
        title="T"
        maxHeight="max-h-[70vh]"
        contentClassName="p-0"
        data-testid="d"
      >
        <button type="button">row 1</button>
        <button type="button">row 2</button>
      </Dialog>,
    );
    const content = document.querySelector('[data-testid="d"] [data-testid="dialog-content"]');
    expect(content).toBeTruthy();
    expect(content!.className).toMatch(/overflow-y-auto/);
    expect(content!.className).toMatch(/min-h-0/);
  });
});
