import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';

interface PreviewEvent {
  file_path: string;
  bring_to_front: boolean;
}

interface PreviewWindowState {
  isOpen: boolean;
  currentImage: string | null;
  autoBringToFront: boolean;
}

export function PreviewWindow() {
  const [state, setState] = useState<PreviewWindowState>({
    isOpen: false,
    currentImage: null,
    autoBringToFront: false,
  });

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

  // 实际预览窗口内容组件
  if (!state.isOpen) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-black">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  return (
    <PreviewWindowContent
      imagePath={state.currentImage}
      autoBringToFront={state.autoBringToFront}
    />
  );
}

// 预览窗口内容组件
function PreviewWindowContent({
  imagePath,
  autoBringToFront,
}: {
  imagePath: string | null;
  autoBringToFront: boolean;
}) {
  const [showToolbar, setShowToolbar] = useState(true);
  const [imageError, setImageError] = useState(false);
  const [localAutoBringToFront, setLocalAutoBringToFront] = useState(autoBringToFront);
  const toolbarTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 同步外部状态
  useEffect(() => {
    setLocalAutoBringToFront(autoBringToFront);
  }, [autoBringToFront]);

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

  // 重置图片错误状态
  useEffect(() => {
    setImageError(false);
  }, [imagePath]);

  const handleMouseMove = () => {
    setShowToolbar(true);
  };

  const handleOpenFolder = async () => {
    if (imagePath) {
      await invoke('open_folder_select_file', { filePath: imagePath });
    }
  };

  const handleToggleAutoFront = async () => {
    try {
      const config = await invoke<{
        enabled: boolean;
        method: string;
        autoBringToFront: boolean;
      }>('get_preview_config');

      const newValue = !localAutoBringToFront;
      const newConfig = {
        ...config,
        autoBringToFront: newValue,
      };
      await invoke('set_preview_config', { config: newConfig });
      setLocalAutoBringToFront(newValue);
    } catch (error) {
      console.error('Failed to update config:', error);
    }
  };

  if (!imagePath) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-black">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  // 使用 convertFileSrc 将文件路径转换为可用的 URL
  const imageSrc = convertFileSrc(imagePath);

  return (
    <div
      className="w-full h-full flex flex-col bg-black"
      onMouseMove={handleMouseMove}
    >
      {/* 图片区域 - 居中显示，保持比例，黑色背景 */}
      <div className="flex-1 relative overflow-hidden flex items-center justify-center bg-black">
        {imageError ? (
          <div className="text-gray-400 text-center">
            <p>无法加载图片</p>
            <p className="text-xs mt-2 text-gray-500">{imagePath}</p>
          </div>
        ) : (
          <img
            src={imageSrc}
            alt="Preview"
            className="max-w-full max-h-full object-contain"
            draggable={false}
            onError={() => setImageError(true)}
          />
        )}
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
          {/* 自动前台按钮 */}
          <button
            onClick={handleToggleAutoFront}
            className={`
              p-2 rounded transition-colors
              ${localAutoBringToFront
                ? 'text-blue-400 bg-blue-400/20 hover:bg-blue-400/30'
                : 'text-gray-400 hover:text-gray-300 hover:bg-gray-700'
              }
            `}
            title={localAutoBringToFront ? '新图片时自动前台显示 (已开启)' : '新图片时自动前台显示 (已关闭)'}
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
