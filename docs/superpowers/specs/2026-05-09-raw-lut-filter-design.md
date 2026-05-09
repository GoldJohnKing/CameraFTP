# RAW LUT Filter — 设计文档

> **状态**: C API 已确认，待实施
> **日期**: 2026-05-09
> **作者**: GoldJohnKing

---

## 1. 概述

将 RawAlchemyCpp（C++ RAW 处理管线）集成到 CameraFTP 图传伴侣中，为 RAW 图片提供一键 LUT 滤镜功能。

### 1.1 需求摘要

| 项目 | 选择 |
|------|------|
| 集成范围 | 完整 RAW 处理管线（解码→镜头校正→测光→风格化→Log→LUT→JPEG） |
| 目标平台 | Windows + Android |
| UI 交互 | Gallery / PreviewWindow 一键应用，选择预制 LUT 名称列表 |
| LUT 来源 | 仅预制 LUT（.cube 文件内嵌） |
| 输出格式 | JPEG |
| 镜头校正 | 配置文件内嵌到编译产物 |
| Log 空间 | 每个预制 LUT 在代码中硬绑定源 Log 空间 |
| 集成方式 | RawAlchemyCpp 产出动态库，Tauri 应用加载调用 |

### 1.2 不做的事

- 不支持用户导入自定义 LUT
- 不提供复杂参数配置界面（镜头校正、测光模式等使用默认值）
- 不支持手动选择 Log 空间
- 不替换原图

---

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (React)                        │
│  GalleryCard · PreviewWindow · LutFilterDialog              │
│  (选择预制 LUT → invoke('enqueue_lut_filter'))              │
└──────────────────────────┬──────────────────────────────────┘
                           │ Tauri IPC
┌──────────────────────────▼──────────────────────────────────┐
│                  Rust (src-tauri)                            │
│  commands/lut_filter.rs  — Tauri 命令                        │
│  lut_filter/service.rs   — 队列化处理 worker                 │
│  lut_filter/ffi.rs       — 动态库加载与 FFI 声明             │
│  lut_filter/presets.rs   — 预制 LUT 注册表                   │
│  lut_filter/resources.rs — 资源文件管理（释放/路径解析）     │
└──────────────────────────┬──────────────────────────────────┘
                           │ 动态库调用 (dlopen/LoadLibrary 或编译时链接)
┌──────────────────────────▼──────────────────────────────────┐
│              RawAlchemyCpp (动态库)                          │
│  raw_alchemy_core.dll (Windows) / libraw_alchemy.so (Android) │
│  完整管线: decodeRaw → lensCorrection → metering →           │
│           stylize → logTransform → applyLUT → writeJpeg     │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. RAW 文件数据流

### 3.1 Android

```
MediaStore 查询 (Kotlin)
    ↓ MediaItemDto { mediaId, uri: "content://...", mimeType: "image/x-nikon-nef" }

GalleryCard / PreviewWindow
    ↓ getUriForId(mediaId) → "content://..."
    ↓ window.ImageViewerAndroid.resolveFilePath("content://...")
    ↓ ImageViewerActivity.resolveUriToFilePath()
    ↓   → ContentResolver.query(DATA column)
    ↓   → "/storage/emulated/0/DCIM/CameraFTP/IMG_001.NEF"

Frontend → invoke('enqueue_lut_filter', { filePaths, lutId })
    ↓ Tauri IPC

Rust lut_filter::service
    ↓ 调用 RawAlchemyCpp 动态库函数
    ↓ LibRaw fopen(path) → 解码 → 管线处理 → 输出 JPEG
    ↓ 保存到: /storage/emulated/0/DCIM/CameraFTP/LutFilter/
    ↓ JNI 调用 MediaStore 注册新文件 → 刷新 Gallery
```

### 3.2 Windows

```
FileIndex 文件扫描
    ↓ FileInfo { path: "D:\\Photos\\CameraFTP\\IMG_001.NEF" }

GalleryCard / PreviewWindow
    ↓ invoke('enqueue_lut_filter', { filePaths, lutId })

Rust lut_filter::service
    ↓ 调用 RawAlchemyCpp 动态库函数
    ↓ LibRaw fopen(path) → 解码 → 管线处理 → 输出 JPEG
    ↓ 保存到: D:\Photos\CameraFTP\LutFilter\
    ↓ file_index filesystem watcher 自动发现 → Gallery 刷新
```

