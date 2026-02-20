# 配置选项卡功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 添加配置选项卡，将开机自启动移入配置页面，新增存储路径配置功能

**Architecture:** 采用响应式底部导航栏设计（跨平台Windows+Android），创建独立的Config页面组件，使用Zustand管理配置状态，通过Tauri IPC与Rust后端通信读写配置

**Tech Stack:** React + TypeScript + TailwindCSS + Zustand + Tauri + Rust

---

## Task 1: 创建配置状态管理Store

**Files:**
- Create: `src/stores/configStore.ts`

**Step 1: 创建配置Store**

```typescript
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';

interface ConfigState {
  config: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'config';
  
  // Actions
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  updateSavePath: (path: string) => Promise<void>;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  selectDirectory: () => Promise<string | null>;
}

const defaultConfig: AppConfig = {
  save_path: '',
  auto_open: true,
  auto_open_program: null,
  port: 21,
  file_extensions: [],
};

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  isLoading: false,
  error: null,
  activeTab: 'home',

  loadConfig: async () => {
    set({ isLoading: true, error: null });
    try {
      const config = await invoke<AppConfig>('load_config');
      set({ config, isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to load config', isLoading: false });
    }
  },

  saveConfig: async (config: AppConfig) => {
    set({ isLoading: true, error: null });
    try {
      await invoke('save_config', { config });
      set({ config, isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to save config', isLoading: false });
      throw err;
    }
  },

  updateSavePath: async (path: string) => {
    const { config, saveConfig } = get();
    if (!config) return;
    const newConfig = { ...config, save_path: path };
    await saveConfig(newConfig);
  },

  setAutostart: async (enabled: boolean) => {
    set({ isLoading: true, error: null });
    try {
      await invoke('set_autostart_command', { enable: enabled });
      set({ isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to set autostart', isLoading: false });
      throw err;
    }
  },

  setActiveTab: (tab: 'home' | 'config') => {
    set({ activeTab: tab });
  },

  selectDirectory: async () => {
    try {
      const selected = await invoke<string | null>('select_directory');
      return selected;
    } catch (err) {
      console.error('Failed to select directory:', err);
      return null;
    }
  },
}));
```

**Step 2: 提交**

```bash
git add src/stores/configStore.ts
git commit -m "feat: add config store for managing settings"
```

---

## Task 2: 添加Rust后端选择目录命令

**Files:**
- Modify: `src-tauri/src/commands.rs` (添加新命令)
- Modify: `src-tauri/src/lib.rs` (注册命令)

**Step 1: 在commands.rs中添加选择目录命令**

在文件末尾添加：

```rust
/// 选择目录对话框
#[tauri::command]
pub async fn select_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let folder_path = app
        .dialog()
        .file()
        .set_title("选择存储路径")
        .blocking_pick_folder();
    
    Ok(folder_path.map(|p| p.to_string_lossy().to_string()))
}
```

**Step 2: 在lib.rs中注册命令**

找到 `generate_handler!` 宏调用，添加 `select_directory`：

```rust
.invoke_handler(tauri::generate_handler![
    start_server,
    stop_server,
    get_server_status,
    get_network_info,
    load_config,
    save_config,
    check_port_available,
    set_autostart_command,
    get_autostart_status,
    quit_application,
    hide_main_window,
    select_directory,  // 添加这一行
])
```

**Step 3: 确保Cargo.toml包含dialog插件**

检查 `src-tauri/Cargo.toml` 是否已有：

```toml
[dependencies]
tauri-plugin-dialog = "2"
```

**Step 4: 在lib.rs中初始化dialog插件**

找到 `Builder` 链式调用，确保包含：

```rust
.plugin(tauri_plugin_dialog::init())
```

**Step 5: 提交**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add select directory command for config page"
```

---

## Task 3: 创建底部导航栏组件

**Files:**
- Create: `src/components/BottomNav.tsx`

**Step 1: 创建底部导航组件**

```typescript
import { Home, Settings } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';

