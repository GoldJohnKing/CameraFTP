/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useServerStore } from '../serverStore';
import { usePermissionStore } from '../permissionStore';

const {
  invokeMock,
  checkAndroidPermissionsMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  checkAndroidPermissionsMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../services/permission-bridge', () => ({
  permissionBridge: {
    checkAll: checkAndroidPermissionsMock,
  },
}));

describe('serverStore characterization', () => {
  beforeEach(() => {
    useServerStore.setState({
      isRunning: false,
      serverInfo: null,
      stats: {
        isRunning: false,
        connectedClients: 0,
        filesReceived: 0,
        bytesReceived: 0,
        lastFile: null,
      },
      isLoading: false,
      error: null,
      showPermissionDialog: false,
    });
    usePermissionStore.setState({
      permissions: {
        storage: false,
        notification: false,
        batteryOptimization: false,
      },
      allGranted: false,
      hasCompletedFirstPermissionCheck: false,
    });

    invokeMock.mockReset();
    checkAndroidPermissionsMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'start_server') {
        return {
          isRunning: true,
          ip: '127.0.0.1',
          port: 2221,
          url: 'ftp://127.0.0.1:2221',
          username: 'anonymous',
          passwordInfo: '(任意密码)',
        };
      }
      if (command === 'stop_server') {
        return null;
      }
      return null;
    });

    checkAndroidPermissionsMock.mockResolvedValue(null);
  });

  it('starts server when permissions are available', async () => {
    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('start_server');
    expect(useServerStore.getState().isRunning).toBe(true);
    expect(useServerStore.getState().serverInfo?.url).toBe('ftp://127.0.0.1:2221');
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 0,
      filesReceived: 0,
      bytesReceived: 0,
      lastFile: null,
    });
  });

  it('preserves existing counters when startServer succeeds while already running', async () => {
    useServerStore.setState((state) => ({
      ...state,
      isRunning: true,
      serverInfo: {
        isRunning: true,
        ip: '127.0.0.1',
        port: 2221,
        url: 'ftp://127.0.0.1:2221',
        username: 'anonymous',
        passwordInfo: '(任意密码)',
      },
      stats: {
        isRunning: true,
        connectedClients: 4,
        filesReceived: 11,
        bytesReceived: 8192,
        lastFile: '/keep.jpg',
      },
    }));

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 4,
      filesReceived: 11,
      bytesReceived: 8192,
      lastFile: '/keep.jpg',
    });
  });

  it('shows permission dialog when startServer prerequisites fail', async () => {
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: false,
      notification: true,
      batteryOptimization: true,
    });

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(false);
    expect(useServerStore.getState().showPermissionDialog).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith('start_server');
  });

  it('syncs checked permissions into the permission store on startServer', async () => {
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(usePermissionStore.getState().permissions).toEqual({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    expect(usePermissionStore.getState().allGranted).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('start_server');
  });

  it('stops server and resets runtime state', async () => {
    useServerStore.setState((state) => ({
      ...state,
      isRunning: true,
      serverInfo: {
        isRunning: true,
        ip: '127.0.0.1',
        port: 2221,
        url: 'ftp://127.0.0.1:2221',
        username: 'anonymous',
        passwordInfo: '(任意密码)',
      },
    }));

    await useServerStore.getState().stopServer();

    expect(invokeMock).toHaveBeenCalledWith('stop_server');
    expect(useServerStore.getState().isRunning).toBe(false);
    expect(useServerStore.getState().serverInfo).toBeNull();
  });

  it('同步防重窗口：checkAll in-flight 期间并发 startServer 恰好触发一次 start_server', async () => {
    // isLoading 要到 checkAll 之后才置位，唯一防线是模块级同步 flag。
    // checkAll 用手动控制的 pending Promise 模拟慢 IPC，窗口未关闭期间
    // 第二次 startServer 必须同步早退（返回 false、不产生额外 invoke）。
    type Permissions = { storage: boolean; notification: boolean; batteryOptimization: boolean };
    let resolveCheckAll!: (value: Permissions | null) => void;
    checkAndroidPermissionsMock.mockImplementation(
      () => new Promise<Permissions | null>((resolve) => { resolveCheckAll = resolve; }),
    );

    const first = useServerStore.getState().startServer();
    const second = useServerStore.getState().startServer();

    expect(await second).toBe(false);

    resolveCheckAll({ storage: true, notification: true, batteryOptimization: true });
    expect(await first).toBe(true);

    const startCalls = invokeMock.mock.calls.filter(([command]) => command === 'start_server');
    expect(startCalls).toHaveLength(1);
  });

  it('同步防重窗口：startServer in-flight 期间 continueAfterPermissionsGranted 早退且不翻转弹窗状态', async () => {
    // 场景：首次 startServer 因权限缺失弹出权限对话框（返回 false、flag 复位）；
    // 用户重开触发第二次 startServer（checkAll 挂起）时又点了对话框的"继续"——
    // continueAfterPermissionsGranted 必须同步早退：不重复调用 start_server，
    // 也不把 showPermissionDialog 错误翻转为 false。
    type Permissions = { storage: boolean; notification: boolean; batteryOptimization: boolean };
    let resolveCheckAll!: (value: Permissions | null) => void;
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: false,
      notification: true,
      batteryOptimization: true,
    });

    // 第一步：权限缺失 → 弹出对话框，未启动。
    expect(await useServerStore.getState().startServer()).toBe(false);
    expect(useServerStore.getState().showPermissionDialog).toBe(true);

    // 第二步：新的 startServer 挂在 checkAll 上（窗口开启）。
    checkAndroidPermissionsMock.mockImplementation(
      () => new Promise<Permissions | null>((resolve) => { resolveCheckAll = resolve; }),
    );
    const second = useServerStore.getState().startServer();

    // 窗口未关闭期间点击"继续"——必须早退，弹窗保持打开。
    await useServerStore.getState().continueAfterPermissionsGranted();
    expect(useServerStore.getState().showPermissionDialog).toBe(true);

    resolveCheckAll({ storage: true, notification: true, batteryOptimization: true });
    expect(await second).toBe(true);

    const startCalls = invokeMock.mock.calls.filter(([command]) => command === 'start_server');
    expect(startCalls).toHaveLength(1);
  });

  it('setServerRunning 清除残留的权限对话框', async () => {
    // 服务启动成功后对话框不应挂在运行中的服务上（另一入口启动时清残留）。
    useServerStore.setState({ showPermissionDialog: true });

    useServerStore.getState().setServerRunning({
      isRunning: true,
      ip: '127.0.0.1',
      port: 2221,
      url: 'ftp://127.0.0.1:2221',
      username: 'anonymous',
      passwordInfo: '(任意密码)',
    });

    expect(useServerStore.getState().isRunning).toBe(true);
    expect(useServerStore.getState().showPermissionDialog).toBe(false);
  });

  it('startInFlight 异常复位：start_server 失败后紧接重试可正常执行', async () => {
    // 钉住 finally 复位可重试语义：第一次 startServer 抛出后，同步 flag
    // 必须已复位 —— 第二次调用（mock 改 resolve）正常走完流程且产生一次
    // 新的 start_server invoke。
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'start_server') {
        throw new Error('start_server failed');
      }
      return null;
    });

    await expect(useServerStore.getState().startServer()).rejects.toThrow('start_server failed');

    // 后端恢复：重试应成功。
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'start_server') {
        return {
          isRunning: true,
          ip: '127.0.0.1',
          port: 2221,
          url: 'ftp://127.0.0.1:2221',
          username: 'anonymous',
          passwordInfo: '(任意密码)',
        };
      }
      return null;
    });

    const startCallsBefore = invokeMock.mock.calls.filter(
      ([command]) => command === 'start_server',
    ).length;

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(useServerStore.getState().isRunning).toBe(true);
    const startCallsAfter = invokeMock.mock.calls.filter(
      ([command]) => command === 'start_server',
    ).length;
    expect(startCallsAfter).toBe(startCallsBefore + 1);
  });

});
