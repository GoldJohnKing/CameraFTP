/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useCallback } from 'react';
import { Shield } from 'lucide-react';
import { usePermissionStore } from '../stores/permissionStore';
import { PermissionList } from './PermissionList';
import { Dialog } from './ui';

interface PermissionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onAllGranted: () => void;
}

export function PermissionDialog({ isOpen, onClose, onAllGranted }: PermissionDialogProps) {
  // Use permission store
  const checkPermissions = usePermissionStore((state) => state.checkPermissions);
  const startPolling = usePermissionStore((state) => state.startPolling);
  const stopPolling = usePermissionStore((state) => state.stopPolling);
  const allGranted = usePermissionStore((state) => state.allGranted);

  useEffect(() => {
    if (!isOpen) return;
    checkPermissions();
    startPolling();
    return () => stopPolling();
  }, [isOpen, checkPermissions, startPolling, stopPolling]);

  // Handle continue button
  const handleContinue = useCallback(() => {
    if (allGranted) {
      onAllGranted();
      onClose();
    }
  }, [allGranted, onAllGranted, onClose]);

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="权限检查"
      subtitle="为确保FTP服务正常运行，请授予以下权限"
      icon={
        <div className="bg-blue-500 rounded-xl w-10 h-10 flex items-center justify-center">
          <Shield className="w-5 h-5 text-white" />
        </div>
      }
      contentClassName="p-6"
      footer={
        <div className="flex gap-3 w-full">
          <button
            onClick={onClose}
            className="flex-1 px-4 py-3 bg-gray-100 text-gray-700 rounded-xl hover:bg-gray-200"
          >
            取消
          </button>
          <button
            onClick={handleContinue}
            disabled={!allGranted}
            className={`flex-1 px-4 py-3 rounded-xl font-medium ${
              allGranted
                ? 'bg-blue-500 text-white hover:bg-blue-600'
                : 'bg-gray-200 text-gray-400 cursor-not-allowed'
            }`}
          >
            {allGranted ? '开始服务' : '请授予权限'}
          </button>
        </div>
      }
    >
      <PermissionList />
    </Dialog>
  );
}
