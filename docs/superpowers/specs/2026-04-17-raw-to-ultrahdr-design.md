# RAW → Ultra HDR 自动转换设计

> CameraFTP - A Cross-platform FTP companion for camera photo transfer
> Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
> SPDX-License-Identifier: AGPL-3.0-or-later

## 概述

在 Android 平台上，将 FTP 接收到的 RAW 图片自动转换为 Ultra HDR 图片。利用 RAW 文件内嵌的全尺寸 JPEG 预览作为 SDR 基底（免重新编码），结合 RAW 解码获得的 HDR 线性数据直接计算增益图(Gain Map)，在 Kotlin 侧组装为 Ultra HDR JPEG 文件。

### 目标平台

- **Android 14+ (API 34)**：Ultra HDR 显示与回放
- **最低构建目标**：与现有项目一致

### 性能目标

- 24MP RAW 文件转换时间：**典型 3–4 秒**，最差 <6 秒
- 内存峰值：<200MB RSS

---

## 增益图计算原理

### 方案：直接比较 + GainMapMin=0

SDR 基底（内嵌 JPEG）经过相机完整 ISP 管线，包含色调曲线的中间调提亮和高光压缩。RAW 解码输出为线性传感器数据。直接比较两者：

```
gain = log2(Y_hdr_linear / Y_sdr_linear)
```

会产生三种情况：

| 区域 | Y_hdr vs Y_sdr | gain | GainMapMin=0 处理后 |
|------|---------------|------|---------------------|
| 中间调（相机提亮） | Y_hdr < Y_sdr | 负 | 截断为 0 → HDR = SDR（不变） |
| 高光（相机压缩） | Y_hdr > Y_sdr | **正** | **保留 → HDR 更亮** ✅ |
| 暗部 | 两者接近 | ≈0 | 不变 |

### 为什么不使用色调曲线推导

色调曲线推导方法（从 HDR/SDR 亮度对拟合相机色调曲线，再应用曲线做同域比较）存在根本性问题：

```
色调曲线方法：gain = curve(Y_hdr) / clamp(curve(Y_hdr), 0, 1)

由于曲线从同一数据推导，对所有观测范围内的 Y_hdr：
  curve(Y_hdr) ∈ [0, 1]
  → clamp 无效 → gain = 1.0 → 增益图全零

高光恢复信号被曲线"吸收"了，因为曲线忠实建模了
相机从 Y_hdr 到 Y_sdr 的映射关系。
```

直接比较不做建模，保留了两个独立来源之间的自然差异（传感器线性响应 vs 相机艺术渲染），这正是 HDR 增益图需要编码的内容。

### 设计约束

- **GainMapMin 必须为 0.0**：截断所有负增益（中间调），确保增益图只添加亮度、不减少亮度。
- 增益图仅计算亮度通道（luminance-only），不修改颜色通道比例，保留相机艺术渲染。
- **单帧 RAW 的 HDR 效果有限**：仅高光区域（Y_hdr > Y_sdr 的像素）获得 HDR 增强。暗部恢复需多帧合并（超出本方案范围）。

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
       │  → 内嵌 JPEG 字节   │              │  → 线性 sRGB 16-bit   │
       │  (SDR 基底，免重编码) │              │                       │
       └──────────┬──────────┘              └───────────┬──────────┘
                  │                                     │
                  │                          ┌──────────▼──────────┐
                  │                          │ 解码内嵌JPEG→线性像素 │
                  │                          │ srgb_to_linear()     │
                  │                          └──────────┬──────────┘
                  │                                     │
                  └──────────────┬──────────────────────┘
                                 │
                      ┌──────────▼──────────┐
                      │   增益图计算          │
                      │   直接比较（同分辨率） │
                      │   gain = Y_hdr/Y_sdr  │
                      │   GainMapMin = 0.0    │
                      │   降采样至 1/4 分辨率  │
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
| ↳ 底层引擎 | **LibRaw** | 0.22+ | LGPL-2.1 / CDDL | ① `unpack_thumb()` — 提取内嵌 JPEG 预览（~0.5s）<br>② `unpack()` — 解包 RAW 传感器数据<br>③ `dcraw_process()` — 反马赛克 + 白平衡 + 色彩校正，输出线性 sRGB |
| JPEG 编解码 | **image** crate | 0.25 | MIT | ① 将内嵌 JPEG 解码为像素值用于增益图计算<br>② 将增益图编码为 JPEG |
| sRGB 转线性 | 自实现 (Rust) | — | AGPL-3.0 | 分段 sRGB gamma 逆函数，用于将 JPEG 8-bit 像素转为线性值 |
| 增益图计算 | 自实现 (Rust) | — | AGPL-3.0 | ① 将 HDR 和 SDR 降采样至增益图分辨率（块平均）<br>② 逐像素计算 log2(Y_hdr/Y_sdr)<br>③ GainMapMin 强制为 0（截断负增益）<br>④ 输出 8-bit 灰度增益图 + 元数据 |
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
参数: gamma=(1,1), no_auto_bright=True, use_camera_wb=True, output_color=sRGB
输出: hdr_data: 16-bit 线性 sRGB 数组（gamma=1，无线性曲线，sRGB 色彩空间基色）
耗时: ~1.5-3.0s
说明: 输出为线性 gamma 但 sRGB 基色空间，确保与内嵌 JPEG 色彩基色一致。
```

### 步骤 4：解码内嵌 JPEG 为线性像素

```
调用: image::load_from_bytes(thumb_buf) → DynamicImage → to_rgba8()
转换: srgb_to_linear(sdr_pixels / 255.0) → sdr_linear: Vec<f32>
耗时: ~0.2-0.5s
```

### 步骤 5：增益图计算（直接比较 + GainMapMin=0）

```
输入:
  hdr_data: 16-bit 线性 sRGB（来自步骤 3）
  sdr_linear: f32 线性 sRGB（来自步骤 4）

