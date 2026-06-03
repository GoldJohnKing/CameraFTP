/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../../App';
import { setupReactRoot } from '../../test-utils/react-root';
import { flush } from '../../test-utils/flush';
import type { AppConfig } from '../../types';

const { enqueueColorGradingMock } = vi.hoisted(() => ({
  enqueueColorGradingMock: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ label: 'main' })),
}));

vi.mock('../../hooks/useColorGradingProgress', () => ({
  enqueueColorGrading: enqueueColorGradingMock,
  getCurrentColorGradingProgress: vi.fn(() => ({
    isProcessing: false, isDone: false, current: 0, total: 0,
    currentFileName: '', failedCount: 0, failedFiles: [],
  })),
  cancelColorGrading: vi.fn(),
}));

vi.mock('../../hooks/useAiEditProgress', () => ({
  enqueueAiEdit: vi.fn(),
  getCurrentAiEditProgress: vi.fn(() => ({
    isProcessing: false, isDone: false, current: 0, total: 0,
    currentFileName: '', failedCount: 0, failedFiles: [],
  })),
  cancelAiEdit: vi.fn(),
}));

vi.mock('../../hooks/useQuitFlow', () => ({
  useQuitFlow: vi.fn(() => ({
    showQuitDialog: false, closeQuitDialog: vi.fn(), handleQuitConfirm: vi.fn(),
  })),
}));