### 3.3 文件访问可行性

- Android: `resolveFilePath()` 通过 `MediaStore.Images.Media.DATA` 列将 `content://` URI 转为文件系统路径。LibRaw 在同进程内通过 `fopen()` 访问，共享应用的 `READ_MEDIA_IMAGES` 权限。与现有 AI Edit 的 `ImageProcessorBridge.kt` 使用 `File(filePath)` 的模式一致。
- Windows: 直接文件系统访问，无特殊处理。

---

## 4. C API 接口

> **来源**: RawAlchemyCpp 项目 `include/raw_alchemy_capi.h`
> **动态库产出**: `raw_alchemy_core.dll` (Windows) / `libraw_alchemy.so` (Android)

### 4.1 错误码

```c
typedef enum RaResult_ {
    RA_OK                = 0,
    RA_ERR_UNKNOWN       = -1,
    RA_ERR_FILE_NOT_FOUND = -2,
    RA_ERR_DECODE_FAILED  = -3,
    RA_ERR_INVALID_PARAM  = -4,
    RA_ERR_LOG_UNSUPPORTED = -5,
    RA_ERR_LUT_LOAD_FAILED = -6,
    RA_ERR_WRITE_FAILED    = -7,
    RA_ERR_NO_LENS_PROFILE = -8,
    RA_ERR_OUT_OF_MEMORY   = -9,
} RaResult;
```

### 4.2 不透明句柄

```c
typedef struct RaImageBuffer_* RaImageBuffer;

RA_API void RA_CALL raImageBufferDestroy(RaImageBuffer buf);
RA_API int RA_CALL raImageGetWidth(RaImageBuffer buf);
RA_API int RA_CALL raImageGetHeight(RaImageBuffer buf);
RA_API const float* RA_CALL raImageGetData(RaImageBuffer buf);
RA_API int RA_CALL raImageGetDataSizeBytes(RaImageBuffer buf);
```

### 4.3 核心处理函数

**`raProcessFile`** — 完整管线处理并保存到磁盘：

```c
RA_API RaResult RA_CALL raProcessFile(
    const char* inputPath,          // UTF-8 输入 RAW 文件路径
    const char* outputPath,         // UTF-8 输出路径（扩展名决定格式）
    const char* logSpace,           // Log 空间名称，NULL 跳过 Log 变换
    const char* lutPath,            // .cube 文件路径，NULL 跳过 LUT
    const char* metering,           // 测光模式，NULL 使用 "matrix"
    float       manualEv,           // 手动曝光 (EV)，useAutoExposure != 0 时忽略
    int         useAutoExposure,    // 非 0 = 自动测光
    int         jpegQuality,        // JPEG 质量 1-100
    int         enableLensCorrection, // 非 0 = 启用镜头校正
    const char* customLensfunDb     // 自定义 Lensfun DB 路径，NULL 使用默认
);
```

管线: Decode → [Lens Correction] → [Exposure] → [Sat/Cont Boost] → [Log Transform] → [LUT] → Save

**`raProcessToBuffer`** — 完整管线处理返回像素数据（不写磁盘）：

```c
RA_API RaResult RA_CALL raProcessToBuffer(
    const char* inputPath,
    const char* logSpace,
    const char* lutPath,
    const char* metering,
    float       manualEv,
    int         useAutoExposure,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaImageBuffer* outBuf           // 输出，调用者需 raImageBufferDestroy
);
```

### 4.4 工具函数

```c
RA_API const char* RA_CALL raGetLastError(void);  // 线程局部错误信息
RA_API const char* RA_CALL raGetVersion(void);     // 版本字符串 (如 "0.1.0")
```

### 4.5 导出宏

Windows 上使用 `RA_SHARED` 宏声明消费端导入符号：
```c
// 编译时定义 RA_SHARED，触发 __declspec(dllimport)
// 非 Windows 平台无需定义
```

