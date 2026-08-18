/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback } from 'react';
import { Home, Settings, Images } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';
import { usePlatform } from '../hooks/usePlatform';

export const BottomNav = memo(function BottomNav() {
  const activeTab = useConfigStore((state) => state.activeTab);
  const setActiveTab = useConfigStore((state) => state.setActiveTab);
  const { isAndroid } = usePlatform();

  const handleGalleryClick = useCallback(async () => {
    if (isAndroid) {
      setActiveTab('gallery');
    } else {
      try {
        await invoke('open_save_directory');
      } catch {
        // Directory may not exist or drive unavailable — silently ignore
      }
    }
  }, [isAndroid, setActiveTab]);

  const navItems = [
    { id: 'home' as const, icon: Home, label: '主页', onClick: () => setActiveTab('home') },
    { id: 'gallery' as const, icon: Images, label: '图库', onClick: handleGalleryClick },
    { id: 'config' as const, icon: Settings, label: '配置', onClick: () => setActiveTab('config') },
  ];

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200" style={{ paddingBottom: 'env(safe-area-inset-bottom)' }}>
      <div className="max-w-md mx-auto flex">
        {navItems.map(({ id, icon: Icon, label, onClick }) => (
          <button
            key={id}
            onClick={onClick}
            className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
              (!isAndroid && id === 'gallery')
                ? 'text-gray-500 hover:text-gray-700'
                : activeTab === id
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
