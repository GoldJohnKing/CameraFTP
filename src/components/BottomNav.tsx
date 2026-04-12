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

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200">
      <div className="max-w-md mx-auto flex">
        <button
          onClick={() => setActiveTab('home')}
          className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
            activeTab === 'home'
              ? 'text-blue-600'
              : 'text-gray-500 hover:text-gray-700'
          }`}
        >
          <Home className="w-6 h-6" />
          <span className="text-xs mt-1 font-medium">主页</span>
        </button>
        
        {isAndroid && (
          <button
            onClick={() => setActiveTab('gallery')}
            className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
              activeTab === 'gallery'
                ? 'text-blue-600'
                : 'text-gray-500 hover:text-gray-700'
            }`}
          >
            <Images className="w-6 h-6" />
            <span className="text-xs mt-1 font-medium">图库</span>
          </button>
        )}
        
        <button
          onClick={() => setActiveTab('config')}
          className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
            activeTab === 'config'
              ? 'text-blue-600'
              : 'text-gray-500 hover:text-gray-700'
          }`}
        >
          <Settings className="w-6 h-6" />
          <span className="text-xs mt-1 font-medium">配置</span>
        </button>
      </div>
    </nav>
  );
});
