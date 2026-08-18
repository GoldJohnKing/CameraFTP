/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

// Use vi.hoisted so mock factories (which are hoisted) can reference these.
const {
  startServerMock,
  stopServerMock,
  checkPrerequisitesMock,
  requestStoragePermissionMock,
  startPollingMock,
  ensureStorageReadyMock,
  serverState,
  permissionState,
} = vi.hoisted(() => {
  const serverState = {
    isRunning: false,
    isLoading: false,
    error: null as string | null,
    startServer: vi.fn(),
    stopServer: vi.fn(),
  };
  // Aliases for readability in assertions
  const startServerMock = serverState.startServer;
  const stopServerMock = serverState.stopServer;
  const checkPrerequisitesMock = vi.fn();

  const permissionState = {
    needsPermission: false,
    pollingIntervalId: null as number | null,
    checkPrerequisites: checkPrerequisitesMock,
    requestStoragePermission: vi.fn(),
    startPolling: vi.fn(),
    ensureStorageReady: vi.fn(),
  };
  const requestStoragePermissionMock = permissionState.requestStoragePermission;
  const startPollingMock = permissionState.startPolling;
  const ensureStorageReadyMock = permissionState.ensureStorageReady;

  return {
    startServerMock,
    stopServerMock,
    checkPrerequisitesMock,
    requestStoragePermissionMock,
    startPollingMock,
    ensureStorageReadyMock,
    serverState,
    permissionState,
  };
});

// Re-define makeStore inside hoisted block so it's available in vi.mock factories
const { makeStore } = vi.hoisted(() => {
  const makeStore = (stateFn: () => Record<string, unknown>) => {
    const useStore = (selector?: (s: Record<string, unknown>) => unknown) =>
      selector ? selector(stateFn()) : stateFn();
    (useStore as unknown as { getState: () => Record<string, unknown> }).getState = stateFn;
    return useStore;
  };
  return { makeStore };
});

vi.mock('../../stores/serverStore', () => ({
  useServerStore: makeStore(() => ({
    isRunning: serverState.isRunning,
    isLoading: serverState.isLoading,
    error: serverState.error,
    startServer: serverState.startServer,
    stopServer: serverState.stopServer,
  })),
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: makeStore(() => ({
    needsPermission: permissionState.needsPermission,
    pollingIntervalId: permissionState.pollingIntervalId,
    checkPrerequisites: permissionState.checkPrerequisites,
    requestStoragePermission: permissionState.requestStoragePermission,
    startPolling: permissionState.startPolling,
    ensureStorageReady: permissionState.ensureStorageReady,
  })),
}));

vi.mock('sonner', () => ({
  toast: { error: vi.fn() },
}));

vi.mock('../ui', () => ({
  LoadingButton: ({
    children,
    onClick,
    isLoading,
  }: {
    children: React.ReactNode;
    onClick: () => void;
    isLoading: boolean;
  }) => <button onClick={onClick} disabled={isLoading}>{children}</button>,
  ErrorMessage: () => null,
}));

vi.mock('../../utils/error', () => ({
  formatError: (err: unknown) => String(err),
}));

import { ServerCard } from '../ServerCard';

describe('ServerCard start/stop flow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    serverState.isRunning = false;
    serverState.isLoading = false;
    serverState.error = null;
    permissionState.needsPermission = false;
    permissionState.pollingIntervalId = null;

    startServerMock.mockResolvedValue(undefined);
    stopServerMock.mockResolvedValue(undefined);
    checkPrerequisitesMock.mockResolvedValue({ canStart: true });
    ensureStorageReadyMock.mockResolvedValue({ success: true });
  });

  it('starts server directly when prerequisites allow', async () => {
    checkPrerequisitesMock.mockResolvedValue({ canStart: true });

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /启动服务器/i }));

    await waitFor(() => {
      expect(checkPrerequisitesMock).toHaveBeenCalledOnce();
      expect(startServerMock).toHaveBeenCalledOnce();
    });
  });

  it('calls ensureStorageReady then startServer when storage check needed', async () => {
    checkPrerequisitesMock.mockResolvedValue({ canStart: false });
    ensureStorageReadyMock.mockResolvedValue({ success: true });

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /启动服务器/i }));

    await waitFor(() => {
      expect(ensureStorageReadyMock).toHaveBeenCalledOnce();
      expect(startServerMock).toHaveBeenCalledOnce();
    });
  });

  it('does not start server when storage ensure fails', async () => {
    checkPrerequisitesMock.mockResolvedValue({ canStart: false });
    ensureStorageReadyMock.mockResolvedValue({ success: false });

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /启动服务器/i }));

    await waitFor(() => {
      expect(ensureStorageReadyMock).toHaveBeenCalledOnce();
    });
    expect(startServerMock).not.toHaveBeenCalled();
  });

  it('requests storage permission and polls without starting server when permission needed', async () => {
    permissionState.needsPermission = true;
    checkPrerequisitesMock.mockResolvedValue({ canStart: false });

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /启动服务器/i }));

    await waitFor(() => {
      expect(requestStoragePermissionMock).toHaveBeenCalledOnce();
      expect(startPollingMock).toHaveBeenCalledWith('storage');
    });
    expect(startServerMock).not.toHaveBeenCalled();
  });

  it('does not restart polling when it is already running', async () => {
    permissionState.needsPermission = true;
    permissionState.pollingIntervalId = 123;
    checkPrerequisitesMock.mockResolvedValue({ canStart: false });

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /启动服务器/i }));

    await waitFor(() => {
      expect(requestStoragePermissionMock).toHaveBeenCalledOnce();
    });
    expect(startPollingMock).not.toHaveBeenCalled();
    expect(startServerMock).not.toHaveBeenCalled();
  });

  it('stops server when running and button clicked', async () => {
    serverState.isRunning = true;

    render(<ServerCard />);
    fireEvent.click(screen.getByRole('button', { name: /停止服务器/i }));

    await waitFor(() => {
      expect(stopServerMock).toHaveBeenCalledOnce();
    });
    expect(startServerMock).not.toHaveBeenCalled();
  });

  it('prevents double-click during startup', async () => {
    let resolveStart!: () => void;
    startServerMock.mockReturnValue(
      new Promise<void>((r) => {
        resolveStart = r;
      }),
    );
    checkPrerequisitesMock.mockResolvedValue({ canStart: true });

    render(<ServerCard />);
    const button = screen.getByRole('button', { name: /启动服务器/i });
    fireEvent.click(button);

    // Wait for the handler to be in-flight (checkPrerequisites called),
    // then attempt a second click — the isStarting guard blocks re-entry.
    await waitFor(() => {
      expect(checkPrerequisitesMock).toHaveBeenCalledOnce();
    });

    fireEvent.click(button);
    expect(startServerMock).toHaveBeenCalledOnce();

    resolveStart();
  });
});
