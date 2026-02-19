/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useCallback, useRef } from 'react';
import { usePermissionStore } from '../stores/permissionStore';
import { PermissionList } from './PermissionList';

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

  // Track if we started polling to clean up correctly
  const wasPollingRef = useRef(false);

  // Check permissions when dialog opens and start polling
  useEffect(() => {
    if (isOpen) {
      checkPermissions();
      startPolling();
      wasPollingRef.current = true;
    }
    
    // Stop polling on cleanup only if we started it
    return () => {
      if (wasPollingRef.current) {
        stopPolling();
        wasPollingRef.current = false;
      }
    };
  }, [isOpen, checkPermissions, startPolling, stopPolling]);

  // Handle continue button
  const handleContinue = useCallback(() => {
    if (allGranted) {
      onAllGranted();
      onClose();
    }
  }, [allGranted, onAllGranted, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-2xl max-w-md w-full shadow-xl">
        {/* Header */}
        <div className="p-6 border-b">
          <h2 className="text-xl font-semibold text-gray-900">权限检查</h2>
          <p className="text-sm text-gray-500 mt-1">
            为确保FTP服务正常运行，请授予以下权限
          </p>
        </div>

        {/* Permission List */}
        <div className="p-6">
          <PermissionList />
        </div>

        {/* Footer */}
        <div className="p-6 border-t flex gap-3">
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
      </div>
    </div>
  );
}
