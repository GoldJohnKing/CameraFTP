# RAW → Ultra HDR 自动转换设计

> CameraFTP - A Cross-platform FTP companion for camera photo transfer
> Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
> SPDX-License-Identifier: AGPL-3.0-or-later

## 概述

在 Android 平台上，将 FTP 接收到的 RAW 图片自动转换为 Ultra HDR 图片。利用 RAW 文件内嵌的全尺寸 JPEG 预览作为 SDR 基底（免重新编码），结合 RAW 解码获得的 HDR 线性数据计算增益图(Gain Map)，在 Kotlin 侧组装为 Ultra HDR JPEG 文件。

### 目标平台

- **Android 14+ (API 34)**：Ultra HDR 显示与回放
- **最低构建目标**：与现有项目一致

### 性能目标

- 24MP RAW 文件转换时间：**典型 3–4 秒**，最差 <6 秒
- 内存峰值：<200MB RSS

---

## 架构总览

```
                         ┌─────────────────────────────────┐
                         │        RAW 文件 (FTP 接收)        │
                         └───────────┬─────────────────────┘
                                     │
                          ┌──────────▼──────────┐
                          │    RAW 类型检测       │  扩展名/魔数判断
                          │ (非RAW → 跳过)       │
                          └──────────┬──────────┘
                                     │
                      ┌──────────────▼──────────────┐
                      │         并行双路处理           │
                      └──────────────┬──────────────┘
                  ┌──────────────────┴──────────────────┐
                  │                                     │
       ┌──────────▼──────────┐              ┌───────────▼──────────┐
       │  快速路径：提取预览   │              │  HDR路径：RAW解码      │
       │  LibRaw unpack_thumb│              │  LibRaw unpack+process│
       │  → 内嵌 JPEG 字节   │              │  → 线性 f32 HDR 数据   │
       │  (SDR 基底，免重编码) │              │                       │
       └──────────┬──────────┘              └───────────┬──────────┘
                  │                                     │
                  │                          ┌──────────▼──────────┐
                  │                          │ 色彩变换 → BT.2020   │
                  │                          └──────────┬──────────┘
                  │                                     │
                  │                          ┌──────────▼──────────┐
                  │                          │ 解码内嵌JPEG→SDR像素 │
                  │                          │ (用于增益图比值计算)  │
                  │                          └──────────┬──────────┘
                  │                                     │
                  └──────────────┬──────────────────────┘
                                 │
                      ┌──────────▼──────────┐
                      │   增益图计算          │
                      │   log2(HDR/SDR)      │
                      │   1/4 分辨率          │
                      └──────────┬──────────┘
                                 │
                      ┌──────────▼──────────┐
                      │   增益图 JPEG 编码    │
                      │   (image crate, 小图) │
                      └──────────┬──────────┘
                                 │
               ┌─────────────────▼─────────────────┐
               │          JNI 桥接（路径传递）        │
               │  Rust 写临时文件 → JNI 传路径字符串  │
               └─────────────────┬─────────────────┘
                                 │
                      ┌──────────▼──────────┐
                      │  Kotlin 容器组装      │
                      │  (纯字节操作，<50ms)  │
                      │  XMP + MPF + ISO     │
                      └──────────┬──────────┘
                                 │
                      ┌──────────▼──────────┐
                      │  MediaStore 写入     │
                      │  + 可选删除原 RAW    │
                      └─────────────────────┘
```

---

## 开源库与原生函数职责清单

### Rust 侧（Tauri 后端）

