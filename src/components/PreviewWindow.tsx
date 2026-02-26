import { useEffect, useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

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

  // 缩放和拖拽状态
  const [scale, setScale] = useState(1);
  const [panX, setPanX] = useState(0);
  const [panY, setPanY] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const appWindow = getCurrentWindow();

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

  // 重置图片错误状态和缩放
  useEffect(() => {
    setImageError(false);
    resetZoom();
  }, [imagePath]);

  // 监听窗口大小变化，重置缩放
  useEffect(() => {
    const handleResize = () => {
      resetZoom();
    };

    // 使用 Tauri 的窗口大小变化监听
    const unlisten = appWindow.onResized(handleResize);
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, [appWindow]);

  // 重置缩放
  const resetZoom = useCallback(() => {
    setScale(1);
    setPanX(0);
    setPanY(0);
  }, []);

  // 处理鼠标滚轮缩放
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    
    const container = containerRef.current;
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    // 计算缩放因子 - 最小为1（不裁切充满窗口）
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    const newScale = Math.max(1, Math.min(5, scale * zoomFactor));

    if (newScale !== scale) {
      // 以鼠标位置为中心缩放
      const scaleRatio = newScale / scale;
      const newPanX = mouseX - (mouseX - panX) * scaleRatio;
      const newPanY = mouseY - (mouseY - panY) * scaleRatio;

      setScale(newScale);
      // 只有在缩放大于1时才允许有平移
      if (newScale > 1) {
        setPanX(newPanX);
        setPanY(newPanY);
      } else {
        setPanX(0);
        setPanY(0);
      }
    }
  }, [scale, panX, panY]);

  // 处理鼠标按下（开始拖拽）
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (scale > 1) {
      setIsDragging(true);
      dragStartRef.current = {
        x: e.clientX - panX,
        y: e.clientY - panY,
      };
    }
  }, [scale, panX, panY]);

  // 处理鼠标移动（拖拽中）
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    // 更新工具栏显示
    setShowToolbar(true);

    if (isDragging && scale > 1) {
      setPanX(e.clientX - dragStartRef.current.x);
      setPanY(e.clientY - dragStartRef.current.y);
    }
  }, [isDragging, scale]);

  // 处理鼠标释放（结束拖拽）
  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  // 处理双击重置
  const handleDoubleClick = useCallback(() => {
    resetZoom();
  }, [resetZoom]);

  // 全局鼠标释放监听（防止拖拽时移出窗口）
  useEffect(() => {
    const handleGlobalMouseUp = () => setIsDragging(false);
    window.addEventListener('mouseup', handleGlobalMouseUp);
    return () => window.removeEventListener('mouseup', handleGlobalMouseUp);
  }, []);

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
      className="w-full h-screen flex flex-col bg-black overflow-hidden"
    >
      {/* 图片区域 - 支持缩放和拖拽，添加 overflow-hidden 防止图片遮挡工具栏 */}
      <div 
        ref={containerRef}
        className={`flex-1 min-h-0 relative overflow-hidden flex items-center justify-center bg-black ${
          isDragging ? 'cursor-grabbing' : scale > 1 ? 'cursor-grab' : 'cursor-default'
        }`}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onDoubleClick={handleDoubleClick}
      >
        {imageError ? (
          <div className="text-gray-400 text-center">
            <p>无法加载图片</p>
            <p className="text-xs mt-2 text-gray-500">{imagePath}</p>
          </div>
        ) : (
          <img
            src={imageSrc}
            alt="Preview"
            className="max-w-full max-h-full object-contain select-none"
            style={{
              transform: `translate(${panX}px, ${panY}px) scale(${scale})`,
              transformOrigin: 'center center',
            }}
            draggable={false}
            onError={() => setImageError(true)}
          />
        )}
      </div>

      {/* 底部工具栏 */}
      <div
        className={`
          bg-gray-800 px-4 py-3 flex items-center justify-between shrink-0
          transition-opacity duration-300
          ${showToolbar ? 'opacity-100' : 'opacity-0'}
        `}
        onMouseMove={() => setShowToolbar(true)}
      >
        {/* 左侧：图片信息和缩放比例 */}
        <div className="flex items-center gap-3 flex-1 min-w-0">
          <div className="text-sm text-gray-300 truncate">
            {imagePath.split(/[/\\]/).pop()}
          </div>
          {scale !== 1 && (
            <span className="text-xs text-blue-400 bg-blue-400/20 px-2 py-0.5 rounded">
              {Math.round(scale * 100)}%
            </span>
          )}
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-3">
          {/* 重置缩放按钮 */}
          {scale !== 1 && (
            <button
              onClick={resetZoom}
              className="p-2 text-gray-300 hover:text-white hover:bg-gray-700 rounded transition-colors"
              title="重置缩放 (双击图片也可重置)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
              </svg>
            </button>
          )}

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