### 4.6 Rust FFI 层

Rust 侧通过 `libloading` crate 在运行时加载动态库：

```rust
// lut_filter/ffi.rs
use libloading::{Library, Symbol};
use std::os::raw::c_char;

#[repr(i32)]
pub enum RaResult {
    Ok = 0,
    ErrUnknown = -1,
    ErrFileNotFound = -2,
    ErrDecodeFailed = -3,
    ErrInvalidParam = -4,
    ErrLogUnsupported = -5,
    ErrLutLoadFailed = -6,
    ErrWriteFailed = -7,
    ErrNoLensProfile = -8,
    ErrOutOfMemory = -9,
}

type RaProcessFileFn = unsafe extern "C" fn(
    *const c_char, *const c_char,     // inputPath, outputPath
    *const c_char, *const c_char,     // logSpace, lutPath
    *const c_char,                     // metering
    f32, c_int,                        // manualEv, useAutoExposure
    c_int,                             // jpegQuality
    c_int,                             // enableLensCorrection
    *const c_char,                     // customLensfunDb
) -> RaResult;

type RaGetLastErrorFn = unsafe extern "C" fn() -> *const c_char;
type RaGetVersionFn = unsafe extern "C" fn() -> *const c_char;

pub struct RawAlchemyLib {
    lib: Library,
    pub process_file: RaProcessFileFn,
    pub get_last_error: RaGetLastErrorFn,
    pub get_version: RaGetVersionFn,
}

impl RawAlchemyLib {
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let lib = unsafe { Library::new(path)? };
        Ok(Self {
            process_file: unsafe { *lib.get(b"raProcessFile")? },
            get_last_error: unsafe { *lib.get(b"raGetLastError")? },
            get_version: unsafe { *lib.get(b"raGetVersion")? },
        })
    }
}
```

**选择运行时加载的理由**：动态库文件名和路径在不同平台不同，运行时加载更灵活；且 `libloading` 是成熟的跨平台方案，避免 build.rs 链接配置的复杂性。

---

## 5. Rust 服务层设计

### 5.1 模块结构

```
src-tauri/src/lut_filter/
├── mod.rs           — 模块入口
├── service.rs       — 队列化 worker（复用 ai_edit/service.rs 模式）
├── ffi.rs           — 动态库加载与 FFI 函数指针
├── presets.rs       — 预制 LUT 注册表（硬编码 LUT 与 Log 空间绑定）
└── resources.rs     — 资源文件管理（LUT + Lensfun DB 释放逻辑）
```

### 5.2 处理队列 worker

复用 AI Edit 的队列化处理模式：

```rust
// lut_filter/service.rs
struct LutFilterService {
    tx: mpsc::Sender<LutFilterTask>,
    handle: Option<JoinHandle<()>>,
}

struct LutFilterTask {
    input_path: PathBuf,
    lut_id: String,
    output_dir: PathBuf,
}
```

Worker 循环：
1. 从队列接收任务
2. 从 `presets.rs` 查找 LUT → 获取 log_space + cube 文件路径
3. 构建管线参数：自动测光 (`useAutoExposure=1`)、镜头校正启用 (`enableLensCorrection=1`)、`customLensfunDb` 指向释放的 Lensfun 数据库路径
4. 调用 `raProcessFile` 动态库函数
5. 检查 `RaResult` 返回值，失败则通过 `raGetLastError()` 获取错误详情
6. 通过 Tauri event 发送进度通知
7. Android: JNI 注册 MediaStore；Windows: file_index 自动发现

### 5.3 预制 LUT 注册表

LUT 文件来源：项目根目录 `F-Log2C_LUT/` 文件夹，所有 LUT 均为 F-Log2C → Fujifilm 胶片模拟的 `.cube` 文件（65grid，约 7.1MB/个）。

构建时将 LUT 文件从 `F-Log2C_LUT/` 复制到 `src-tauri/resources/luts/`（见第 6 节构建流程），运行时通过 `ensure_resources()` 释放到 app 数据目录。

