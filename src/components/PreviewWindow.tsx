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

// 全局配置变化事件类型
interface ConfigChangedEvent {
  config: {
    enabled: boolean;
    method: string;
    autoBringToFront: boolean;
  };
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
  const [isFullscreen, setIsFullscreen] = useState(false);
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

  // 监听全局配置变化事件
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<ConfigChangedEvent>('preview-config-changed', (event) => {
        setLocalAutoBringToFront(event.payload.config.autoBringToFront);
      });
      return unlisten;
    };

    const unlistenPromise = setupListener();
    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, []);

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

  // 监听全屏状态变化 - 使用更可靠的方式
  useEffect(() => {
    let animationFrameId: number;

    const checkFullscreen = async () => {
      const fullscreen = await appWindow.isFullscreen();
      setIsFullscreen(fullscreen);
      animationFrameId = requestAnimationFrame(checkFullscreen);
    };

    checkFullscreen();

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }
    };
  }, [appWindow]);

  // 重置缩放
  const resetZoom = useCallback(() => {
    setScale(1);
    setPanX(0);
    setPanY(0);
  }, []);

  // 切换全屏
  const toggleFullscreen = useCallback(async () => {
    try {
      await appWindow.setFullscreen(!isFullscreen);
    } catch (error) {
      console.error('Failed to toggle fullscreen:', error);
    }
  }, [isFullscreen, appWindow]);

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
      className="w-full h-screen relative bg-black overflow-hidden"
    >
      {/* 图片区域 - 支持缩放和拖拽 */}
      <div 
        ref={containerRef}
        className={`absolute inset-0 flex items-center justify-center bg-black ${
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

      {/* 底部工具栏 - 浮动覆盖在图片上，半透明磨砂效果 */}
      <div
        className={`
          absolute bottom-4 left-4 right-4 
          bg-gray-900/80 backdrop-blur-md 
          border border-gray-700/50
          rounded-xl
          px-4 py-3 flex items-center justify-between
          shadow-lg
          transition-all duration-300
          ${showToolbar ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4 pointer-events-none'}
        `}
        onMouseMove={() => setShowToolbar(true)}
      >
        {/* 左侧：图片信息和缩放比例 */}
        <div className="flex items-center gap-3 flex-1 min-w-0">
          <div className="text-sm text-gray-200 truncate">
            {imagePath.split(/[/\\]/).pop()}
          </div>
          {scale !== 1 && (
            <span className="text-xs text-blue-300 bg-blue-500/20 px-2 py-0.5 rounded">
              {Math.round(scale * 100)}%
            </span>
          )}
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-2">
          {/* 重置缩放按钮 - 箭头向内 */}
          {scale !== 1 && (
            <button
              onClick={resetZoom}
              className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
              title="重置缩放 (双击图片也可重置)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M15 9h4.5M15 9V4.5M15 9l5.25-5.25M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
              </svg>
            </button>
          )}

          {/* 全屏按钮 */}
          <button
            onClick={toggleFullscreen}
            className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
            title={isFullscreen ? '退出全屏' : '全屏显示'}
          >
            {isFullscreen ? (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M15 9h4.5M15 9V4.5M15 9l5.25-5.25M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15" />
              </svg>
            )}
          </button>

          {/* 自动前台按钮 - 使用窗口图标 */}
          <button
            onClick={handleToggleAutoFront}
            className={`
              p-2 rounded-lg transition-colors
              ${localAutoBringToFront
                ? 'text-blue-300 bg-blue-500/20 hover:bg-blue-500/30'
                : 'text-gray-300 hover:text-white hover:bg-white/10'
              }
            `}
            title={localAutoBringToFront ? '新图片时自动前台显示 (已开启)' : '新图片时自动前台显示 (已关闭)'}
          >
            {localAutoBringToFront ? (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" />
              </svg>
            )}
          </button>

          {/* 打开文件夹 */}
          <button
            onClick={handleOpenFolder}
            className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
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