1. 对齐：若 HDR 尺寸略大于 SDR → 中心裁剪 HDR 以匹配 SDR

2. 计算两者亮度（BT.709 权重）：
   Y_hdr = 0.2126*R + 0.7152*G + 0.0722*B  (线性，归一化到 [0, 1])
   Y_sdr = 0.2126*R + 0.7152*G + 0.0722*B  (线性，已去 gamma)

3. 降采样至增益图分辨率（1/4，块平均）：
   Y_hdr_down = block_average(Y_hdr, H/4, W/4)
   Y_sdr_down = block_average(Y_sdr, H/4, W/4)

4. 逐像素计算增益（直接比较）：
   gain = (Y_hdr_down + ε_hdr) / (Y_sdr_down + ε_sdr)
   log_g = log2(max(gain, 1e-10))

   增益特性：
     中间调（相机提亮）：Y_hdr < Y_sdr → gain < 1 → log_g < 0 → 截断为 0
     高光（相机压缩）：  Y_hdr > Y_sdr → gain > 1 → log_g > 0 → 保留 ✅
     暗部：              两者接近       → gain ≈ 1 → log_g ≈ 0

5. 统计与编码（GainMapMin 强制为 0）：
   max_log = max(percentile(log_g, 99), 0.01)  // 鲁棒最大值
   recovery = clamp(log_g / max_log, 0.0, 1.0)  // 负值截断为 0
   gainmap[y][x] = round(recovery * 255)

输出:
  gainmap_buf: Vec<u8>       // 8-bit 灰度增益图
  metadata: GainMapMetadata {
    min: 0.0,                  // hdrgm:GainMapMin  ← 强制为 0
    max: max_log,              // hdrgm:GainMapMax
    gamma: 1.0,                // hdrgm:Gamma
    epsilon_sdr: 1/64,         // hdrgm:OffsetSDR
    epsilon_hdr: 1/64,         // hdrgm:OffsetHDR
    capacity_min: 0.0,         // hdrgm:HDRCapacityMin
    capacity_max: max_log,     // hdrgm:HDRCapacityMax
  }
耗时: ~0.3-0.8s
```

### 步骤 6：增益图 JPEG 编码

```
调用: image::codecs::jpeg::JpegEncoder::encode(gray_image, quality=75)
输出: gainmap_jpeg: Vec<u8>  // ~0.5MB
耗时: ~0.1-0.3s
```

### 步骤 7：JNI 桥接

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

### 步骤 8：Kotlin 容器组装

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

### 步骤 9：MediaStore 写入

```
调用: MediaStoreBridge.createEntryNative(...)
文件名: {原文件名去掉扩展名}_UltraHDR.jpg
MIME: image/jpeg
耗时: ~0.2-0.5s
```

### 步骤 10：可选删除原 RAW

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
| RAW 解码+反马赛克 | 1.5–3.0s | LibRaw AHD，NEON 优化，输出线性 sRGB |
| 内嵌 JPEG 解码+sRGB→线性 | 0.2–0.5s | image crate + 分段 gamma 逆函数 |
| 增益图计算 | 0.3–0.8s | 1/4 分辨率降采样 + 逐像素 log2 |
| 增益图 JPEG 编码 | 0.1–0.3s | ~1.5MP 灰度 |
| 临时文件写入 | 0.2–0.3s | 主图 + 增益图 |
| Kotlin 容器组装 | <0.05s | 纯字节操作 |
| MediaStore 写入 | 0.2–0.5s | |
| **总计** | **2.8–6.4s** | **典型 3–5s** |

### 内存峰值分析

| 缓冲区 | 大小 | 存活时段 |
|--------|------|----------|
| 内嵌 JPEG 字节 | ~15–25MB | 步骤 2–7 |
| RAW 16-bit RGB | ~48MB | 步骤 3–5 |
| SDR 线性 f32 | ~96MB | 步骤 4–5 |
| 增益图 8-bit 灰度 | ~1.5MB | 步骤 5–6 |
| **峰值 RSS** | **~190MB** | 步骤 4–5 期间 |

优化：步骤 5 完成后可立即释放 SDR f32 和 RAW 16-bit 缓冲区，峰值降至 ~25MB。

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
RAW 解码 → 线性 sRGB
  → 通用色调映射（Reinhard 曲线）→ SDR 8-bit
  → SDR JPEG 编码（image crate）← 额外耗时 2-4s
  → 增益图计算：gain = log2(Y_hdr_linear / Y_sdr_tonemapped)
     → Reinhard 曲线会压缩高光，高光区域 gain > 0
     → 中间调因 boost 导致负增益，GainMapMin=0 截断
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
├── gainmap.rs              # 增益图计算：直接比较 + GainMapMin=0
├── srgb.rs                 # sRGB gamma 分段逆函数
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
| 色调曲线推导后同域比较 | 从同一 RAW 数据推导色调曲线再应用回同一数据，曲线完美拟合残差为零，增益图全零，高光恢复信号被曲线吸收 |
| 自生成 SDR 基底 | 需重新实现相机 ISP（色调曲线、色彩科学、降噪锐化），且自生成 SDR 与线性 HDR 同源，增益图同样全零；不如直接使用相机已渲染好的内嵌 JPEG |
| 提取相机 EXIF 中的色调曲线数据 | 各厂商色调曲线数据均不完整存储在 RAW 文件中（仅存风格名称/ID），无法完整重建 |
| RGB 三通道增益图 | 与 SDR 基底的色调曲线失配会导致跨通道色调偏移，亮度-only 增益图更安全 |