| 组件 | 库/API | 版本 | 许可证 | 具体职责 |
|------|--------|------|--------|---------|
| RAW 解码器 | **rsraw** (LibRaw Rust wrapper) | 0.1.1 | MIT | 封装 LibRaw C++ 引擎，提供 Rust FFI 接口 |
| ↳ 底层引擎 | **LibRaw** | 0.22+ | LGPL-2.1 / CDDL | ① `unpack_thumb()` — 提取内嵌 JPEG 预览（~0.5s）<br>② `unpack()` — 解包 RAW 传感器数据<br>③ `dcraw_process()` — 反马赛克 + 白平衡 + 色彩校正，输出线性 RGB |
| JPEG 编解码 | **image** crate | 0.25 | MIT | ① 将内嵌 JPEG 解码为像素值，用于与 HDR 数据比对<br>② 将增益图编码为 JPEG |
| 色彩空间变换 | 自实现 (Rust) | — | AGPL-3.0 | camera RGB → XYZ → BT.2020 线性，使用 LibRaw 输出的颜色矩阵 |
| 增益图计算 | 自实现 (Rust) | — | AGPL-3.0 | ① 将 HDR 和 SDR 降采样至 1/4 分辨率<br>② 逐像素计算 recovery 值<br>③ 输出 8-bit 灰度增益图 + 元数据 |
| EXIF 解析 | **nom-exif** | 2.7 | MIT | 从 RAW 文件中提取 EXIF 元数据（ISO、光圈、快门等），用于 Gallery 展示 |

### Kotlin 侧（Android Bridge）

| 组件 | API | 具体职责 |
|------|-----|---------|
| JNI 桥接 | 自实现 `UltraHdrBridge.kt` | 接收 Rust 侧写入的临时文件路径，读取 JPEG 字节和元数据 |
| Ultra HDR 容器组装 | 自实现 `UltraHdrAssembler.kt` | ① 解析主图 JPEG，分离 EXIF/标记段与熵编码数据<br>② 构造 XMP 标记（GContainer + hdrgm 元数据）<br>③ 构造 APP2 标记（ISO 21496-1 + MPF）<br>④ 拼接：新 SOI + 新标记段 + 主图熵数据 + 增益图段 |
| MediaStore 写入 | **MediaStoreBridge.kt**（现有） | 通过 ContentResolver 写入转换后的 Ultra HDR JPEG |
| RAW 文件删除 | **MediaStoreBridge.kt**（扩展） | 可选：转换成功后删除原 RAW 文件 |

---

## 数据链路（字节级详细流程）

### 步骤 1：RAW 类型检测

```
输入: FTP PUT 事件的文件路径
处理: 检查文件扩展名是否 ∈ {CR2,CR3,NEF,NRW,ARW,SR2,SRF,RAF,ORF,RW2,PEF,DNG,DCR,KDC,3FR,IIQ,ERF,MEF,MRW}
输出: 是 RAW → 进入转换队列；否 → 跳过
```

### 步骤 2：内嵌 JPEG 提取（快速路径）

```
调用: rsraw::LibRaw::open_file(path) → unpack_thumb()
输出: thumb_buf: Vec<u8>   // 原始 JPEG 字节，零修改
耗时: ~0.3-0.5s
降级: 若预览不存在或非全尺寸 → 标记需要完整 SDR 编码
```

### 步骤 3：RAW 解码（HDR 路径）

```
调用: rsraw::LibRaw::open_file(path) → unpack() → dcraw_process()
输出: imgdata.image: 16-bit 线性 RGB 数组
耗时: ~1.5-3.0s
颜色矩阵: imgdata.color.cam_xyz / imgdata.color.cam_rgb
```

### 步骤 4：色彩空间变换

```
输入: 16-bit 线性 camera RGB
变换: camera_matrix × [R,G,B] → CIE XYZ → BT.2020 线性
输出: hdr_linear: Vec<f32>  // BT.2020 线性浮点
耗时: ~0.1-0.3s
```

### 步骤 5：解码内嵌 JPEG 为像素

```
调用: image::load_from_bytes(thumb_buf) → DynamicImage → to_rgba8()
输出: sdr_pixels: Vec<u8>  // 8-bit sRGB RGBA
耗时: ~0.2-0.5s
```

### 步骤 6：增益图计算

