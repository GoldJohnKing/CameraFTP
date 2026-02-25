import { useEffect, useCallback } from 'react';
import { usePermissionStore } from '../stores/permissionStore';

interface PermissionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onAllGranted: () => void;
}

export function PermissionDialog({ isOpen, onClose, onAllGranted }: PermissionDialogProps) {
  // Use permission store
  const permissions = usePermissionStore((state) => state.permissions);
  const isLoading = usePermissionStore((state) => state.isLoading);
  const checkPermissions = usePermissionStore((state) => state.checkPermissions);
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const requestNotificationPermission = usePermissionStore((state) => state.requestNotificationPermission);
  const requestBatteryOptimization = usePermissionStore((state) => state.requestBatteryOptimization);
  const startPolling = usePermissionStore((state) => state.startPolling);
  const stopPolling = usePermissionStore((state) => state.stopPolling);
  const allGranted = usePermissionStore((state) => state.allGranted);

  // Check permissions when dialog opens and start polling
  useEffect(() => {
    if (isOpen) {
      checkPermissions();
      startPolling();
    }
    
    // Stop polling when dialog closes
    return () => {
      if (!isOpen) {
        stopPolling();
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

  // Permission request handlers (polling auto-detects changes)
  const handleRequestStorage = useCallback(() => requestStoragePermission(), [requestStoragePermission]);
  const handleRequestNotification = useCallback(() => requestNotificationPermission(), [requestNotificationPermission]);
  const handleRequestBattery = useCallback(() => requestBatteryOptimization(), [requestBatteryOptimization]);

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
        <div className="p-6 space-y-4">
          {/* Storage Permission */}
          <div className="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
            <div className="flex items-center gap-3">
              <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
                permissions.storage ? 'bg-green-100' : 'bg-gray-200'
              }`}>
                {permissions.storage ? (
                  <svg className="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                )}
              </div>
              <div>
                <p className="font-medium text-gray-900">文件访问权限</p>
                <p className="text-xs text-gray-500">用于保存相机上传的照片</p>
              </div>
            </div>
            {!permissions.storage && (
              <button
                onClick={handleRequestStorage}
                disabled={isLoading}
                className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
              >
                授予
              </button>
            )}
          </div>

          {/* Notification Permission */}
          <div className="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
            <div className="flex items-center gap-3">
              <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
                permissions.notification ? 'bg-green-100' : 'bg-gray-200'
              }`}>
                {permissions.notification ? (
                  <svg className="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
                  </svg>
                )}
              </div>
              <div>
                <p className="font-medium text-gray-900">通知权限</p>
                <p className="text-xs text-gray-500">用于显示服务状态和快捷操作</p>
              </div>
            </div>
            {!permissions.notification && (
              <button
                onClick={handleRequestNotification}
                disabled={isLoading}
                className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
              >
                授予
              </button>
            )}
          </div>

          {/* Battery Optimization */}
          <div className="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
            <div className="flex items-center gap-3">
              <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
                permissions.batteryOptimization ? 'bg-green-100' : 'bg-gray-200'
              }`}>
                {permissions.batteryOptimization ? (
                  <svg className="w-5 h-5 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  <svg className="w-5 h-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                )}
              </div>
              <div>
                <p className="font-medium text-gray-900">电池优化白名单</p>
                <p className="text-xs text-gray-500">防止后台运行时被系统清理</p>
              </div>
            </div>
            {!permissions.batteryOptimization && (
              <button
                onClick={handleRequestBattery}
                disabled={isLoading}
                className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
              >
                授予
              </button>
            )}
          </div>
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
