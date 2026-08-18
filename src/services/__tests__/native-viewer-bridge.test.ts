/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { registerNativeViewerBridges } from '../native-viewer-bridge';
import { PERMISSION_RECHECK_REQUESTED_EVENT } from '../../utils/gallery-refresh';

const { checkPermissionsMock } = vi.hoisted(() => ({
  checkPermissionsMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../hooks/useAiEditProgress', () => ({
  applyAndEnqueueAiEdit: vi.fn(),
  getCurrentAiEditProgress: vi.fn(() => null),
  cancelAiEdit: vi.fn(),
}));

vi.mock('../../hooks/useColorGradingProgress', () => ({
  getCurrentColorGradingProgress: vi.fn(() => null),
  cancelColorGrading: vi.fn(),
}));

vi.mock('../../hooks/useColorGradingPresets', () => ({
  getCachedColorGradingPresets: vi.fn(() => []),
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: {
    getState: () => ({ draft: null, updateDraft: vi.fn() }),
  },
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: {
    getState: () => ({ checkPermissions: checkPermissionsMock }),
  },
}));

vi.mock('../image-open', () => ({
  requestExifForPositions: vi.fn(),
}));

describe('registerNativeViewerBridges permission re-check listener', () => {
  it('re-checks permissions when the native re-check event fires', () => {
    checkPermissionsMock.mockClear();
    const cleanup = registerNativeViewerBridges();

    window.dispatchEvent(new CustomEvent(PERMISSION_RECHECK_REQUESTED_EVENT));

    expect(checkPermissionsMock).toHaveBeenCalledTimes(1);

    cleanup();

    window.dispatchEvent(new CustomEvent(PERMISSION_RECHECK_REQUESTED_EVENT));

    expect(checkPermissionsMock).toHaveBeenCalledTimes(1);
  });
});