```
1. 将 hdr_linear 和 sdr_pixels 降采样至 1/4 分辨率（双线性插值）
2. 逐像素计算:
   Y_hdr  = 0.2627*R + 0.6780*G + 0.0593*B   // BT.2020 亮度权重
   Y_sdr  = 0.2126*R + 0.7152*G + 0.0722*B   // BT.709 亮度权重
   gain   = (Y_hdr + ε_hdr) / (Y_sdr + ε_sdr)
   log_g  = log2(gain)

3. 统计 min/max:
   min_log = min(所有 log_g)
   max_log = max(所有 log_g)

4. 编码:
   recovery = clamp((log_g - min_log) / (max_log - min_log), 0, 1) ^ gamma
   gainmap[y][x] = round(recovery * 255)

输出:
  gainmap_buf: Vec<u8>       // 8-bit 灰度增益图
  metadata: GainMapMetadata {
    min: min_log,             // hdrgm:GainMapMin
    max: max_log,             // hdrgm:GainMapMax
    gamma: 1.0,               // hdrgm:Gamma
    epsilon_sdr: 1/64,        // hdrgm:OffsetSDR
    epsilon_hdr: 1/64,        // hdrgm:OffsetHDR
    capacity_min: 0.0,        // hdrgm:HDRCapacityMin
    capacity_max: max_log,    // hdrgm:HDRCapacityMax
  }
耗时: ~0.2-0.5s
```

### 步骤 7：增益图 JPEG 编码

```
调用: image::codecs::jpeg::JpegEncoder::encode(gray_image, quality=75)
输出: gainmap_jpeg: Vec<u8>  // ~0.5MB
耗时: ~0.1-0.3s
```

### 步骤 8：JNI 桥接

```
Rust 侧:
  1. 将 primary_jpeg (内嵌 JPEG 原始字节) 写入临时文件
  2. 将 gainmap_jpeg 写入临时文件
  3. 构造 JSON 元数据字符串
  4. 通过 JNI 调用 Kotlin 侧方法，传入三个路径字符串

Kotlin 侧:
  1. 读取三个临时文件
  2. 执行容器组装
  3. 写入 MediaStore
  4. 删除临时文件
  5. 返回结果给 Rust
```

### 步骤 9：Kotlin 容器组装

```
输出字节流结构:
  ┌─── FF D8                              ← SOI
  ├─── FF E1 [len] [原 JPEG 的 EXIF]      ← APP1: EXIF（从主图提取）
  ├─── FF E1 [len] XMP                    ← APP1: XMP 增益图元数据
  │    {hdrgm:Version="1.0",
  │     Container:Directory [Primary, GainMap],
  │     hdrgm:GainMapMin/Max, Gamma, OffsetSDR/HDR, HDRCapacityMin/Max}
  ├─── FF E2 [len] ISO 21496-1 ver        ← APP2: ISO 版本标识
  │    "urn:iso:std:iso:ts:21496:-1\0" + min_ver(00 00) + writer_ver(00 00)
  ├─── FF E2 [len] MPF                    ← APP2: 多图容器
  │    "MPF\0" + TIFF IFD + 2×MPEntry
  ├─── [主图熵编码数据 + FF D9]            ← 原 JPEG 的 SOI 之后的数据（含原 EOI）
  ├─── FF D8                              ← 增益图 SOI
  ├─── FF E1 [len] XMP                    ← 增益图的 hdrgm 元数据
  ├─── FF E2 [len] ISO 21496-1 meta       ← 增益图的 ISO 二进制元数据
  └─── [增益图熵编码数据 + FF D9]          ← 增益图 JPEG 的 SOI 之后的数据

耗时: <50ms
```

### 步骤 10：MediaStore 写入

```
调用: MediaStoreBridge.createEntryNative(...)
文件名: {原文件名去掉扩展名}_UltraHDR.jpg
MIME: image/jpeg
耗时: ~0.2-0.5s
```

### 步骤 11：可选删除原 RAW

```
条件: config.ultraHdr.autoDeleteRaw == true 且转换成功
调用: MediaStoreBridge 删除原 RAW 条目
```