```rust
// lut_filter/presets.rs
pub struct PresetLut {
    pub id: &'static str,
    pub display_name: &'static str,
    pub log_space: &'static str,
    pub cube_filename: &'static str,
}

pub const PRESET_LUTS: &[PresetLut] = &[
    PresetLut { id: "acros",           display_name: "ACROS",           log_space: "F-Log2C", cube_filename: "FLog2C_to_ACROS_65grid_V.1.00.cube" },
    PresetLut { id: "astia",           display_name: "Astia",           log_space: "F-Log2C", cube_filename: "FLog2C_to_ASTIA_65grid_V.1.00.cube" },
    PresetLut { id: "classic-chrome",  display_name: "Classic Chrome",  log_space: "F-Log2C", cube_filename: "FLog2C_to_CLASSIC-CHROME_65grid_V.1.00.cube" },
    PresetLut { id: "classic-neg",     display_name: "Classic Neg",     log_space: "F-Log2C", cube_filename: "FLog2C_to_CLASSIC-Neg._65grid_V.1.00.cube" },
    PresetLut { id: "eterna",          display_name: "ETERNA",          log_space: "F-Log2C", cube_filename: "FLog2C_to_ETERNA_65grid_V.1.00.cube" },
    PresetLut { id: "eterna-bb",       display_name: "ETERNA Bleach Bypass", log_space: "F-Log2C", cube_filename: "FLog2C_to_ETERNA-BB_65grid_V.1.00.cube" },
    PresetLut { id: "pro-neg-std",     display_name: "PRO Neg. Std",    log_space: "F-Log2C", cube_filename: "FLog2C_to_PRO-Neg.Std_65grid_V.1.00.cube" },
    PresetLut { id: "provia",          display_name: "Provia",          log_space: "F-Log2C", cube_filename: "FLog2C_to_PROVIA_65grid_V.1.00.cube" },
    PresetLut { id: "reala-ace",       display_name: "REALA ACE",       log_space: "F-Log2C", cube_filename: "FLog2C_to_REALA-ACE_65grid_V.1.00.cube" },
    PresetLut { id: "velvia",          display_name: "Velvia",          log_space: "F-Log2C", cube_filename: "FLog2C_to_Velvia_65grid_V.1.00.cube" },
    PresetLut { id: "flog2c-709",      display_name: "F-Log2C → Rec.709", log_space: "F-Log2C", cube_filename: "FLog2C_to_FLog2C-709_65grid_V.1.00.cube" },
];
```

> `flog2c-709` 为技术转换 LUT（F-Log2C → Rec.709），非胶片模拟，作为"中性"选项提供。

TypeScript 类型通过 ts-rs 生成。

### 5.4 资源文件管理

资源分为两类：
1. **LUT 文件**（`src-tauri/resources/luts/`）：从项目根目录 `F-Log2C_LUT/` 复制而来
2. **Lensfun 数据库**（`src-tauri/lib/lensfun/data/db/`）：通过 git submodule 引入 lensfun 官方仓库

```rust
// lut_filter/resources.rs
pub fn ensure_resources(app_data_dir: &Path) -> Result<ResourcePaths, AppError>;

pub struct ResourcePaths {
    pub lensfun_db_dir: PathBuf,    // {app_data_dir}/lensfun_db/
    pub lut_presets_dir: PathBuf,   // {app_data_dir}/lut_presets/
}
```

释放策略：
- 检查 `{app_data_dir}/lensfun_db/.version` 标记文件
- 不存在或版本不匹配时，从 bundle resources / assets 复制全部文件
- 避免每次启动都重新释放
- Lensfun DB 仅需复制 `data/db/*.xml` 文件（约 50+ 个 XML），无需 DTD/XSD

### 5.5 Tauri 命令

```rust
// commands/lut_filter.rs

#[command]
pub async fn get_preset_luts() -> Vec<PresetLutInfo>;

#[command]
pub async fn enqueue_lut_filter(
    state: State<'_, LutFilterState>,
    app: AppHandle,
    file_paths: Vec<String>,
    lut_id: String,
) -> Result<(), AppError>;

#[command]
pub async fn is_raw_file(file_path: String) -> bool;
```

注册到 `lib.rs` 的 `invoke_handler`。

