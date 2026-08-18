/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { Power } from 'lucide-react';
import { useState, useCallback, memo } from 'react';
import { toast } from 'sonner';
import { useServerStore } from '../stores/serverStore';
import { usePermissionStore } from '../stores/permissionStore';
import { LoadingButton, ErrorMessage } from './ui';
import { formatError } from '../utils/error';


export const ServerCard = memo(function ServerCard() {
  const [isStarting, setIsStarting] = useState(false);
  const isRunning = useServerStore((state) => state.isRunning);
  const isLoading = useServerStore((state) => state.isLoading);
  const error = useServerStore((state) => state.error);
  const startServer = useServerStore((state) => state.startServer);
  const stopServer = useServerStore((state) => state.stopServer);
  const needsPermission = usePermissionStore((state) => state.needsPermission);
  const checkPrerequisites = usePermissionStore((state) => state.checkPrerequisites);
  const ensureStorageReady = usePermissionStore((state) => state.ensureStorageReady);
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const startPermissionPolling = usePermissionStore((state) => state.startPolling);

  const handleStartServer = useCallback(async () => {
    if (isStarting) return;

    setIsStarting(true);

    try {
      const check = await checkPrerequisites();

      if (!check.canStart) {
        if (needsPermission) {
          // Route through the Kotlin PermissionBridge (handles system settings
          // and partial-access flow) and poll until storage is granted.
          // On desktop both bridge calls are no-ops.
          requestStoragePermission();
          if (usePermissionStore.getState().pollingIntervalId === null) {
            startPermissionPolling('storage');
          }
        } else {
          const result = await ensureStorageReady();
          if (result.success) {
            await startServer();
          }
        }
      } else {
        await startServer();
      }
    } catch (err) {
      toast.error('启动服务器失败：' + formatError(err));
    } finally {
      setIsStarting(false);
    }
  }, [isStarting, checkPrerequisites, needsPermission, requestStoragePermission, startPermissionPolling, ensureStorageReady, startServer]);

  const handleToggle = async () => {
    try {
      if (isRunning) {
        await stopServer();
      } else {
        await handleStartServer();
      }
    } catch {
      // 错误已在 store 中处理
    }
  };

  return (
    <div>
      <LoadingButton
        isLoading={isLoading || isStarting}
        loadingText={isRunning ? '停止中...' : '启动中...'}
        onClick={handleToggle}
        variant={isRunning ? 'danger' : 'primary'}
        icon={<Power className="w-5 h-5" />}
      >
        {isRunning ? '停止服务器' : '启动服务器'}
      </LoadingButton>

      <ErrorMessage message={error} />
    </div>
  );
});
