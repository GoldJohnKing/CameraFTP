import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Camera } from 'lucide-react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { ConfigCard } from './components/ConfigCard';
import { BottomNav } from './components/BottomNav';
import { useServerStore } from './stores/serverStore';
import { useConfigStore } from './stores/configStore';
import { useTauriListeners } from './hooks/useTauriListeners';

function App() {
  const { initializeListeners, startServer, stopServer } = useServerStore();
  const { activeTab, loadConfig } = useConfigStore();
  const [showQuitDialog, setShowQuitDialog] = useState(false);

  // 初始化 store 的监听器
  useEffect(() => {
    const setup = async () => {
      const cleanup = await initializeListeners();
      return cleanup;
    };
    
    let cleanupFn: (() => void) | undefined;
    setup().then(cleanup => {
      cleanupFn = cleanup;
    });
    
    return () => {
      cleanupFn?.();
    };
  }, [initializeListeners]);

  // 使用自定义 hook 管理其他监听器
  useTauriListeners([
    {
      event: 'tray-start-server',
      handler: () => startServer().catch(console.error)
    },
    {
      event: 'tray-stop-server',
      handler: () => stopServer().catch(console.error)
    },
    {
      event: 'window-close-requested',
      handler: () => setShowQuitDialog(true)
    },
    {
      event: 'android-open-manage-storage-settings',
      handler: () => {
        // Android: 跳转到设置页面开启所有文件访问权限
        if ((window as any).SAFPickerAndroid?.openAllFilesAccessSettings) {
          (window as any).SAFPickerAndroid.openAllFilesAccessSettings();
        } else {
          console.warn('SAFPickerAndroid.openAllFilesAccessSettings not available');
          // 备用方案：提示用户手动开启
          alert('请手动前往 设置 > 应用 > 图传伴侣 > 权限 > 开启"所有文件访问权限"');
        }
      }
    },
  ]);

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
        console.error('Failed to hide window:', err);
      }
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 pb-20">
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
              Camera FTP Companion
            </p>
          </header>
        )}

        {/* Main Content */}
        <div className="space-y-4">
          {activeTab === 'home' ? (
            <>
              <ServerCard />
              <InfoCard />
              <StatsCard />
            </>
          ) : (
            <>
              <div className="py-4">
                <h1 className="text-2xl font-bold text-gray-900">配置</h1>
                <p className="text-sm text-gray-500 mt-1">管理应用设置</p>
              </div>
              <ConfigCard />
            </>
          )}
        </div>

        {/* Footer - 只在主页显示 */}
        {activeTab === 'home' && (
          <footer className="text-center py-6 text-xs text-gray-400">
            <p>© 2025 Camera FTP Companion</p>
            <p className="mt-1">让摄影工作流更简单</p>
          </footer>
        )}
      </div>

      {/* Bottom Navigation */}
      <BottomNav />
    </div>
  );
}

export default App;