---

## 6. 构建系统集成

### 6.1 Git Submodule 引入

两个 git submodule：

```bash
# 在 camera-ftp-companion 仓库根目录执行
git submodule add https://github.com/shenmintao/RawAlchemyCpp.git src-tauri/lib/rawalchemy
git submodule add https://github.com/lensfun/lensfun.git src-tauri/lib/lensfun
git submodule update --init --recursive
```

`.gitmodules`:
```
[submodule "src-tauri/lib/rawalchemy"]
    path = src-tauri/lib/rawalchemy
    url = https://github.com/shenmintao/RawAlchemyCpp.git
[submodule "src-tauri/lib/lensfun"]
    path = src-tauri/lib/lensfun
    url = https://github.com/lensfun/lensfun.git
```

> lensfun 仓库较大（含 docs/build 系统），但我们只需要 `data/db/` 目录的 XML 文件。
> submodule 方式便于后续更新镜头数据库，无需手动维护。

### 6.2 目录结构

```
camera-ftp-companion/
├── F-Log2C_LUT/                      # LUT 源文件（已存在于仓库根目录）
│   ├── FLog2C_to_ACROS_65grid_V.1.00.cube
│   ├── FLog2C_to_ASTIA_65grid_V.1.00.cube
│   ├── FLog2C_to_CLASSIC-CHROME_65grid_V.1.00.cube
│   ├── FLog2C_to_CLASSIC-Neg._65grid_V.1.00.cube
│   ├── FLog2C_to_ETERNA_65grid_V.1.00.cube
│   ├── FLog2C_to_ETERNA-BB_65grid_V.1.00.cube
│   ├── FLog2C_to_FLog2C-709_65grid_V.1.00.cube
│   ├── FLog2C_to_PRO-Neg.Std_65grid_V.1.00.cube
│   ├── FLog2C_to_PROVIA_65grid_V.1.00.cube
│   ├── FLog2C_to_REALA-ACE_65grid_V.1.00.cube
│   └── FLog2C_to_Velvia_65grid_V.1.00.cube
├── src-tauri/
│   ├── lib/
│   │   ├── rawalchemy/               # Git submodule → RawAlchemyCpp
│   │   │   ├── CMakeLists.txt
│   │   │   ├── include/
│   │   │   │   ├── raw_alchemy_capi.h
│   │   │   │   └── raw_alchemy_export.h
│   │   │   ├── src/
│   │   │   ├── third_party/
│   │   │   ├── toolchains/
│   │   │   └── scripts/
│   │   └── lensfun/                  # Git submodule → lensfun/lensfun
│   │       └── data/
│   │           └── db/               # 镜头校正数据库 XML 文件
│   │               ├── mil-fujifilm.xml
│   │               ├── slr-nikon.xml
│   │               ├── ... (50+ XML files)
│   │               └── timestamp.txt
│   ├── resources/                    # 构建时生成（不提交到仓库）
│   │   ├── luts/                     # 从 F-Log2C_LUT/ 复制
│   │   │   ├── FLog2C_to_ACROS_65grid_V.1.00.cube
│   │   │   └── ...
│   │   └── lensfun_db/              # 从 lib/lensfun/data/db/ 复制
│   │       ├── mil-fujifilm.xml
│   │       └── ...
│   ├── build.rs
│   └── ...
├── .gitmodules
└── ...
```

### 6.3 动态库预编译

RawAlchemyCpp 动态库在子模块目录中独立编译，产出物被复制到 camera-ftp-companion 的构建流程中：

**Windows** (在 WSL2 中执行 RawAlchemyCpp 的 Windows 构建脚本):
```bash
# 在 src-tauri/lib/rawalchemy/ 目录下
cmd.exe /C scripts\build_windows.bat Release
# 产出:
#   build-windows-dll/bin/Release/raw_alchemy_core.dll
#   build-windows-dll/lib/Release/raw_alchemy_core.lib
```

**Android** (在 WSL2 中执行):
```bash
# 在 src-tauri/lib/rawalchemy/ 目录下
ANDROID_NDK=$NDK_HOME ./scripts/build_android.sh arm64
# 产出:
#   build-android-arm64/libraw_alchemy.so
```

