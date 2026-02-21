# Android 存储路径与权限管理重构设计文档

**日期**: 2025-02-21  
**作者**: Claude Code  
**状态**: 设计完成，待实现  

---

## 1. 设计目标

彻底重构安卓端的存储路径设置和权限管理，解决以下问题：

1. **强制路径配置**：无默认存储路径，必须用户手动选择
2. **简化权限流程**：Toast提示后直接打开SAF选择器，无中间弹窗
3. **智能重新申请**：权限失效后预选中已配置路径，减少用户操作
4. **版本统一**：设置最低支持 Android 11 (API 30)

---

## 2. 核心设计原则

### 2.1 无默认路径原则
- Android端不提供任何默认存储路径
- 用户首次使用必须手动选择存储位置
- 未配置路径时只能查看界面，无法启动服务器

### 2.2 延迟检查原则
- 权限检查不在APP启动时进行
- 仅在用户点击"启动服务器"时检查
- 避免打扰用户的浏览操作

### 2.3 配置同步原则
- 启动弹窗和配置选项卡使用同一配置源
- 两处配置完全同步，修改一处自动更新另一处
- 共享同一个状态管理

### 2.4 SAF优先原则
- 使用 Storage Access Framework (SAF) 选择目录
- 选择路径时自动获得该路径的持久化权限
- 一步完成路径选择和权限申请
- 无需中间弹窗，Toast提示后直接打开SAF选择器

---

## 3. 系统要求

### 3.1 最低系统版本

```
minSdkVersion: 30 (Android 11)
targetSdkVersion: 34 (Android 14)
```

选择 API 30 的原因：
- SAF 功能完整且稳定
- Scoped Storage 强制实施后，SAF 是标准解决方案
- 覆盖绝大多数现代 Android 设备（>95%）

### 3.2 所需权限

```xml
<uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE" />
<uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" />
<uses-permission android:name="android.permission.MANAGE_EXTERNAL_STORAGE"
    tools:ignore="ScopedStorage" />
```

---

## 4. 架构设计

### 4.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         前端 (React/TypeScript)                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   ServerCard    │  │  ConfigCard     │  │   useStorage    │  │
│  │  (启动服务器按钮) │  │ (设置页路径配置) │  │  Permission     │  │
│  └────────┬────────┘  └────────┬────────┘  │  (Hook)         │  │
│           │                    │           └─────────────────┘  │
│           │                    │                                │
│           │  1. 点击启动服务器  │                                │
│           │  2. 检查路径+权限   │                                │
│           │  3. Toast + 打开SAF │                                │
│           └─────────────────────┘                                │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ Tauri IPC
                              │
┌─────────────────────────────┼──────────────────────────────────┐
│                      后端 (Rust)                      │        │
│  ┌─────────────────┐  ┌─────────────────────────────┐ │        │
│  │    commands     │  │    storage_permission.rs    │ │        │
│  │  (Tauri命令)     │  │   (存储权限管理模块)         │ │        │
│  └─────────────────┘  └─────────────────────────────┘ │        │
│           │                         │                 │        │
│           │            ┌────────────┘                 │            │
│           │            │                              │            │
│  ┌────────▼────────────▼─────────────┐                │            │
│  │         Android 平台适配层         │                │            │
│  │  ┌─────────────────────────────┐  │                │            │
│  │  │  src/platform/android.rs    │  │                │            │
│  │  │  (SAF接口封装)               │  │                │            │
│  │  └─────────────────────────────┘  │                │            │
│  └───────────────────────────────────┘                │            │
└───────────────────────────────────────────────────────┴────────────┘
```

### 4.2 组件职责

| 组件 | 职责 |
|------|------|
| `ServerCard` | 启动服务器按钮，触发权限检查和SAF选择器 |
| `ConfigCard` | 设置页路径配置入口，直接打开SAF选择器 |
| `useStoragePermission` | 存储权限状态管理 |
| `storage_permission.rs` | Rust端权限验证和配置管理 |
| `platform/android.rs` | Android SAF 接口封装 |

---

## 5. 交互流程

### 5.1 启动服务器流程

```
用户点击"启动服务器"按钮
    │
    ▼