vi.mock('../../services/server-events', () => ({
  initializeServerEvents: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock('../../bootstrap/useAppBootstrap', () => ({
  useAppBootstrap: vi.fn(),
}));

vi.mock('../../stores/serverStore', () => ({
  useServerStore: Object.assign(
    () => ({
      showPermissionDialog: false,
      closePermissionDialog: vi.fn(),
      continueAfterPermissionsGranted: vi.fn(),
    }),
    { getState: () => ({ showPermissionDialog: false }), setState: vi.fn() },
  ),
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: (selector: (s: { initialize: () => void }) => unknown) =>
    selector({ initialize: vi.fn() }),
}));

vi.mock('../ServerCard', () => ({ ServerCard: () => <div>ServerCard</div> }));
vi.mock('../StatsCard', () => ({ StatsCard: () => <div>StatsCard</div> }));
vi.mock('../InfoCard', () => ({ InfoCard: () => <div>InfoCard</div> }));
vi.mock('../LatestPhotoCard', () => ({ LatestPhotoCard: () => <div>LatestPhotoCard</div> }));
vi.mock('../ConfigCard', () => ({ ConfigCard: () => <div>ConfigCard</div> }));
vi.mock('../GalleryCard', () => ({ GalleryCard: () => <div>GalleryCard</div> }));
vi.mock('../BottomNav', () => ({ BottomNav: () => <div>BottomNav</div> }));
vi.mock('../PermissionDialog', () => ({ PermissionDialog: () => null }));
vi.mock('../PreviewWindow', () => ({ PreviewWindow: () => <div>PreviewWindow</div> }));
vi.mock('../TaskProgressPanel', () => ({ TaskProgressPanel: () => null }));

import { useConfigStore } from '../../stores/configStore';

const BASE_DRAFT: AppConfig = {
  savePath: '/tmp/test',
  port: 2121,
  autoSelectPort: false,
  advancedConnection: { enabled: false, auth: { anonymous: true, username: '', passwordHash: '' } },
  previewConfig: null,
  androidImageViewer: null,
  autoColorGrading: { enabled: true, presetId: 'fujifilm-provia', meteringMode: 'highlight-safe', evOffset: 0 },
  colorGradingLastUsed: null,
  aiEdit: {
    autoEdit: false, prompt: '', manualPrompt: '', manualModel: '',
    provider: { type: 'seed-edit', model: 'doubao-seedream-4-0-250828', apiKey: '' },
  },
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function setPartialDraft(overrides: Partial<AppConfig>) {
  useConfigStore.setState((s: any) => ({
    draft: { ...s.draft, ...overrides },
  }));
}

type TriggerFn = (f: string, l: string, m: string, e: number, s: boolean) => Promise<void>;

describe('Color grading bridge functions', () => {
  const { getRoot } = setupReactRoot();
  const w = window as unknown as Record<string, unknown>;
  let trigger: TriggerFn;

  beforeEach(async () => {
    enqueueColorGradingMock.mockClear();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    useConfigStore.setState({
      config: null,
      draft: { ...BASE_DRAFT },
      isLoading: false,
      error: null,
      activeTab: 'home' as const,
      platform: 'android',
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);

    await act(async () => {
      getRoot().render(<App />);
      await flush();
    });

    trigger = w.__tauriTriggerColorGrading as TriggerFn;
  });

  // --- __tauriGetAutoColorGradingEnabled ---

  describe('__tauriGetAutoColorGradingEnabled', () => {
    it('returns "true" when autoColorGrading is enabled', () => {
      setPartialDraft({ autoColorGrading: { enabled: true, presetId: 'x', meteringMode: 'matrix', evOffset: 0 } });
      const result = (w.__tauriGetAutoColorGradingEnabled as () => string)();
      expect(result).toBe('true');
    });

    it('returns "false" when autoColorGrading is disabled', () => {
      setPartialDraft({ autoColorGrading: { enabled: false, presetId: 'x', meteringMode: 'matrix', evOffset: 0 } });
      const result = (w.__tauriGetAutoColorGradingEnabled as () => string)();
      expect(result).toBe('false');
    });
  });

  // --- __tauriGetColorGradingLastUsed ---

  describe('__tauriGetColorGradingLastUsed', () => {
    it('returns null JSON when no last-used config', () => {
      setPartialDraft({ colorGradingLastUsed: null });
      const result = (w.__tauriGetColorGradingLastUsed as () => string)();
      expect(JSON.parse(result)).toBeNull();
    });

    it('returns last-used config as JSON', () => {
      setPartialDraft({
        colorGradingLastUsed: {
          presetId: 'fujifilm-velvia',
          meteringMode: 'matrix', evOffset: 1.5,
        },
      });

      const result = (w.__tauriGetColorGradingLastUsed as () => string)();
      expect(JSON.parse(result)).toEqual({
        presetId: 'fujifilm-velvia',
        meteringMode: 'matrix', evOffset: 1.5,
      });
    });
  });

  // --- __tauriTriggerColorGrading ---

  describe('__tauriTriggerColorGrading', () => {
    it('passes meteringMode and evOffset to enqueueColorGrading', async () => {
      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-provia', 'highlight-safe', 0, false);
      });

      expect(enqueueColorGradingMock).toHaveBeenCalledWith(
        ['/photo.nef'], 'fujifilm-provia', 'highlight-safe', 0,
      );
    });

    it('passes negative evOffset', async () => {
      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-velvia', 'matrix', -2.5, false);
      });

      expect(enqueueColorGradingMock).toHaveBeenCalledWith(
        ['/photo.nef'], 'fujifilm-velvia', 'matrix', -2.5,
      );
    });

    it('always saves colorGradingLastUsed on confirm', async () => {
      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-provia', 'highlight-safe', 0, false);
      });

      expect(useConfigStore.getState().draft?.colorGradingLastUsed).toEqual({
        presetId: 'fujifilm-provia',
        meteringMode: 'highlight-safe',
        evOffset: 0,
      });
    });

    it('does not overwrite autoColorGrading when syncToAuto is false', async () => {
      setPartialDraft({
        autoColorGrading: {
          enabled: true, presetId: 'kodak-vision-2383',
          meteringMode: 'average', evOffset: 3.0,
        },
      });

      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-provia', 'highlight-safe', 0, false);
      });

      expect(useConfigStore.getState().draft?.autoColorGrading).toEqual({
        enabled: true, presetId: 'kodak-vision-2383',
        meteringMode: 'average', evOffset: 3.0,
      });
    });

    it('syncs to autoColorGrading when syncToAuto is true', async () => {
      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-velvia', 'matrix', 2.5, true);
      });

      expect(useConfigStore.getState().draft?.autoColorGrading).toEqual({
        enabled: true, presetId: 'fujifilm-velvia',
        meteringMode: 'matrix', evOffset: 2.5,
      });
    });

    it('preserves autoColorGrading.enabled when syncing', async () => {
      setPartialDraft({
        autoColorGrading: {
          enabled: false, presetId: 'old',
          meteringMode: 'spot', evOffset: -1.0,
        },
      });

      await act(async () => {
        await trigger('/photo.nef', 'fujifilm-provia', 'highlight-safe', 0, true);
      });

      expect(useConfigStore.getState().draft?.autoColorGrading?.enabled).toBe(false);
    });
  });
});
