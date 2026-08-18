/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { parsePortInput, usePortCheck } from '../usePortCheck';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

describe('parsePortInput', () => {
  it('returns empty for whitespace-only input', () => {
    expect(parsePortInput('   ', 1, 65535)).toEqual({
      valid: false,
      reason: 'empty',
    });
  });

  it('returns empty for empty string', () => {
    expect(parsePortInput('', 1, 65535)).toEqual({
      valid: false,
      reason: 'empty',
    });
  });

  it('returns invalid_number for non-numeric input', () => {
    expect(parsePortInput('abc', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for port 0', () => {
    expect(parsePortInput('0', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for negative port', () => {
    expect(parsePortInput('-1', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for port above 65535', () => {
    expect(parsePortInput('65536', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns out_of_range when below min', () => {
    expect(parsePortInput('80', 1025, 65535)).toEqual({
      valid: false,
      reason: 'out_of_range',
    });
  });

  it('returns out_of_range when above max', () => {
    expect(parsePortInput('100', 1, 80)).toEqual({
      valid: false,
      reason: 'out_of_range',
    });
  });

  it('returns valid for port 21', () => {
    expect(parsePortInput('21', 1, 65535)).toEqual({
      valid: true,
      port: 21,
    });
  });

  it('returns valid at min boundary', () => {
    expect(parsePortInput('1025', 1025, 65535)).toEqual({
      valid: true,
      port: 1025,
    });
  });

  it('returns valid at max boundary', () => {
    expect(parsePortInput('65535', 1, 65535)).toEqual({
      valid: true,
      port: 65535,
    });
  });

  it('returns valid for port 1 (lower absolute bound)', () => {
    expect(parsePortInput('1', 1, 65535)).toEqual({
      valid: true,
      port: 1,
    });
  });

  it('trims whitespace before parsing', () => {
    expect(parsePortInput('  2121  ', 1, 65535)).toEqual({
      valid: true,
      port: 2121,
    });
  });
});

describe('usePortCheck', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('is not checking initially', () => {
    invokeMock.mockResolvedValue(true);
    const { result } = renderHook(() => usePortCheck());
    expect(result.current.isChecking).toBe(false);
  });

  it('invokes check_port_available with the given port and resolves availability', async () => {
    let resolveInvoke!: (value: boolean) => void;
    invokeMock.mockImplementation(() => new Promise<boolean>((resolve) => {
      resolveInvoke = resolve;
    }));

    const { result } = renderHook(() => usePortCheck());
    let promise!: Promise<{ available: boolean }>;
    act(() => {
      promise = result.current.checkPort(2121);
    });

    expect(invokeMock).toHaveBeenCalledWith('check_port_available', { port: 2121 });
    // While the invoke is pending, isChecking must be true.
    expect(result.current.isChecking).toBe(true);

    let outcome!: { available: boolean };
    await act(async () => {
      resolveInvoke(true);
      outcome = await promise;
    });

    expect(outcome).toEqual({ available: true });
    expect(result.current.isChecking).toBe(false);
  });

  it('returns { available: false } when invoke rejects', async () => {
    invokeMock.mockRejectedValue(new Error('boom'));

    const { result } = renderHook(() => usePortCheck());
    let outcome!: { available: boolean };
    await act(async () => {
      outcome = await result.current.checkPort(2121);
    });

    expect(outcome).toEqual({ available: false });
    expect(result.current.isChecking).toBe(false);
  });

  it('does not start any check on mount', () => {
    invokeMock.mockResolvedValue(true);
    renderHook(() => usePortCheck());
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('resets isChecking even when invoke rejects mid-flight', async () => {
    let rejectInvoke!: (reason: unknown) => void;
    invokeMock.mockImplementation(() => new Promise((_, reject) => {
      rejectInvoke = reject;
    }));

    const { result } = renderHook(() => usePortCheck());
    let pending!: Promise<{ available: boolean }>;
    act(() => {
      pending = result.current.checkPort(8080);
    });
    expect(result.current.isChecking).toBe(true);

    await act(async () => {
      rejectInvoke(new Error('unavailable'));
      await pending;
    });

    expect(result.current.isChecking).toBe(false);
  });

  it('is safe to unmount while a check is in flight', async () => {
    let resolveInvoke!: (value: boolean) => void;
    invokeMock.mockImplementation(() => new Promise((resolve) => {
      resolveInvoke = resolve;
    }));

    const { result, unmount } = renderHook(() => usePortCheck());
    let pending!: Promise<{ available: boolean }>;
    act(() => {
      pending = result.current.checkPort(2121);
    });

    unmount();

    // Completing the invoke after unmount must not throw (state setter on
    // unmounted component is a no-op in React 18) and the promise resolves.
    let outcome: { available: boolean } | undefined;
    await act(async () => {
      resolveInvoke(false);
      outcome = await pending;
    });
    expect(outcome).toEqual({ available: false });
  });

  it('reports isChecking for each sequential check', async () => {
    let resolveInvoke!: (value: boolean) => void;
    invokeMock.mockImplementation(() => new Promise<boolean>((resolve) => {
      resolveInvoke = resolve;
    }));

    const { result } = renderHook(() => usePortCheck());

    let first!: Promise<{ available: boolean }>;
    act(() => {
      first = result.current.checkPort(1111);
    });
    expect(result.current.isChecking).toBe(true);

    let firstOutcome!: { available: boolean };
    await act(async () => {
      resolveInvoke(true);
      firstOutcome = await first;
    });
    expect(firstOutcome).toEqual({ available: true });
    expect(result.current.isChecking).toBe(false);

    let second!: Promise<{ available: boolean }>;
    act(() => {
      second = result.current.checkPort(2222);
    });
    expect(result.current.isChecking).toBe(true);

    let secondOutcome!: { available: boolean };
    await act(async () => {
      resolveInvoke(false);
      secondOutcome = await second;
    });
    expect(secondOutcome).toEqual({ available: false });
    expect(result.current.isChecking).toBe(false);
  });
});
