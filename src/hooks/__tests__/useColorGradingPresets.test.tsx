/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';
import type { ColorGradingPreset } from '../../types';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const PRESETS: ColorGradingPreset[] = [
  {
    id: 'fujifilm-provia',
    displayName: 'Fuji Provia',
    logSpace: 'V-Log',
    cubeFilename: 'Fujifilm_PROVIA_VLog.cube',
  },
  {
    id: 'fujifilm-velvia',
    displayName: 'Fuji Velvia',
    logSpace: 'V-Log',
    cubeFilename: 'Fujifilm_Velvia_VLog.cube',
  },
];

// The hook caches at module scope; re-import a fresh module instance per test
// so the singleton state does not leak between cases.
let useColorGradingPresetsRef: typeof import('../useColorGradingPresets').useColorGradingPresets;
let getCachedColorGradingPresetsRef: typeof import('../useColorGradingPresets').getCachedColorGradingPresets;

function PresetsHarness() {
  const presets = useColorGradingPresetsRef();
  return (
    <ul data-testid="presets">
      {presets.map((p) => (
        <li key={p.id}>{p.displayName}</li>
      ))}
    </ul>
  );
}

/** Two independent consumers mounted at the same time. */
function DualConsumerHarness() {
  return (
    <>
      <PresetsHarness />
      <PresetsHarness />
    </>
  );
}

/** Records the preset count on every render so post-unmount updates are observable. */
const renderLog: number[] = [];

function RenderCountingHarness() {
  const presets = useColorGradingPresetsRef();
  renderLog.push(presets.length);
  return <div data-testid="preset-count">{presets.length}</div>;
}

describe('useColorGradingPresets', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(async () => {
    vi.resetModules();
    ({
      useColorGradingPresets: useColorGradingPresetsRef,
      getCachedColorGradingPresets: getCachedColorGradingPresetsRef,
    } = await import('../useColorGradingPresets'));

    renderLog.length = 0;
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(PRESETS);
  });

  function presetNames(): string[] {
    return Array.from(getContainer().querySelectorAll('[data-testid="presets"] li')).map(
      (li) => li.textContent ?? '',
    );
  }

  it('fetches presets via invoke on first mount and exposes them', async () => {
    await act(async () => {
      getRoot().render(<PresetsHarness />);
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('get_color_grading_presets');
    expect(presetNames()).toEqual(['Fuji Provia', 'Fuji Velvia']);
    expect(getCachedColorGradingPresetsRef()).toEqual(PRESETS);
  });

  it('reuses the same in-flight promise when a second consumer mounts before resolution', async () => {
    let resolveFetch!: (value: ColorGradingPreset[]) => void;
    invokeMock.mockImplementationOnce(
      () =>
        new Promise<ColorGradingPreset[]>((resolve) => {
          resolveFetch = resolve;
        }),
    );

    await act(async () => {
      getRoot().render(<DualConsumerHarness />);
      await Promise.resolve();
    });

    // Both consumers mounted while the fetch is pending, but only one invoke.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(getContainer().querySelectorAll('[data-testid="presets"]')).toHaveLength(2);

    await act(async () => {
      resolveFetch(PRESETS);
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(presetNames()).toEqual(['Fuji Provia', 'Fuji Velvia', 'Fuji Provia', 'Fuji Velvia']);
  });

  it('keeps presets empty on a failed fetch and lets the next mount retry', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    invokeMock.mockRejectedValueOnce(new Error('presets unavailable'));

    await act(async () => {
      getRoot().render(<PresetsHarness />);
      await flush();
    });

    // The failure is swallowed: the hook has no error state, presets stay empty.
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(presetNames()).toEqual([]);
    expect(consoleError).toHaveBeenCalled();

    // The failed promise must not stay cached — a real remount fetches again.
    // （先卸载再挂载：对同组件的重复 render 是更新而非重挂载，
    //   [] 依赖的 effect 不会重跑，无法触发重试。）
    await act(async () => {
      getRoot().render(null);
      await flush();
    });
    await act(async () => {
      getRoot().render(<PresetsHarness />);
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(presetNames()).toEqual(['Fuji Provia', 'Fuji Velvia']);

    consoleError.mockRestore();
  });

  it('does not update state when the fetch resolves after unmount', async () => {
    let resolveFetch!: (value: ColorGradingPreset[]) => void;
    invokeMock.mockImplementationOnce(
      () =>
        new Promise<ColorGradingPreset[]>((resolve) => {
          resolveFetch = resolve;
        }),
    );

    await act(async () => {
      getRoot().render(<RenderCountingHarness />);
      await Promise.resolve();
    });

    expect(renderLog).toEqual([0]);
    const renderCountAfterMount = renderLog.length;

    act(() => {
      getRoot().unmount();
    });

    await act(async () => {
      resolveFetch(PRESETS);
      await flush();
    });

    // The module-level cache DID update (resolution is independent of the
    // consumer), but the unmounted component rendered nothing after resolve —
    // that distinction is exactly what the cancelled flag guards.
    expect(getCachedColorGradingPresetsRef()).toEqual(PRESETS);
    expect(renderLog.length).toBe(renderCountAfterMount);
    expect(renderLog).toEqual([0]);
  });
});
