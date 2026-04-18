/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Camera, X } from 'lucide-react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { LatestPhotoCard } from './components/LatestPhotoCard';
import { ConfigCard } from './components/ConfigCard';
import { GalleryCard } from './components/GalleryCard';
import { AiEditProgressBar } from './components/AiEditProgressBar';
import { BottomNav } from './components/BottomNav';
import { PermissionDialog } from './components/PermissionDialog';
import { PreviewWindow } from './components/PreviewWindow';
import { useAppBootstrap } from './bootstrap/useAppBootstrap';
import { useQuitFlow } from './hooks/useQuitFlow';
import { enqueueAiEdit, getCurrentAiEditProgress } from './hooks/useAiEditProgress';
import { useServerStore } from './stores/serverStore';
import { useConfigStore } from './stores/configStore';

function App() {
  const { showPermissionDialog, closePermissionDialog, continueAfterPermissionsGranted } = useServerStore();
  const { activeTab } = useConfigStore();
  const updateDraft = useConfigStore(state => state.updateDraft);
  const isPreviewWindow = getCurrentWindow().label === 'preview';
  const { showQuitDialog, closeQuitDialog, handleQuitConfirm } = useQuitFlow({ enabled: !isPreviewWindow });

  useAppBootstrap({ isMainWindow: !isPreviewWindow });

  // Register JS functions for native Android ImageViewerActivity prompt dialog integration
  useEffect(() => {
    const w = window as unknown as Record<string, unknown>;

    w.__tauriGetAiEditPrompt = () => {
      const draft = useConfigStore.getState().draft;
      const manualPrompt = draft?.aiEdit?.manualPrompt || '';
      const manualModel = draft?.aiEdit?.manualModel || '';
      const prompt = manualPrompt || draft?.aiEdit?.prompt || '';
      const model = manualModel || (draft?.aiEdit?.provider?.type === 'seed-edit' ? draft.aiEdit.provider.model : '') || '';
      const autoEdit = draft?.aiEdit?.autoEdit ?? false;
      return JSON.stringify({ prompt, model, autoEdit });
    };

    w.__tauriTriggerAiEditWithPrompt = async (filePath: string, prompt: string, model?: string, saveAsAutoEdit?: boolean) => {
      updateDraft(d => ({
        ...d,
        aiEdit: {
          ...d.aiEdit,
          manualPrompt: prompt,
          manualModel: model ?? '',
          ...(saveAsAutoEdit ? {
            prompt,
            provider: {
              ...d.aiEdit.provider,
              model: model ?? d.aiEdit.provider.model,
            },
          } : {}),
        },
      }));

      await enqueueAiEdit([filePath], prompt, model);
    };

    w.__tauriGetAiEditProgress = () => {
      return getCurrentAiEditProgress();
    };

    w.__tauriCancelAiEdit = async () => {
      const { cancelAiEdit } = await import('./hooks/useAiEditProgress');
      await cancelAiEdit();
    };

    return () => {
      delete w.__tauriGetAiEditPrompt;
      delete w.__tauriTriggerAiEditWithPrompt;
      delete w.__tauriGetAiEditProgress;
      delete w.__tauriCancelAiEdit;
    };
  }, [updateDraft]);

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
              onClick={closeQuitDialog}
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
          {/* 使用 fixed 定位让图库高度匹配屏幕高度 */}
          <div className={activeTab === 'gallery' ? 'fixed inset-0 bg-gray-50 z-0' : 'hidden'}>
            <div className="h-full max-w-md mx-auto">
              <GalleryCard />
            </div>
          </div>

          {/* 配置 */}
          <div className={activeTab === 'config' ? '' : 'hidden'}>
            <ConfigCard />
          </div>
        </div>

        {/* Footer - 只在主页显示 */}
        {activeTab === 'home' && (
          <footer className="text-center py-6 text-xs text-gray-400">
            <p>© 2026 CameraFTP by GoldJohnKing</p>
          </footer>
        )}
      </div>

      {/* AI Edit Progress - always visible overlay */}
      <AiEditProgressBar position="fixed" />

      {/* Bottom Navigation */}
      <BottomNav />
    </div>
  );
}

export default App;
