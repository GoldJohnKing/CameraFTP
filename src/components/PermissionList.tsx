import { usePermissionStore } from '../stores/permissionStore';

interface PermissionListProps {
  showStorage?: boolean;
  showNotification?: boolean;
  showBattery?: boolean;
  /** Use compact style (dots only) vs detailed style (icons with descriptions) */
  variant?: 'compact' | 'detailed';
}

export function PermissionList({ 
  showStorage = true, 
  showNotification = true, 
  showBattery = true,
  variant = 'detailed',
}: PermissionListProps) {
  const permissions = usePermissionStore((state) => state.permissions);
  const isLoading = usePermissionStore((state) => state.isLoading);
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const requestNotificationPermission = usePermissionStore((state) => state.requestNotificationPermission);
  const requestBatteryOptimization = usePermissionStore((state) => state.requestBatteryOptimization);

  if (variant === 'compact') {
    return (
      <div className="space-y-3">
        {showStorage && (
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className={`w-3 h-3 rounded-full ${permissions.storage ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm text-gray-700">文件访问权限</span>
            </div>
            <button
              onClick={requestStoragePermission}
              className={`text-xs ${permissions.storage ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
            >
              {permissions.storage ? '已授权' : '授权'}
            </button>
          </div>
        )}

        {showNotification && (
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className={`w-3 h-3 rounded-full ${permissions.notification ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm text-gray-700">通知权限</span>
            </div>
            <button
              onClick={requestNotificationPermission}
              className={`text-xs ${permissions.notification ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
            >
              {permissions.notification ? '已授权' : '授权'}
            </button>
          </div>
        )}

        {showBattery && (
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className={`w-3 h-3 rounded-full ${permissions.batteryOptimization ? 'bg-green-500' : 'bg-red-500'}`} />
              <span className="text-sm text-gray-700">电池优化白名单</span>
            </div>
            <button
              onClick={requestBatteryOptimization}
              className={`text-xs ${permissions.batteryOptimization ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
            >
              {permissions.batteryOptimization ? '已授权' : '授权'}
            </button>
          </div>
        )}
      </div>
    );
  }

  // Detailed variant (default)
  return (
    <div className="space-y-4">
      {showStorage && (
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
              onClick={requestStoragePermission}
              disabled={isLoading}
              className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
            >
              授予
            </button>
          )}
        </div>
      )}

      {showNotification && (
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
              onClick={requestNotificationPermission}
              disabled={isLoading}
              className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
            >
              授予
            </button>
          )}
        </div>
      )}

      {showBattery && (
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
              onClick={requestBatteryOptimization}
              disabled={isLoading}
              className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
            >
              授予
            </button>
          )}
        </div>
      )}
    </div>
  );
}