---

## 性能预估

**测试基准：** 24MP RAW（6000×4000，如 Sony A7 III），中端设备 Snapdragon 778G (Cortex-A78)

| 阶段 | 耗时 | 备注 |
|------|------|------|
| 内嵌 JPEG 提取 | 0.3–0.5s | LibRaw `unpack_thumb()` |
| RAW 解码+反马赛克 | 1.5–3.0s | LibRaw AHD，NEON 优化 |
| 色彩变换 | 0.1–0.3s | 3×3 矩阵，24M 像素 |
| 内嵌 JPEG 解码 | 0.2–0.5s | image crate |
| 增益图计算 | 0.2–0.5s | 1/4 分辨率，~1.5M 像素 |
| 增益图 JPEG 编码 | 0.1–0.3s | ~1.5MP 灰度 |
| 临时文件写入 | 0.2–0.3s | 主图 + 增益图 |
| Kotlin 容器组装 | <0.05s | 纯字节操作 |
| MediaStore 写入 | 0.2–0.5s | |
| **总计** | **2.8–5.9s** | **典型 3–4s** |

### 内存峰值分析

| 缓冲区 | 大小 | 存活时段 |
|--------|------|----------|
| 内嵌 JPEG 字节 | ~15–25MB | 步骤 2–8 |
| RAW 16-bit RGB | ~48MB | 步骤 3–4 |
| HDR 线性 f32 | ~96MB | 步骤 4–6 |
| SDR 8-bit RGBA | ~24MB | 步骤 5–6 |
| 增益图 8-bit 灰度 | ~1.5MB | 步骤 6–7 |
| **峰值 RSS** | **~190MB** | 步骤 4–5 期间 |

优化：步骤 5 完成后可立即释放 HDR f32 缓冲区，峰值降至 ~120MB。

---

## 降级策略

| 条件 | 降级行为 |
|------|---------|
| 内嵌预览不存在 | 跳过快速路径，执行完整管线：RAW 解码 → Rust 侧色调映射 → image crate 编码 SDR JPEG → 组装 |
| 内嵌预览非全尺寸 | 同上 |
| 内嵌预览色彩空间为 Adobe RGB | Rust 侧转换为 sRGB 后再作为 SDR 基底 |
| RAW 解码失败 | 记录错误日志，跳过该文件，不删除原 RAW |
| 转换超时（>30s） | CancellationToken 取消，保留原 RAW |
| 存储空间不足 | 跳过转换，通过事件通知前端 |

### 完整管线降级（无内嵌预览时）

当内嵌预览不可用时，回退到完整开发管线：

```
RAW 解码 → 线性 f32
  → 色彩变换 → BT.2020
  → 色调映射（Reinhard 或 Hable 曲线）→ SDR 8-bit
  → SDR JPEG 编码（image crate）← 额外耗时 2-4s
  → 增益图计算 + 编码
  → 容器组装
```

预估额外耗时 2–4s，总计 5–8s。

---

## 配置项

在现有 `AppConfig` 中新增字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct UltraHdrConfig {
    /// 总开关
    pub enabled: bool,
    /// FTP 接收后自动触发转换
    pub auto_convert: bool,
    /// 转换完成后自动删除原 RAW 文件
    pub auto_delete_raw: bool,
    /// 增益图 JPEG 编码质量 (0-100)
    pub gainmap_quality: u8,
    /// 增益图降采样倍率 (相对原图)
    pub gainmap_scale: u8,
}