export function BottomNav() {
  const { activeTab, setActiveTab } = useConfigStore();

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 safe-area-pb">
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
}
```

**Step 2: 在index.css中添加安全区域样式**

```css
/* 为底部导航栏添加安全区域适配（iOS/Android刘海屏） */
.safe-area-pb {
  padding-bottom: env(safe-area-inset-bottom, 0px);
}
```

**Step 3: 提交**

```bash
git add src/components/BottomNav.tsx src/index.css
git commit -m "feat: add bottom navigation bar for tab switching"
```

---

## Task 4: 创建配置页面组件

**Files:**
- Create: `src/components/ConfigCard.tsx`

**Step 1: 创建配置卡片组件**

```typescript
import { useEffect, useState } from 'react';
import { Folder, Power, Loader2, Check } from 'lucide-react';
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
    const newValue = !autostartEnabled;
    await setAutostart(newValue);
    setAutostartEnabled(newValue);
    showSaveSuccess();
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
            disabled={isLoading}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              autostartEnabled ? 'bg-blue-600' : 'bg-gray-200'
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                autostartEnabled ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
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
```

**Step 2: 添加缺失的import**

在文件顶部添加：

```typescript
import { invoke } from '@tauri-apps/api/core';
```

**Step 3: 提交**

```bash
git add src/components/ConfigCard.tsx
git commit -m "feat: add config page component with save path and autostart settings"
```

---

## Task 5: 修改App.tsx集成选项卡功能

**Files:**
- Modify: `src/App.tsx`

**Step 1: 修改App.tsx**

```typescript
import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { Camera } from 'lucide-react';
import { ServerCard } from './components/ServerCard';
import { StatsCard } from './components/StatsCard';
import { InfoCard } from './components/InfoCard';
import { ConfigCard } from './components/ConfigCard';
import { BottomNav } from './components/BottomNav';
import { useServerStore } from './stores/serverStore';
import { useConfigStore } from './stores/configStore';

function App() {
  const { initializeListeners, startServer, stopServer } = useServerStore();
  const { activeTab, loadConfig } = useConfigStore();
  const [showQuitDialog, setShowQuitDialog] = useState(false);

  useEffect(() => {
    // 初始化配置
    loadConfig();
    
    // 初始化事件监听器
    let cleanup: (() => Promise<void>) | null = null;
    let trayStartUnlisten: (() => void) | null = null;
    let trayStopUnlisten: (() => void) | null = null;
    let windowCloseUnlisten: (() => void) | null = null;
    
    const setupListeners = async () => {
      cleanup = await initializeListeners();
      
      trayStartUnlisten = await listen('tray-start-server', () => {
        startServer().catch(console.error);
      });
      
      trayStopUnlisten = await listen('tray-stop-server', () => {
        stopServer().catch(console.error);
      });
      
      windowCloseUnlisten = await listen('window-close-requested', () => {
        setShowQuitDialog(true);
      });
    };
    
    setupListeners();
    
    return () => {
      if (cleanup) cleanup();
      if (trayStartUnlisten) trayStartUnlisten();
      if (trayStopUnlisten) trayStopUnlisten();
      if (windowCloseUnlisten) windowCloseUnlisten();
    };
  }, [initializeListeners, startServer, stopServer, loadConfig]);

  const handleQuitConfirm = async (quit: boolean) => {
    if (quit) {
      await invoke('quit_application');
    } else {
      setShowQuitDialog(false);
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
            <h3 className="text-lg font-semibold text-gray-900 mb-2">确认退出</h3>
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
            <h1 className="text-2xl font-bold text-gray-900">图传伴侣</h1>
            <p className="text-sm text-gray-500 mt-1">Camera FTP Companion</p>
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
```

**Step 2: 提交**

```bash
git add src/App.tsx
git commit -m "feat: integrate tabs into main app with bottom navigation"
```

---

## Task 6: 验证TypeScript类型

**Step 1: 运行类型检查**

```bash
cd /mnt/d/GitRepos/camera-ftp-companion && npx tsc --noEmit
```

**Step 2: 修复任何类型错误**

根据错误提示修复类型问题。

**Step 3: 提交**

```bash
git commit -m "fix: resolve TypeScript type issues"
```

---

## Task 7: 测试功能

**Step 1: 启动开发服务器测试**

```bash
bun run dev
```

**Step 2: 验证以下功能：**
- 底部导航栏显示正常
- 点击"主页"和"配置"可以切换选项卡
- 配置页面可以显示当前存储路径
- 点击"选择"按钮可以打开目录选择对话框
- 选择目录后路径更新并保存
- 开机自启动开关可以正常工作
- 切换回主页后服务器功能正常

**Step 3: 完整构建测试**

```bash
./build-full.sh
```

**Step 4: 提交最终更改**

```bash
git add .
git commit -m "feat: complete config tab implementation with save path and autostart"
```

---

## 总结

完成以上所有任务后，应用将具备：

1. ✅ 底部导航栏（主页/配置选项卡）
2. ✅ 配置页面包含存储路径设置
3. ✅ 配置页面包含开机自启动开关
4. ✅ 跨平台兼容的UI设计
5. ✅ 所有设置自动持久化

**下一步建议：**
- 可以添加更多配置项（端口设置、文件类型过滤等）
- 考虑添加导入/导出配置功能
- 为Android平台优化底部导航栏样式