### 6.4 动态库与资源打包

**Windows**:
- `raw_alchemy_core.dll` 通过 Tauri bundle resources 打包到 exe 旁
- LUT 文件和 Lensfun DB 通过 Tauri bundle resources 打包
- Rust 通过运行时 `libloading::Library::new("raw_alchemy_core.dll")` 加载

```json
// tauri.conf.json 新增
{
  "bundle": {
    "resources": [
      "resources/luts/*.cube",
      "resources/lensfun_db/*.xml",
      "resources/lensfun_db/timestamp.txt",
      "lib/rawalchemy/build-windows-dll/bin/Release/raw_alchemy_core.dll"
    ]
  }
}
```

**Android**:
- `libraw_alchemy.so` 放入 `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/`
- APK 安装后 Android 自动部署 .so，Rust 通过 `libloading::Library::new("libraw_alchemy.so")` 加载
- 资源文件（LUT .cube + Lensfun DB .xml）放入 `src-tauri/gen/android/app/src/main/assets/` 目录
- Android 端通过 Tauri 的 asset 管理或 `context.getAssets()` 访问

**资源文件构建时复制**：
```
build.sh 步骤（资源准备）:
  cp F-Log2C_LUT/*.cube → src-tauri/resources/luts/
  cp src-tauri/lib/lensfun/data/db/*.xml → src-tauri/resources/lensfun_db/
  cp src-tauri/lib/lensfun/data/db/timestamp.txt → src-tauri/resources/lensfun_db/
```

### 6.5 构建流程

构建脚本（`build.sh`）需要新增步骤：资源准备 + 动态库编译 + 复制产出物，在 Rust 编译前完成。

```
./build.sh windows android
    │
    ├── 1. 准备资源文件 (新增步骤)
    │   ├── mkdir -p src-tauri/resources/luts/
    │   ├── cp F-Log2C_LUT/*.cube → src-tauri/resources/luts/
    │   ├── mkdir -p src-tauri/resources/lensfun_db/
    │   └── cp src-tauri/lib/lensfun/data/db/*.xml → src-tauri/resources/lensfun_db/
    │       (同时复制 timestamp.txt)
    │
    ├── 2. 编译 RawAlchemyCpp 动态库 (新增步骤)
    │   ├── Windows: cmd.exe /C scripts\build_windows.bat
    │   └── Android: ANDROID_NDK=... ./scripts/build_android.sh arm64
    │
    ├── 3. 复制动态库产出物到构建目录 (新增步骤)
    │   ├── Windows: .dll → Tauri bundle resources 路径
    │   └── Android: .so → jniLibs/arm64-v8a/
    │       (资源文件同步到 assets/ 目录)
    │
    ├── 4. build-frontend.sh (不变)
    │
    ├── 5. build-windows.sh (不变)
    │   └── cargo.exe build --release
    │       └── build.rs 无需修改（运行时加载）
    │
    └── 6. build-android.sh (不变)
        └── bun run tauri android build --target aarch64
            └── .so 已在 jniLibs/ 中，APK 自动打包
```

### 6.6 build.rs

由于采用运行时加载（`libloading`），`build.rs` 不需要新增链接配置。仅保持现有的 Tauri 构建逻辑不变。

---

## 7. UI 设计

### 7.1 LUT 选择器

采用与 AI 修图对话框（`PromptDialog`）一致的 UI 风格：`Dialog` 容器 + `Select` 下拉选择 + 底部确认/取消按钮。

```
┌─────────────────────────────────────┐
│  LUT 滤镜                           │
│  使用胶片模拟滤镜处理 RAW 照片       │
│  ┌─────────────────────────────┐    │
│  │  滤镜   [Classic Neg    ▾]  │    │
│  └─────────────────────────────┘    │
│                                     │
│              [取消]  [应用]          │
└─────────────────────────────────────┘
```

