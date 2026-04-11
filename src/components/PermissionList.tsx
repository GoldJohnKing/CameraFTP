/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo } from 'react';
import { Check, Folder, Bell, Zap } from 'lucide-react';
import { usePermissionStore } from '../stores/permissionStore';

interface PermissionListProps {
  /** Use compact style (dots only) vs detailed style (icons with descriptions) */
  variant?: 'compact' | 'detailed';
}

// ===== Permission Item Components =====

interface PermissionItemCompactProps {
  label: string;
  granted: boolean;
  onRequest: () => void;
}

const PermissionItemCompact = memo(function PermissionItemCompact({
  label,
  granted,
  onRequest,
}: PermissionItemCompactProps) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-2">
        <div className={`w-3 h-3 rounded-full ${granted ? 'bg-green-500' : 'bg-red-500'}`} />
        <span className="text-sm text-gray-700">{label}</span>
      </div>
      {granted ? (
        <span className="text-xs text-green-600">已授权</span>
      ) : (
        <button
          onClick={onRequest}
          className="text-xs text-blue-500 hover:text-blue-600"
        >
          授权
        </button>
      )}
    </div>
  );
});

interface PermissionItemDetailedProps {
  title: string;
  description: string;
  granted: boolean;
  isLoading: boolean;
  grantedIcon: React.ReactNode;
  deniedIcon: React.ReactNode;
  onRequest: () => void;
}

const PermissionItemDetailed = memo(function PermissionItemDetailed({
  title,
  description,
  granted,
  isLoading,
  grantedIcon,
  deniedIcon,
  onRequest,
}: PermissionItemDetailedProps) {
  return (
    <div className="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
      <div className="flex items-center gap-3">
        <div className={`w-10 h-10 rounded-full flex items-center justify-center ${
          granted ? 'bg-green-100' : 'bg-gray-200'
        }`}>
          {granted ? (
            <div className="text-green-600">{grantedIcon}</div>
          ) : (
            <div className="text-gray-400">{deniedIcon}</div>
          )}
        </div>
        <div>
          <p className="font-medium text-gray-900">{title}</p>
          <p className="text-xs text-gray-500">{description}</p>
        </div>
      </div>
      {!granted && (
        <button
          onClick={onRequest}
          disabled={isLoading}
          className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 disabled:opacity-50"
        >
          授予
        </button>
      )}
    </div>
  );
});

// ===== Permission Icons =====

const CheckIcon = <Check className="w-5 h-5" />;
const FolderIcon = <Folder className="w-5 h-5" />;
const BellIcon = <Bell className="w-5 h-5" />;
const ZapIcon = <Zap className="w-5 h-5" />;

// ===== Permission Configuration =====

interface PermissionConfig {
  key: 'storage' | 'notification' | 'batteryOptimization';
  label: string;
  description: string;
  grantedIcon: React.ReactNode;
  deniedIcon: React.ReactNode;
}

const PERMISSION_CONFIGS: PermissionConfig[] = [
  {
    key: 'storage',
    label: '文件访问权限',
    description: '用于保存相机上传的照片',
    grantedIcon: CheckIcon,
    deniedIcon: FolderIcon,
  },
  {
    key: 'notification',
    label: '通知权限',
    description: '用于显示服务状态和快捷操作',
    grantedIcon: CheckIcon,
    deniedIcon: BellIcon,
  },
  {
    key: 'batteryOptimization',
    label: '电池优化白名单',
    description: '防止后台运行时被系统清理',
    grantedIcon: CheckIcon,
    deniedIcon: ZapIcon,
  },
];

// ===== Main Component =====

export function PermissionList({
  variant = 'detailed',
}: PermissionListProps) {
  const permissions = usePermissionStore((state) => state.permissions);
  const isLoading = usePermissionStore((state) => state.isLoading);
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const requestNotificationPermission = usePermissionStore((state) => state.requestNotificationPermission);
  const requestBatteryOptimization = usePermissionStore((state) => state.requestBatteryOptimization);

  const requestHandlers = {
    storage: requestStoragePermission,
    notification: requestNotificationPermission,
    batteryOptimization: requestBatteryOptimization,
  };

  const visiblePermissions = PERMISSION_CONFIGS;

  if (variant === 'compact') {
    return (
      <div className="space-y-3">
        {visiblePermissions.map(config => (
          <PermissionItemCompact
            key={config.key}
            label={config.label}
            granted={permissions[config.key]}
            onRequest={requestHandlers[config.key]}
          />
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {visiblePermissions.map(config => (
        <PermissionItemDetailed
          key={config.key}
          title={config.label}
          description={config.description}
          granted={permissions[config.key]}
          isLoading={isLoading}
          grantedIcon={config.grantedIcon}
          deniedIcon={config.deniedIcon}
          onRequest={requestHandlers[config.key]}
        />
      ))}
    </div>
  );
}
