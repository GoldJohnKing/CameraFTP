/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Camera, X } from 'lucide-react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { LatestPhotoCard } from './components/LatestPhotoCard';
import { ConfigCard } from './components/ConfigCard';
import { GalleryCard } from './components/GalleryCard';
import { BottomNav } from './components/BottomNav';
import { PermissionDialog } from './components/PermissionDialog';
import { PreviewWindow } from './components/PreviewWindow';
import { useServerStore } from './stores/serverStore';
import { useConfigStore } from './stores/configStore';
import { usePermissionStore } from './stores/permissionStore';

function App() {
  const { initializeListeners, showPermissionDialog, closePermissionDialog, continueAfterPermissionsGranted } = useServerStore();
  const { activeTab, loadConfig, loadPlatform, platform } = useConfigStore();
  const initializePermissions = usePermissionStore((state) => state.initialize);
  const [showQuitDialog, setShowQuitDialog] = useState(false);
  const [isPreviewWindow, setIsPreviewWindow] = useState(false);

  // 检测当前是否是预览窗口
  useEffect(() => {
    const window = getCurrentWindow();
    setIsPreviewWindow(window.label === 'preview');
  }, []);

  // 加载平台信息并设置 html class（用于平台自适应样式）
  useEffect(() => {
    loadPlatform();
  }, [loadPlatform]);

  // Initialize permission store (Android only, safe to call on all platforms)
  useEffect(() => {
    initializePermissions();
  }, [initializePermissions]);

  // 根据平台设置 html 元素的 class
  useEffect(() => {
    if (platform && platform !== 'unknown') {
      document.documentElement.className = `platform-${platform}`;
    }
  }, [platform]);

  // 初始化 store 的监听器
  useEffect(() => {
    let cleanupFn: (() => void) | undefined;
    let isCancelled = false;

    const setup = async () => {
      try {
        const cleanup = await initializeListeners();
        if (!isCancelled) {
          cleanupFn = cleanup;
        } else {
          cleanup();
        }
      } catch (err) {
        console.warn('[App] Listener initialization failed:', err);
      }
    };

    setup();

    return () => {
      isCancelled = true;
      cleanupFn?.();
    };
  }, [initializeListeners]);

  // 监听退出请求自定义事件（由 serverStore 中的 window-close-requested 触发）
  useEffect(() => {
    const handleQuitRequest = () => {
      setShowQuitDialog(true);
    };
    window.addEventListener('app-quit-requested', handleQuitRequest);
    return () => {
      window.removeEventListener('app-quit-requested', handleQuitRequest);
    };
  }, []);

  // 加载配置
  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const handleQuitConfirm = async (quit: boolean) => {
    if (quit) {
      // 通过Rust命令退出程序
      await invoke('quit_application');
    } else {
      // 先关闭弹窗
      setShowQuitDialog(false);
      // 通过Rust命令隐藏窗口
      try {
        await invoke('hide_main_window');
      } catch (err) {
        console.warn('[App] Failed to hide window:', err);
      }
    }
  };

  // 如果是预览窗口，直接渲染预览组件
  if (isPreviewWindow) {
    return <PreviewWindow />;
  }

  return (
    <div className="min-h-screen bg-gray-50 pb-20">
      {/* 退出确认对话框 */}
      {showQuitDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-[100]">
          <div className="bg-white rounded-lg p-6 max-w-sm mx-4 shadow-xl relative">
            <button
              onClick={() => setShowQuitDialog(false)}
              className="absolute top-3 right-3 p-1 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-full transition-colors"
              aria-label="关闭"
            >
              <X className="w-5 h-5" />
            </button>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              确认退出
            </h3>
            <p className="text-gray-600 mb-4">
              您是要退出程序还是最小化到系统托盘？
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => handleQuitConfirm(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
              >
                最小化到托盘
              </button>
              <button
                onClick={() => handleQuitConfirm(true)}
                className="px-4 py-2 text-white bg-red-600 rounded-lg hover:bg-red-700 transition-colors"
              >
                退出程序
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Android 权限检查对话框 */}
      <PermissionDialog
        isOpen={showPermissionDialog}
        onClose={closePermissionDialog}
        onAllGranted={continueAfterPermissionsGranted}
      />

      <div className="max-w-md mx-auto p-4">
        {/* Header - 只在主页显示 */}
        {activeTab === 'home' && (
          <header className="text-center py-6">
            <div className="w-16 h-16 bg-blue-600 rounded-2xl flex items-center justify-center mx-auto mb-3">
              <Camera className="w-8 h-8 text-white" />
            </div>
            <h1 className="text-2xl font-bold text-gray-900">
              图传伴侣
            </h1>
            <p className="text-sm text-gray-500 mt-1">
              让摄影工作流更简单
            </p>
          </header>
        )}

        {/* Main Content */}
        <div className="space-y-4">
          {/* 主页 */}
          <div className={activeTab === 'home' ? '' : 'hidden'}>
            <div className="space-y-4">
              <ServerCard />
              <InfoCard />
              <LatestPhotoCard />
              <StatsCard />
            </div>
          </div>

          {/* 图库 - 使用 CSS 隐藏代替条件渲染，保持状态和滚动位置 */}
          <div className={activeTab === 'gallery' ? '' : 'hidden'}>
            <GalleryCard />
          </div>

          {/* 配置 */}
          <div className={activeTab === 'config' ? '' : 'hidden'}>
            <ConfigCard />
          </div>
        </div>

        {/* Footer - 只在主页显示 */}
        {activeTab === 'home' && (
          <footer className="text-center py-6 text-xs text-gray-400">
            <p>© 2025 CameraFTP by GoldJohnKing</p>
          </footer>
        )}
      </div>

      {/* Bottom Navigation */}
      <BottomNav />
    </div>
  );
}

export default App;