- 复用现有 `Dialog` 组件（`src/components/ui/Dialog.tsx`）
- 复用现有 `Select` 组件（`src/components/ui/Select.tsx`），LUT 列表作为 `SelectOption[]`
- 选择后点击"应用"开始处理
- 不提供 LUT 缩略图预览
- LUT 列表数据通过 `invoke('get_preset_luts')` 获取，转换为 `SelectOption[]` 格式

### 7.2 集成点

**Gallery 多选模式** (`GalleryCard.tsx` FAB 菜单):
- 在 "修图" 后添加 "LUT 滤镜" 菜单项
- 仅当选中文件包含 RAW 文件时显示（通过 `MediaItemDto.mimeType` 判断 `image/x-*`）
- 点击 → 打开 LutFilterDialog → 选择 LUT → 批量处理

**PreviewWindow 工具栏** (`PreviewWindow.tsx`):
- AI Edit 按钮旁添加 LUT 滤镜按钮（调色板图标）
- 仅当当前图片是 RAW 时显示
- 点击 → 打开同一个 LutFilterDialog → 处理单张

### 7.3 进度显示

复用 `AiEditProgressBar` 的浮动进度条模式：
- 监听 `lut-filter-progress` Tauri 事件
- 事件类型对齐 AI Edit: `progress` / `completed` / `failed` / `done`
- 包含文件名、进度百分比、取消按钮

### 7.4 新增前端文件

```
src/
├── components/
│   └── LutFilterDialog.tsx    # LUT 选择器弹窗
├── hooks/
│   └── useLutFilterProgress.ts # 进度监听 + enqueueLutFilter()
└── types/
    └── index.ts               # 增加 PresetLutInfo 类型导出
```

---

## 8. 输出与刷新

### 8.1 输出路径

| 平台 | 路径 |
|------|------|
| Windows | `{原图目录}\LutFilter\{原名}_{lut_id}_{timestamp}.jpg` |
| Android | `/storage/emulated/0/DCIM/CameraFTP/LutFilter/{原名}_{lut_id}_{timestamp}.jpg` |

### 8.2 Gallery 刷新

| 平台 | 机制 |
|------|------|
| Windows | `file_index` filesystem watcher 自动发现新文件 |
| Android | 处理完成后 JNI 调用 MediaStore API 注册 → 前端刷新 Gallery |

---

## 9. 待办事项

- [x] 确定预制 LUT 列表及对应的 Log 空间映射 → 全部为 F-Log2C，11 个胶片模拟 + 1 个技术转换 LUT
- [x] 确定镜头校正数据库来源 → lensfun 官方仓库 git submodule
- [ ] 验证 Android 上 LibRaw 对文件系统路径的访问权限
- [ ] 验证 Lensfun 数据库在 Android 上的路径解析（通过 `customLensfunDb` 参数）
- [ ] 验证 `libloading` 在 Android Tauri 应用中加载 `libraw_alchemy.so` 的路径
- [ ] 实现构建脚本中 RawAlchemyCpp 动态库的预编译和复制步骤
- [ ] 确定是否需要精简 Lensfun DB（仅保留常用品牌如 Fujifilm/Nikon/Canon/Sony 的 XML）

---

## 10. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| Android scoped storage 文件访问受限 | 沿用现有 AI Edit 的文件路径解析链路，已验证可行 |
| `libloading` 在 Android 上加载 .so 路径不确定 | Android 自动部署 jniLibs/ 中的 .so，通过库名加载即可；需实测验证 |
| Lensfun 数据库路径解析在 Android 上异常 | RawAlchemyCpp 的 `customLensfunDb` 参数支持自定义路径，从 app 私有目录传入 |
| 动态库体积过大影响 APK 大小 | LibRaw + Lensfun + libjpeg-turbo 全部静态链接进 .so，预估增量 5-10MB |
| LUT 文件体积较大（11 个 × 7.1MB ≈ 78MB） | 构建时可按需筛选；运行时释放到 app 数据目录后可被系统清理 |
| lensfun submodule 仓库较大（含完整构建系统） | 仅使用 `data/db/` 目录的 XML 文件；可考虑 sparse-checkout 或构建时脚本提取 |
| RawAlchemyCpp submodule 子依赖初始化不完整 | `build.sh` 中确保 `git submodule update --init --recursive` |