impl Default for UltraHdrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_convert: true,
            auto_delete_raw: false,
            gainmap_quality: 75,
            gainmap_scale: 4,
        }
    }
}
```

---

## 与现有架构的集成点

| 集成点 | 现有模块 | 扩展方式 |
|--------|---------|---------|
| FTP 接收触发 | `ftp/listeners.rs` FtpDataListener | 在 PUT 事件处理中增加 RAW 类型检测和自动入队逻辑，复用 AI Edit 的串行队列模式 |
| 转换服务 | 新建 `src-tauri/src/ultra_hdr/` | 参考 `ai_edit/service.rs` 的双通道队列架构（manual + auto） |
| 配置管理 | `config.rs` AppConfig | 在 AppConfig 中增加 `ultra_hdr: UltraHdrConfig` 字段 |
| Android 桥接 | 新建 `bridges/UltraHdrBridge.kt` | 参考 `ImageProcessorBridge.kt` 的路径传递模式 |
| ProGuard 规则 | `proguard-rules.pro` | 添加 `-keep class com.gjk.cameraftpcompanion.bridges.UltraHdrBridge { *; }` |
| 前端配置 UI | 新建组件 | 在 Config tab 中增加 Ultra HDR 配置面板（参考 AiEditConfigCard） |
| Gallery 索引 | `file_index/service.rs` | 转换完成后将新文件加入索引 |
| Tauri 命令注册 | `src-tauri/src/lib.rs` | 注册新的 Tauri 命令 |

### 新增模块结构

```
src-tauri/src/ultra_hdr/
├── mod.rs                  # 公共导出
├── config.rs               # UltraHdrConfig 结构体
├── service.rs              # 转换服务（双通道队列）
├── processor.rs            # RAW 解码 + 增益图计算管线
├── gainmap.rs              # 增益图计算算法
├── color.rs                # 色彩空间变换
├── types.rs                # GainMapMetadata 等数据类型
└── android_bridge.rs       # JNI 调用 Kotlin 容器组装

src-tauri/gen/android/.../bridges/
└── UltraHdrBridge.kt       # Android 侧容器组装 + MediaStore 写入

src/components/
└── UltraHdrConfigCard.tsx   # 配置面板 UI
```

---

## 文件命名规则

| 原始 RAW 文件 | Ultra HDR 输出 |
|--------------|----------------|
| `IMG_0001.CR3` | `IMG_0001_UltraHDR.jpg` |
| `DSC_0024.NEF` | `DSC_0024_UltraHDR.jpg` |
| `_DSC0001.ARW` | `_DSC0001_UltraHDR.jpg` |

输出文件保存在与原 RAW 文件相同的目录中。

---

## 前端命令清单

| Tauri 命令 | 用途 |
|-----------|------|
| `load_ultra_hdr_config` | 加载 Ultra HDR 配置 |
| `save_ultra_hdr_config` | 保存 Ultra HDR 配置 |
| `trigger_ultra_hdr` | 手动触发单文件转换（含结果回调） |
| `enqueue_ultra_hdr` | 批量入队（多文件，fire-and-forget） |
| `cancel_ultra_hdr` | 取消进行中的转换 |

---

## 事件流

```
UltraHdrProgressEvent:
  - Queued    { file_name: String, position: usize }
  - Progress  { file_name: String, stage: String, percent: u8 }
  - Completed { file_name: String, output_path: String, duration_ms: u64 }
  - Failed    { file_name: String, error: String }
  - Done      { processed: usize, failed: usize }
```

---

## 已排除的方案

| 方案 | 排除原因 |
|------|---------|
| ultrahdr-rs 全 Rust 编码 | zenjpeg 在 ARM64 无 NEON SIMD，24MP JPEG 编码需 2–5s，成为瓶颈 |
| Android `Bitmap.setGainmap()` | 该 API 仅用于 Canvas 内存渲染，`Bitmap.compress()` 不写入 Gain Map，无法生成有效 Ultra HDR 文件 |
| Android `YuvImage.compressToJpegR()` | 需要原始 YUV 像素数据输入，不接受已编码 JPEG，无法用于后组装 |
| Vulkan Compute GPU 加速 | 后台任务场景下，2000+ 行 SPIR-V 的工程复杂度不值得 3–5s 的加速收益 |
| rawler 替代 LibRaw | 不支持内嵌预览提取（`unpack_thumb()`），且反马赛克算法优化程度低于 LibRaw |