┌─────────────────────────────────────┐
│ ServerCard 调用 checkAndStartServer │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 检查是否有已配置路径且权限有效       │
│                                     │
│ 条件：                              │
│ - save_path_uri 存在且不为空        │
│ - 通过 SAF 验证该 URI 可读写        │
└─────────────────────────────────────┘
    │
    ├── 否（未配置或权限失效）────────► 显示 Toast 提示
    │                                      │
    │                           ┌───────────────────────┐
    │                           │ Toast: "请先选择用于  │
    │                           │ 保存照片的存储目录"   │
    │                           └───────────────────────┘
    │                                      │
    │                                      ▼
    │                           ┌───────────────────────┐
    │                           │ 自动打开 SAF 选择器    │
    │                           │ ACTION_OPEN_DOCUMENT  │
    │                           │ _TREE                 │
    │                           │                       │
    │                           │ 尝试预选中上次路径：   │
    │                           │ EXTRA_INITIAL_URI     │
    │                           └───────────────────────┘
    │                                      │
    │                                      ▼
    │                           ┌───────────────────────┐
    │                           │ 用户选择目录          │
    │                           │ 系统返回 content://   │
    │                           │ URI                   │
    │                           └───────────────────────┘
    │                                      │
    │                                      ▼
    │                           ┌───────────────────────┐
    │                           │ 前端调用 Rust 命令：   │
    │                           │ - save_storage_path   │
    │                           │ - takePersistableUri  │
    │                           │   Permission          │
    │                           └───────────────────────┘
    │                                      │
    │                                      ▼
    │                           ┌───────────────────────┐
    │                           │ Toast: "存储路径已设置 │
    │                           │ 为：xxx"              │
    │                           │ 自动启动服务器         │
    │                           └───────────────────────┘
    │
    └── 是（路径配置有效）────────────► 直接启动服务器
```

### 5.2 设置页配置流程

```
用户进入"配置"选项卡
    │
    ▼
┌─────────────────────────────────────┐
│ ConfigCard 显示当前存储路径         │
│ - 路径名称                          │
│ - 权限状态（有效/无效）              │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 用户点击"更改存储路径"              │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 直接打开 SAF 选择器（无弹窗）        │
│                                     │
│ 尝试预选中当前路径：                 │
│ EXTRA_INITIAL_URI                   │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 用户选择新路径 / 取消               │
└─────────────────────────────────────┘
    │
    ├──────── 选择 ───────────────► 更新配置并保存
    │                               Toast: "存储路径已更新"
    │                               界面显示新路径
    │
    └──────── 取消 ───────────────► 回到设置页，无变化
```

---

## 6. 数据结构

### 6.1 配置存储格式

```typescript
// src/types/index.ts
interface AppConfig {
  // ... 其他配置项
  
  // 存储路径配置（Android 专用）
  save_path: string;              // 用户友好的路径名（如 "DCIM/CameraFTPCompanion"）
  save_path_uri?: string;         // Android SAF URI（content://...）
  save_path_raw?: string;         // 真实文件路径（如可获取）
}
```

### 6.2 权限状态类型

```typescript
// 存储权限状态
interface StoragePermissionState {
  path: string | null;           // 用户友好的路径名
  uri: string | null;            // content:// URI
  isValid: boolean;              // 权限是否有效
  isChecking: boolean;           // 是否检查中
  lastChecked: number | null;    // 上次检查时间戳
}

// Hook 返回类型
interface UseStoragePermissionReturn {
  state: StoragePermissionState;
  checkPermission: () => Promise<boolean>;
  requestPermission: () => Promise<{ path: string; uri: string } | null>;
  validateBeforeServerStart: () => Promise<boolean>;
  refresh: () => Promise<void>;
}
```

---

## 7. UI 设计

### 7.1 Toast 提示

无需弹窗组件，使用 Toast 提示用户：

#### 场景1：首次启动服务器（无路径配置）

```
┌──────────────────────────────────────────────────┐
│                                                  │
│   ┌───────────────────────────────────────────┐  │
│   │  ⚠️ 请先选择用于保存照片的存储目录        │  │
│   └───────────────────────────────────────────┘  │
│                                                  │
│   [立即自动打开 SAF 选择器]                       │
│                                                  │
└──────────────────────────────────────────────────┘
```

#### 场景2：权限失效后重新选择

```
┌──────────────────────────────────────────────────┐
│                                                  │
│   ┌───────────────────────────────────────────┐  │
│   │  ⚠️ 存储权限已失效，请重新选择目录        │  │
│   │                                           │  │
│   │  当前路径：DCIM/CameraFTPCompanion        │  │
│   │  状态：❌ 权限无效                         │  │
│   └───────────────────────────────────────────┘  │
│                                                  │
│   [立即自动打开 SAF 选择器]                       │
│                                                  │
└──────────────────────────────────────────────────┘
```

#### 场景3：选择成功

```
┌──────────────────────────────────────────────────┐
│                                                  │
│   ┌───────────────────────────────────────────┐  │
│   │  ✅ 存储路径已设置为：DCIM/CameraFTP     │  │
│   └───────────────────────────────────────────┘  │
│                                                  │
│   [自动启动服务器]                                │
│                                                  │
└──────────────────────────────────────────────────┘
```

#### 场景4：用户取消选择

```
┌──────────────────────────────────────────────────┐
│                                                  │
│   ┌───────────────────────────────────────────┐  │
│   │  ⚠️ 未选择存储路径，服务器未启动          │  │
│   └───────────────────────────────────────────┘  │
│                                                  │
└──────────────────────────────────────────────────┘
```

### 7.2 ConfigCard 设置页

```
┌──────────────────────────────────────────────────┐
│ 存储设置                                          │
├──────────────────────────────────────────────────┤
│                                                  │
│  当前路径                                         │
│  ┌──────────────────────────────────────────┐    │
│  │ 📁 DCIM/CameraFTPCompanion      ✅ 正常   │    │
│  └──────────────────────────────────────────┘    │
│                                                  │
│  [  更改存储路径  ]                               │
│                                                  │
└──────────────────────────────────────────────────┘
```

---

## 8. API 设计

### 8.1 Rust Tauri 命令

```rust
/// 验证存储路径权限是否有效
#[tauri::command]
pub async fn validate_storage_permission(
    app: AppHandle,
    uri: String,
) -> Result<bool, String>;

