import { useEffect, useState } from 'react';
import { Folder, Check, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';

export function ConfigCard() {
  const {
    config,
    isLoading,
    error,
    loadConfig,
    updateSavePath,
    setAutostart,
    selectDirectory,
  } = useConfigStore();

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [isLoadingAutostart, setIsLoadingAutostart] = useState(false);

  useEffect(() => {
    loadConfig();
    loadAutostartStatus();
  }, []);

  const loadAutostartStatus = async () => {
    try {
      const status = await invoke<boolean>('get_autostart_status');
      setAutostartEnabled(status);
    } catch (err) {
      console.error('Failed to load autostart status:', err);
    }
  };

  const handleSelectDirectory = async () => {
    const selected = await selectDirectory();
    if (selected) {
      await updateSavePath(selected);
      showSaveSuccess();
    }
  };

  const handleAutostartToggle = async () => {
    setIsLoadingAutostart(true);
    try {
      const newValue = !autostartEnabled;
      await setAutostart(newValue);
      setAutostartEnabled(newValue);
      showSaveSuccess();
    } finally {
      setIsLoadingAutostart(false);
    }
  };

  const showSaveSuccess = () => {
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2000);
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
      <div className="p-4 border-b border-gray-100">
        <h2 className="text-lg font-semibold text-gray-900">应用配置</h2>
        <p className="text-sm text-gray-500 mt-1">管理应用设置和偏好</p>
      </div>

      <div className="p-4 space-y-6">
        {/* 存储路径配置 */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-2">
            存储路径
          </label>
          <div className="flex gap-2">
            <div className="flex-1 min-w-0">
              <div className="w-full px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg text-sm text-gray-700 truncate">
                {config?.save_path || '未设置'}
              </div>
            </div>
            <button
              onClick={handleSelectDirectory}
              disabled={isLoading}
              className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Folder className="w-4 h-4" />
              <span className="text-sm">选择</span>
            </button>
          </div>
          <p className="text-xs text-gray-500 mt-2">
            相机上传的文件将保存到此目录
          </p>
        </div>

        {/* 开机自启动配置 */}
        <div className="flex items-center justify-between py-2">
          <div>
            <label className="block text-sm font-medium text-gray-700">
              开机自启动
            </label>
            <p className="text-xs text-gray-500 mt-1">
              系统启动时自动运行图传伴侣
            </p>
          </div>
          <button
            onClick={handleAutostartToggle}
            disabled={isLoadingAutostart}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              autostartEnabled ? 'bg-blue-600' : 'bg-gray-200'
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            {isLoadingAutostart ? (
              <Loader2 className="w-4 h-4 animate-spin text-white absolute left-1" />
            ) : (
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  autostartEnabled ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            )}
          </button>
        </div>

        {/* 保存成功提示 */}
        {saveSuccess && (
          <div className="flex items-center gap-2 text-green-600 text-sm">
            <Check className="w-4 h-4" />
            <span>设置已保存</span>
          </div>
        )}

        {/* 错误提示 */}
        {error && (
          <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
            <p className="text-sm text-red-600">{error}</p>
          </div>
        )}
      </div>
    </div>
  );
}
