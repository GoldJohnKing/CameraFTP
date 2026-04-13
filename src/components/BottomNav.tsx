/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo } from 'react';
import { Home, Settings, Images } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import { usePlatform } from '../hooks/usePlatform';

export const BottomNav = memo(function BottomNav() {
  const { activeTab, setActiveTab } = useConfigStore();
  const { isAndroid } = usePlatform();

  const navItems = [
    { id: 'home' as const, icon: Home, label: '主页' },
    ...(isAndroid ? [{ id: 'gallery' as const, icon: Images, label: '图库' }] : []),
    { id: 'config' as const, icon: Settings, label: '配置' },
  ];

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200">
      <div className="max-w-md mx-auto flex">
        {navItems.map(({ id, icon: Icon, label }) => (
          <button
            key={id}
            onClick={() => setActiveTab(id)}
            className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
              activeTab === id
                ? 'text-blue-600'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            <Icon className="w-6 h-6" />
            <span className="text-xs mt-1 font-medium">{label}</span>
          </button>
        ))}
      </div>
    </nav>
  );
});