/// 保存存储路径配置
#[tauri::command]
pub async fn save_storage_path(
    app: AppHandle,
    path_name: String,      // 用户友好的路径名
    uri: String,            // content:// URI
) -> Result<(), String>;

/// 获取当前存储路径配置
#[tauri::command]
pub fn get_storage_path(app: AppHandle) -> Result<Option<StoragePathInfo>, String>;

/// 获取推荐存储路径（用于初始化建议）
#[tauri::command]
pub fn get_recommended_storage_path(app: AppHandle) -> Result<String, String>;

/// 启动服务器前的检查
#[tauri::command]
pub async fn check_server_start_prerequisites(
    app: AppHandle,
) -> Result<ServerStartCheckResult, String>;

/// 结构定义
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoragePathInfo {
    pub path_name: String,
    pub uri: String,
    pub is_valid: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,     // 如果不能启动，原因
    pub current_path: Option<StoragePathInfo>,
}
```

### 8.2 前端 Hook API

```typescript
// useStoragePermission.ts
export function useStoragePermission(): UseStoragePermissionReturn;

// 使用示例
function ServerCard() {
  const { validateBeforeServerStart } = useStoragePermission();
  const [showStorageDialog, setShowStorageDialog] = useState(false);
  
  const handleStartServer = async () => {
    const canStart = await validateBeforeServerStart();
    
    if (!canStart) {
      setShowStorageDialog(true);
      return;
    }
    
    // 可以启动，继续启动服务器
    await startServer();
  };
  
  return (
    <>
      <button onClick={handleStartServer}>启动服务器</button>
      <StorageDialog 
        isOpen={showStorageDialog}
        onClose={() => setShowStorageDialog(false)}
        onPathSelected={() => {
          setShowStorageDialog(false);
          startServer(); // 选择路径后自动启动
        }}
      />
    </>
  );
}
```

---

## 9. 错误处理

### 9.1 错误场景与处理

| 错误场景 | 用户提示 | 处理方式 |
|---------|---------|---------|
| SAF 选择器崩溃 | "无法打开文件选择器，请重试" | 提供重试按钮 |
| 用户取消选择 | （无提示） | 关闭弹窗，不启动服务器 |
| 持久化权限失败 | "无法保存权限，请重新选择" | 返回选择器 |
| 路径不可写 | "该目录无法写入，请选择其他目录" | 返回选择器 |
| 路径验证超时 | "验证超时，请重试" | 提供重试按钮 |

### 9.2 错误边界

```typescript
// StorageDialog 错误处理
const handleSelectPath = async () => {
  try {
    setIsLoading(true);
    const result = await openSAFPicker(initialUri);
    
    if (!result) {
      // 用户取消，不做任何操作
      return;
    }
    
    // 验证路径
    const isValid = await validatePath(result.uri);
    if (!isValid) {
      toast.error('该目录无法访问，请重新选择');
      return;
    }
    
    // 保存配置
    await saveStoragePath(result.name, result.uri);
    
    // 成功回调
    onPathSelected(result);
    
  } catch (error) {
    toast.error('选择路径失败：' + error.message);
  } finally {
    setIsLoading(false);
  }
};
```

---

## 10. 与现有代码的集成

### 10.1 配置存储兼容

现有配置结构：
```rust
pub struct AppConfig {
    pub save_path: PathBuf,
    // ...
}
```

新设计保持 `save_path` 字段不变（存储用户友好的路径名），新增 `save_path_uri` 字段存储 SAF URI。

### 10.2 ServerCard 集成

修改现有的 `startServer` 逻辑，在真正启动前检查权限：

```typescript
// stores/serverStore.ts
const startServer = async () => {
  // 1. 检查存储权限
  const { validateBeforeServerStart } = useStoragePermission.getState();
  const canStart = await validateBeforeServerStart();
  
  if (!canStart) {
    // 触发显示 StorageDialog
    eventBus.emit('show-storage-dialog');
    return;
  }
  
  // 2. 原有启动逻辑
  // ...
};
```

### 10.3 ConfigCard 集成

ConfigCard 中复用 StorageDialog 组件，共享相同的选择逻辑。

---

## 11. 测试策略

### 11.1 单元测试

- `validate_storage_permission`: 验证各种 URI 的有效性
- `save_storage_path`: 配置保存和加载
- `useStoragePermission` Hook: 状态管理和流程验证

### 11.2 集成测试

- 启动服务器完整流程
- 权限失效后重新选择流程
- 设置页更改路径流程

### 11.3 手动测试清单

- [ ] 首次安装APP，点击启动服务器，显示Toast提示并自动打开SAF选择器
- [ ] 选择路径后，自动启动服务器
- [ ] 进入设置页，显示当前路径
- [ ] 在设置页更改路径，配置同步更新
- [ ] 撤销权限后，下次启动服务器提示重新选择
- [ ] 重新选择时，尝试预选中上次路径
- [ ] 取消选择，不启动服务器，状态不变

---

## 12. 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| SAF 选择器在某些 ROM 上表现异常 | 中 | 高 | 测试主流 ROM，提供降级提示 |
| `EXTRA_INITIAL_URI` 预选中无效 | 高 | 低 | 优雅降级，不预选中 |
| 持久化权限被系统回收 | 低 | 中 | 每次启动服务器验证，失效后提示 |
| 用户选择的路径不可写 | 低 | 中 | 选择后立即验证，失败提示重选 |

---

## 13. 后续优化（可选）

1. **路径快捷选择**：提供"DCIM"、"Pictures"等常用路径快捷按钮
2. **路径历史记录**：保存最近使用的3个路径
3. **智能推荐**：根据存储空间推荐合适的默认位置
4. **权限自动恢复**：监听系统权限变化，主动提示

---

## 14. 决策记录

### 决策1：最低版本设为 Android 11 (API 30)

**背景**: 需要确保 SAF 功能完整可用

**选择**: minSdkVersion = 30

**原因**:
- SAF 在 API 30 上完全成熟
- Scoped Storage 强制执行后，SAF 是标准方案
- 覆盖绝大多数现代设备

**影响**: 放弃 Android 10 及以下设备（约 5% 市场份额）

---

### 决策2：使用 SAF 而非 MANAGE_EXTERNAL_STORAGE

**背景**: 需要访问用户选择的任意目录

**选择**: Storage Access Framework

**原因**:
- 用户体验更好（内置选择器）
- 权限粒度更细（仅申请需要的目录）
- Google Play 审核更友好

**替代方案**: MANAGE_EXTERNAL_STORAGE 需要跳转到系统设置，体验差

---

### 决策3：使用Toast提示直接打开SAF选择器

**背景**: 需要处理首次配置和权限失效两种场景

**选择**: Toast提示后直接打开SAF选择器，无中间弹窗

**原因**:
- 减少用户操作步骤
- 更流畅的用户体验
- 符合原生应用行为模式

**影响**: 无法向用户展示详细的状态信息，需要依赖Toast传达关键信息

---

## 15. 附录

### 15.1 SAF 相关参考

- [Android Storage Access Framework](https://developer.android.com/guide/topics/providers/document-provider)
- [Persist permissions](https://developer.android.com/training/data-storage/shared/documents-files#persist-permissions)
- [Open document tree](https://developer.android.com/reference/android/content/Intent#ACTION_OPEN_DOCUMENT_TREE)

### 15.2 Tauri 相关参考

- [Tauri Android Plugin Development](https://tauri.app/develop/plugins/mobile/)
- [Tauri Command System](https://tauri.app/develop/calling-rust/)

---

**文档版本**: 1.0  
**最后更新**: 2025-02-21
