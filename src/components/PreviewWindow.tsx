import { useEffect, useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

interface PreviewEvent {
  file_path: string;
  bring_to_front: boolean;
}

interface PreviewWindowState {
  isOpen: boolean;
  currentImage: string | null;
  isFullscreen: boolean;
  autoBringToFront: boolean;
}

export function PreviewWindow() {
  const [state, setState] = useState<PreviewWindowState>({
    isOpen: false,
    currentImage: null,
    isFullscreen: false,
    autoBringToFront: false,
  });

  const appWindow = getCurrentWindow();

  // 监听 Rust 发来的预览事件
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      unlisten = await listen<PreviewEvent>('preview-image', (event) => {
        const { file_path, bring_to_front } = event.payload;

        setState(prev => ({
          ...prev,
          isOpen: true,
          currentImage: file_path,
          autoBringToFront: bring_to_front,
        }));
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // 处理全屏切换
  const toggleFullscreen = useCallback(async () => {
    try {
      const isFullscreen = await appWindow.isFullscreen();
      await appWindow.setFullscreen(!isFullscreen);
      setState(prev => ({ ...prev, isFullscreen: !isFullscreen }));
    } catch (error) {
      console.error('Failed to toggle fullscreen:', error);
    }
  }, [appWindow]);

  // ESC 键退出全屏
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        const isFullscreen = await appWindow.isFullscreen();
        if (isFullscreen) {
          await appWindow.setFullscreen(false);
          setState(prev => ({ ...prev, isFullscreen: false }));
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [appWindow]);

  // 实际预览窗口内容组件
  if (!state.isOpen) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-gray-900">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  return (
    <PreviewWindowContent
      imagePath={state.currentImage}
      isFullscreen={state.isFullscreen}
      autoBringToFront={state.autoBringToFront}
      onFullscreenToggle={toggleFullscreen}
    />
  );
}

// 预览窗口内容组件
function PreviewWindowContent({
  imagePath,
  isFullscreen,
  autoBringToFront,
  onFullscreenToggle,
}: {
  imagePath: string | null;
  isFullscreen: boolean;
  autoBringToFront: boolean;
  onFullscreenToggle: () => void;
}) {
  const [showToolbar, setShowToolbar] = useState(true);
  const toolbarTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 自动隐藏工具栏
  useEffect(() => {
    if (toolbarTimeoutRef.current) {
      clearTimeout(toolbarTimeoutRef.current);
    }

    toolbarTimeoutRef.current = setTimeout(() => {
      setShowToolbar(false);
    }, 3000);

    return () => {
      if (toolbarTimeoutRef.current) {
        clearTimeout(toolbarTimeoutRef.current);
      }
    };
  }, [showToolbar]);

  const handleMouseMove = () => {
    setShowToolbar(true);
  };

  const handleOpenFolder = async () => {
    if (imagePath) {
      await invoke('open_folder_select_file', { filePath: imagePath });
    }
  };

  const handleToggleAutoFront = async () => {
    // 更新配置 - 这里需要通过store或其他方式获取当前配置
    try {
      const config = await invoke<{
        enabled: boolean;
        method: string;
        autoBringToFront: boolean;
        rememberPosition: boolean;
      }>('get_preview_config');

      const newConfig = {
        ...config,
        autoBringToFront: !autoBringToFront,
      };
      await invoke('set_preview_config', { config: newConfig });
    } catch (error) {
      console.error('Failed to update config:', error);
    }
  };

  if (!imagePath) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-gray-900">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  return (
    <div
      className={`w-full h-full flex flex-col bg-gray-900 ${isFullscreen ? 'fixed inset-0 z-50' : ''}`}
      onMouseMove={handleMouseMove}
    >
      {/* 图片区域 - 始终填满，object-cover 保持比例裁剪 */}
      <div className="flex-1 relative overflow-hidden">
        <img
          src={`file://${imagePath}`}
          alt="Preview"
          className="w-full h-full object-cover"
          draggable={false}
          onDoubleClick={onFullscreenToggle}
        />
      </div>

      {/* 底部工具栏 */}
      <div
        className={`
          bg-gray-800 px-4 py-3 flex items-center justify-between
          transition-opacity duration-300
          ${showToolbar ? 'opacity-100' : 'opacity-0'}
        `}
      >
        {/* 左侧：图片信息 */}
        <div className="text-sm text-gray-300 truncate flex-1">
          {imagePath.split(/[/\\]/).pop()}
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-3">
          {/* 全屏按钮 */}
          <button
            onClick={onFullscreenToggle}
            className="p-2 text-gray-300 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title={isFullscreen ? '退出全屏' : '全屏'}
          >
            {isFullscreen ? (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15" />
              </svg>
            )}
          </button>

          {/* 自动前台按钮 */}
          <button
            onClick={handleToggleAutoFront}
            className={`
              p-2 rounded transition-colors
              ${autoBringToFront
                ? 'text-blue-400 bg-blue-400/20 hover:bg-blue-400/30'
                : 'text-gray-400 hover:text-gray-300 hover:bg-gray-700'
              }
            `}
            title={autoBringToFront ? '新图片时自动前台显示 (已开启)' : '新图片时自动前台显示 (已关闭)'}
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
            </svg>
          </button>

          {/* 打开文件夹 */}
          <button
            onClick={handleOpenFolder}
            className="p-2 text-gray-300 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="打开文件夹"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
