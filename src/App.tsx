import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { useServerStore } from './stores/serverStore';
import { Camera } from 'lucide-react';

function App() {
  const { initializeListeners, startServer, stopServer } = useServerStore();
  const [showQuitDialog, setShowQuitDialog] = useState(false);

  useEffect(() => {
    // 初始化事件监听器
    let cleanup: (() => Promise<void>) | null = null;
    let trayStartUnlisten: (() => void) | null = null;
    let trayStopUnlisten: (() => void) | null = null;
    let windowCloseUnlisten: (() => void) | null = null;
    
    const setupListeners = async () => {
      cleanup = await initializeListeners();
      
      // 监听托盘启动服务器请求
      trayStartUnlisten = await listen('tray-start-server', () => {
        startServer().catch(console.error);
      });
      
      // 监听托盘停止服务器请求
      trayStopUnlisten = await listen('tray-stop-server', () => {
        stopServer().catch(console.error);
      });
      
      // 监听窗口关闭请求（点击X号）- 只有X号才显示确认弹窗
      windowCloseUnlisten = await listen('window-close-requested', () => {
        setShowQuitDialog(true);
      });
    };
    
    setupListeners();
    
    return () => {
      if (cleanup) {
        cleanup();
      }
      if (trayStartUnlisten) {
        trayStartUnlisten();
      }
      if (trayStopUnlisten) {
        trayStopUnlisten();
      }
      if (windowCloseUnlisten) {
        windowCloseUnlisten();
      }
    };
  }, [initializeListeners, startServer, stopServer]);

  const handleQuitConfirm = async (quit: boolean) => {
    if (quit) {
      // 通过Rust命令退出程序
      await invoke('quit_application');
    } else {
      // 先关闭弹窗
      setShowQuitDialog(false);
      // 短暂延迟确保弹窗关闭后再隐藏窗口
      setTimeout(async () => {
        try {
          const window = getCurrentWindow();
          await window.hide();
        } catch (err) {
          console.error('Failed to hide window:', err);
        }
      }, 10);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50">
      {/* 退出确认对话框 */}
      {showQuitDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-sm mx-4 shadow-xl">
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

      <div className="max-w-md mx-auto p-4">
        {/* Header */}
        <header className="text-center py-6">
          <div className="w-16 h-16 bg-blue-600 rounded-2xl flex items-center justify-center mx-auto mb-3">
            <Camera className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-2xl font-bold text-gray-900">
            图传伴侣
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            Camera FTP Companion
          </p>
        </header>

        {/* Main Content */}
        <div className="space-y-4">
          <ServerCard />
          <StatsCard />
          <InfoCard />
        </div>

        {/* Footer */}
        <footer className="text-center py-6 text-xs text-gray-400">
          <p>© 2025 Camera FTP Companion</p>
          <p className="mt-1">让摄影工作流更简单</p>
        </footer>
      </div>
    </div>
  );
}

export default App;
