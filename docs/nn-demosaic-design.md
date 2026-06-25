# 神经网络去马赛克集成设计方案

将 [x-veon](https://github.com/naorunaoru/x-veon) 神经网络去马赛克模型集成到 CameraFTP 的 RawAlchemyCpp C++ 层。

- **状态**：已定稿
- **目标平台**：Android arm64-v8a (min SDK 35) + Windows x64
- **NN 加速范围**：仅最终全分辨率输出路径；预览与降噪继续使用现有传统算法
- **机型覆盖**：仅 Qualcomm Snapdragon 8 Gen 2 及更新 SoC；非白名单机型自动回退传统方案

---

## 1. 背景与目标

CameraFTP 是一个基于 Tauri v2 的跨平台相机照片传输工具。现有的 RAW 处理流水线位于自研 C++17 共享库 RawAlchemyCpp 中：LibRaw 解码 RAW → CFA mosaic buffer（C++ 堆内）→ 传统 demosaic（RCD / Markesteijn）→ 小波降噪 → LUT/镜头校正 → JPEG/TIFF 输出。

本方案引入 x-veon 神经网络 demosaic 模型作为**最终全分辨率输出路径**的去马赛克引擎，替代该路径上的传统 demosaic 步骤。NN 不参与预览（预览仍走传统算法，保证响应性），不替代降噪（降噪继续走现有小波算法）。

设计原则：
- **不引入额外兜底机制**：现有传统 demosaic 与降噪代码全部保留编译，作为预览路径与 NN 不可用时的回退。
- **极简可靠性机制**：无校准、无 PSNR 门控、无启发式检查、无遥测。仅保留"QNN init 失败 → 永久走传统"与"NaN/Inf 检测 → 报错"两条。
- **窄口径机型支持**：仅白名单 Qualcomm SoC 启用 NN，其余机型直接走传统方案，不引入跨厂商 NNAPI 复杂度。
- **不暴露新 FFI**：复用现有 `raProcessFileWithLUT` 全尺寸一条龙接口，新增一个参数切换 NN/传统路径。

---

## 2. 模型契约（x-veon）

### 2.1 双模型

| 模型 | 文件 | 参数量 | CFA 覆盖 | base_width | 验证 PSNR |
|---|---|---|---|---|---|
| Bayer | `bayer.onnx` | 1,945,717 (~1.95M) | 2×2 RGGB | 16 | 46.04 dB |
| X-Trans | `xtrans.onnx` | 7,769,301 (~7.77M) | 6×6 X-Trans | 32 | 45.78 dB |

两个模型共享同一 `XTransUNet` 拓扑（仅 base_width 不同），按 LibRaw 检测的 `raw_pattern` shape 选择：`(2,2)` → Bayer，`(6,6)` → X-Trans。

### 2.2 ONNX I/O 契约（静态 288×288，opset 17）

| | 名称 | Shape | Dtype | 含义 |
|---|---|---|---|---|
| **Input 0** | `input` | `[1, 4, 288, 288]` | float32 | `[CFA灰度, R-mask, G-mask, B-mask]` planar NCHW |
| **Output 0** | `output` | `[1, 3, 288, 288]` | float32 | 线性白平衡后的 camRGB |

**关键设计点**：残差 CFA skip **在图内**（`output = broadcast(CFA) + Conv1×1(decoder_out)`），输出是绝对 RGB 而非 delta，**无需 `match_gain` 后处理**（对比 RawNIND 需要 10⁶× 缩放补偿）。激活有界，fp16 数值安全（作者验证 PSNR ≥ 35dB）。

ONNX op 全部为标准算子，QNN HTP FP16 与 DirectML 全覆盖：`Conv` / `ConvTranspose` / `MaxPool` / `Relu` / `Concat` / `BatchNormalization`（eval 模式已 bake） / `Add`。无 exotic op。

### 2.3 预处理流程（严格按 x-veon 运行时）

1. **Crop 到 active area**：应用 sensor crop margins。
2. **检测 CFA 相位偏移 `(dy, dx)`**：匹配规范 RGGB（周期 2）或 X-Trans 6×6 模式。非规范相位（BGGR/GRBG/GBRG 等）通过后续 mirror-pad 对齐到规范原点。
3. **归一化**：`(raw - blackLevels[0]) / (whiteLevels[0] - blackLevels[0])`，**单一**黑白点（非 per-site）。
4. **高光重建（WB 之前）**：inpaint-opposed 算法（移植自 darktable），clip 因子 ×0.93。
5. **per-site WB**：`cfa[y,x] *= wb[ch]`，其中 `ch = pattern[(y+dy)%period][(x+dx)%period]`，WB 倍率 G 归一化为 1。
6. **mirror-pad top/left `dy, dx`**：对齐到规范 CFA 相位。
7. **Tile 网格**：patch=288, overlap=48, stride=240。图像整体 mirror-pad 以使整数 tile 网格完整覆盖。
8. **构造 tile 输入**：规范 mask one-hot（R/G/B）+ CFA tile，concat 成 `Float32Array(4 × 288 × 288)` planar NCHW。

### 2.4 后处理流程

1. **Tile 混合**：梯形线性 ramp 加权平均。
   - 1D 窗口：`w[i] = i / overlap`（0→1 over 48px），中间平 1，对称。
   - 2D 权重：`w = wy * wx`。
   - 累积：`output += tile * w; weights += w;`，最终 `output /= weights`。
2. **Crop padding**：去掉步骤 6 的 `(dy, dx)` 与 tile 网格 padding，NCHW → HWC。
3. **camRGB → sRGB 矩阵**：`M = inv(xyzToCam @ inv(XYZ_TO_SRGB))`，`xyzToCam` 行归一化。
4. **低端 clamp**：`max(0, …)`。高端不 clamp（保 HDR 高光），不应用 gamma。

### 2.5 Tile 参数

| 参数 | 值 | 备注 |
|---|---|---|
| `PATCH_SIZE` | 288 | 静态，模型 bake 死 |
| `OVERLAP` | 48 | 大于 X-Trans 周期 6 与有效感受野，接缝安全 |
| stride | 240 | = patch − overlap |
| 对齐周期 | 48 (X-Trans) / 16 (Bayer) | CFA 周期与 U-Net 5 级下采样（÷32）的 lcm |
| 混合方式 | 梯形线性 ramp 加权平均 | 比 darktable 的 h/v-strip 简单 |

---

## 3. 推理引擎

### 3.1 平台覆盖

| 平台 | 执行提供者（EP） | 备注 |
|---|---|---|
| **Android arm64-v8a** (min SDK 35) | **QNN HTP FP16** | 仅白名单机型；非白名单 → 传统 demosaic，不初始化 NN |
| **Windows x64** | **DirectML** | app-local DLL，覆盖 NVIDIA / AMD / Intel 全部 DX12 GPU |

不使用 NNAPI（即便 Qualcomm SoC 上 NNAPI 也能工作）。QNN-direct 比 NNAPI-wrapped 性能高 10–100×（Google 官方 LiteRT QNN Accelerator 数据），且无静默 CPU 降级，且 vendor 自带的 `libQnnHtp.so` 隔离 OEM HAL 质量差异。窄口径策略使跨厂商抽象层（NNAPI）的复杂度不必要。

### 3.2 Android QNN HTP 配置

#### 白名单（Kotlin 启动期检测 `Build.SOC_MODEL`）

| `Build.SOC_MODEL` | SoC | Hexagon 架构 | 支持 FP16-on-HTP |
|---|---|---|---|
| `SM8550` | Snapdragon 8 Gen 2 | v73 | ✅ |
| `SM8650` | Snapdragon 8 Gen 3 | v75 | ✅ |
| `SM8750` | Snapdragon 8 Elite | v79 | ✅ |
| `SM8845` | Snapdragon 8 Elite Gen 4 | v81 | ✅ |
| `SM8850` | Snapdragon 8 Elite Gen 5 | v81 | ✅ |

**不在白名单 → Rust 侧不调用 `ra_demosaic_nn_init`，全场景（预览 + 最终）走传统 demosaic。**

> ⚠️ SD 8 Gen 1 (SM8450, Hexagon v69) 及更早型号不支持 FP16-on-HTP（强制 INT8 量化，色彩关键场景不应冒精度风险），故不在白名单内。这些机型走传统方案。

#### QNN Session 参数

- `enable_htp_fp16_precision = "1"`：喂 fp32 tensor，Hexagon 内部 fp16 math，返回 fp32。
- `disable_cpu_ep_fallback = "1"`：部分 op 无法 offload 时显式报错，不静默降级慢 CPU。
- **离线 context binary**：构建期用 `qnn-context-binary-generator`（x64 主机，`--config_file` 指定 `soc_model`）为每个目标 SoC 生成 `.serialized.bin`，作为 Android asset 打包。运行时 `retrieve_context` 加载，约 180ms（vs 在线编译 3–5s，且推理快 3–5×）。

#### Vendor 的 QNN runtime 文件（全部代际，进 `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/`）

```
libonnxruntime.so               (ORT 核心，约 20MB)
libQnnHtp.so                    (HTP 通用 dispatcher)
libQnnHtpV73Skel.so             (SD 8 Gen 2)
libQnnHtpV75Skel.so             (SD 8 Gen 3)
libQnnHtpV79Skel.so             (SD 8 Elite)
libQnnHtpV81Skel.so             (SD 8 Elite Gen 4/5)
libQnnSystem.so                 (元数据/context 检索)
libQnnHtpPrepare.so             (设备端图准备)
```

来源：Maven `com.qualcomm.qti:qnn-runtime:2.34.0`，公开可分发（发布前复核 QAIRT EULA 文本）。APK 净增约 15–30MB。

#### Kotlin 加载顺序（`MainActivity.onCreate`）

```kotlin
companion object {
    private const val TAG = "MainActivity"
    private val HEXAGON_V73_PLUS_WHITELIST = setOf(
        "SM8550", "SM8650", "SM8750", "SM8845", "SM8850"
    )
}

private fun detectNnCapability(): NnCapability {
    val socModel = Build.SOC_MODEL
    val enabled = socModel in HEXAGON_V73_PLUS_WHITELIST
    Log.d(TAG, "SoC=$socModel, NN enabled=$enabled")
    return NnCapability(enabled, socModel)
}

// System.loadLibrary 顺序（仅白名单机型执行 QNN 库加载）：
// 1. libonnxruntime.so
// 2. libQnnSystem.so → libQnnHtp.so → 对应代际 Skel
// 3. libraw_alchemy_core.so（依赖前两者）
```

`NnCapability` 通过现有 JS Bridge 传给 Rust，Rust 据此决定是否调用 NN 初始化。

### 3.3 Windows DirectML 配置

- **ORT 版本**：尽可能锁最新的稳定版（撰写时为 1.24.x 系列；发布前到 [onnxruntime.ai](https://onnxruntime.ai/) 与 [NuGet Microsoft.ML.OnnxRuntime.directml](https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime.directml) 复核最新稳定版本号并锁定）。每次升级需回归测试。
- **避开已知坏版本**：ORT 1.19.0–1.20.0 存在 DirectML "Catastrophic failure / Unspecified error" 初始化崩溃（[issue #22815](https://github.com/microsoft/onnxruntime/issues/22815)），永不使用此区间。
- **Vendor app-local `onnxruntime.dll`（约 20MB）+ `DirectML.dll`（约 20MB）**：gzip-embed 进 Rust binary，启动时按现有 `color_grading/ffi.rs:16-184` 的模式（哈希后缀 + `%TEMP%` 解压 + `LoadLibrary`）部署。
- **关键技巧（避开 System32 旧版冲突，[issue #18831](https://github.com/microsoft/onnxruntime/issues/18831)）**：自己 `LoadLibrary("DirectML.dll")` + `DMLCreateDevice` 创建 DML device，然后通过 `OrtSessionOptionsAppendExecutionProvider_DML(sessionOptions, dmlDevice)` 把预创建的 device 交给 ORT，绕开 ORT 自己的 loader。
- **OS 下限**：Windows 10 version 1903 (build 18362)。DirectML 自此版本起作为系统组件随附。
- 不加 CUDA（DirectML 一包覆盖三厂 GPU，已达 CUDA 性能的 80–90%；CUDA 会引入 ~1GB cuDNN 包袱与 vendor lock）。

### 3.4 EP 注册顺序

**Android**：
```
1. QNN HTP FP16 (enable_htp_fp16_precision="1", disable_cpu_ep_fallback="1", 加载离线 context binary)
2. [无中间兜底] — QNN init 失败 → 该 session 标记不可用，永久走传统 demosaic
```

**Windows**：
```
1. DirectML (device index 0)
2. [无中间兜底] — DirectML init 失败 → 永久走传统 demosaic
```

无 NNAPI、无 XNNPACK 中间层。NN 不可用时直接走传统，不存在"用 CPU 跑 NN 兜底"的路径。

---

## 4. C++ 集成（RawAlchemyCpp）

### 4.1 文件结构

```
src-tauri/lib/rawalchemy/src/
├── demosaic_dispatch.cpp        ← 现有空 seam，改为运行期路由入口
├── demosaic_rcd.cpp             ← 保留编译（预览路径 + 最终兜底）
├── demosaic_markesteijn.cpp     ← 保留编译（预览路径 + 最终兜底）
├── demosaic_nn_xveon.cpp        ← 新增（继承 -ffast-math）：NN 推理 + 预处理 + tile 混合
├── nn_nan_guard.cpp             ← 新增（编译时关 -ffast-math）：NaN/Inf 扫描
└── ...
```

### 4.2 Session 配置（per oracle 评审）

- ORT session `intra_op_num_threads = 1`，`inter_op_num_threads = 1`，关闭 spin-wait。
- QNN EP 内部已并行；上层用 `#pragma omp parallel for` 并行跨 tile 推理（OpenMP 已在双平台链接）。
- 单例 session 跨图复用，避免重复加载 context binary（首次约 180ms，后续零开销）。
- `demosaic_nn_xveon.cpp` 继承核心 `-ffast-math` 享受优化；`nn_nan_guard.cpp` 编译时**关闭 `-ffast-math`**（否则优化器会删除 `isnan()`/`isinf()` 守卫，使 NaN 检测失效）。

### 4.3 FFI 接口（复用现有，不新增）

**不暴露新接口**。在现有 `raProcessFileWithLUT` 全尺寸一条龙接口上新增一个参数 `enableNnDemosaic`，与现有 `enableLensCorrection` 风格一致（`int` 作 bool）。

更新后的签名（[`include/raw_alchemy_capi.h`](../src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h) 与 [`src/raw_alchemy_capi.cpp`](../src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp)）：

```c
/**
 * 与 raProcessFileWithLUT 相同，新增 enableNnDemosaic 控制去马赛克算法选择。
 *
 * @param enableNnDemosaic  0 = 传统 demosaic（RCD/Markesteijn）；
 *                          非 0 = NN demosaic（x-veon）。NN 未初始化或不可用时
 *                          返回错误码（不静默回退，由调用方决策）。
 * @return RA_OK on success. NN 推理产出 NaN/Inf 时返回专门的错误码。 */
RA_API RaResult RA_CALL raProcessFileWithLUT(
    const char* inputPath,
    const char* outputPath,
    const char* logSpace,
    const float* lutTable,
    int         lutSize,
    const float* lutDomainMin,
    const float* lutDomainMax,
    const char* metering,
    float       evOffset,
    int         jpegQuality,
    int         enableLensCorrection,
    const char* customLensfunDb,
    int         enableNnDemosaic   /* ← 新增参数 */
);
```

Rust 侧 [`color_grading/ffi.rs`](../src-tauri/src/color_grading/ffi.rs) 的 `RaProcessFileWithLUTFn` 类型与 libloading 调用同步更新。`raProcessFile`（无 LUT 版本）同样加该参数，保持一致。

调用方（Rust service / Tauri command）根据 Kotlin 传来的 `nnEnabled` 决定传 0 还是非 0。NN 接口返回错误时，调用方负责决策是否再用 `enableNnDemosaic=0` 重试一次（即"上层回退"），C++ 层不自动回退。

### 4.4 运行期路由（`demosaic_dispatch.cpp`）

```cpp
DemosaicResult demosaic_dispatch(Context* ctx, bool enable_nn) {
    if (ctx->path == PREVIEW) {
        return classical_demosaic(ctx);   // 预览永远走传统
    }
    // FINAL_FULL_RES
    if (enable_nn && ctx->nn_session_available) {
        auto result = nn_demosaic_xveon(ctx);
        // NaN/Inf 由 nn_nan_guard 检测，检测到时直接报错（不回退）
        if (result.has_nan_inf) {
            return DemosaicResult::Error(RA_ERR_NN_NAN_OUTPUT);
        }
        return result;
    }
    return classical_demosaic(ctx);   // NN 未启用或不可用 → 传统
}
```

### 4.5 NaN/Inf 守卫（`nn_nan_guard.cpp`）

```cpp
// 编译时关闭 -ffast-math（CMakeLists 单独 target 属性）
bool nn_output_has_nan_inf(const float* data, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        if (std::isnan(data[i]) || std::isinf(data[i])) return true;
    }
    return false;
}
```

输出 tensor 一遍扫描，发现 NaN/Inf → 上报错误码，**不回退传统**（按需求决策，便于调试期暴露问题）。

---

## 5. 模型资源化

### 5.1 目录布局

```
src-tauri/resources/models/xveon/
├── bayer.onnx           (~4MB, fp16, opset 17)
├── xtrans.onnx          (~15.5MB, fp16, opset 17)
└── context_binaries/    (Android only)
    ├── sm8550.serialized.bin
    ├── sm8650.serialized.bin
    ├── sm8750.serialized.bin
    └── sm8845_8850.serialized.bin
```

### 5.2 打包与提取

- 复用 `scripts/build.sh` 的 `prepare_lut_resources()` 模式打包进 `resources/`。
- 运行时提取到与 Lensfun DB 同目录（参考 `color_grading/resources.rs` 的现有实现）。
- **离线优先**：相机传输场景不能假设联网，模型与 context binary 全部 vendor，不走 darktable 式按需下载。
- Windows 不需要 context binary（DirectML 在线编译，开销可接受）。

---

## 6. 可靠性机制（极简）

按设计原则，仅保留两条机制，无校准、无遥测、无启发式检查：

### 6.1 QNN/DirectML init 失败 → 永久走传统

- session 创建失败（`libQnnHtp.so` 加载失败、DSP domain 异常、context binary 损坏 / `DirectML.dll` 缺失、GPU 不支持）→ 标记 `nn_session_available = false`，全场景走传统 demosaic。
- 结果缓存在内存，本次进程生命周期内不重试。

### 6.2 每次推理 NaN/Inf 扫描 → 直接报错

- `nn_nan_guard.cpp`（关 `-ffast-math` 编译）扫输出 tensor。
- 发现 NaN/Inf → 返回错误码 `RA_ERR_NN_NAN_OUTPUT`，**不回退传统**（按需求决策）。
- 由调用方决定后续动作（如提示用户、重试、或用传统路径再跑一次）。

### 6.3 不包含的机制（明确说明）

- ❌ 校准 + PSNR 门控（窄口径 QNN-direct 已足够可靠，无需预校准）
- ❌ 输出合理性启发式（均值/方差/通道坍缩检查）
- ❌ wall-clock 监控触发回退
- ❌ 遥测上报（无 `ep_registered`、`output_has_nan_inf`、`wall_clock_ms` 等字段采集）
- ❌ XNNPACK 中间兜底层（NN 失败直接走传统，不经 CPU 跑 NN）

---

## 7. 完整工作流

| 路径 | 白名单机型 | 非白名单机型 |
|---|---|---|
| **实时预览** | 传统 RCD / Markesteijn | 传统 RCD / Markesteijn |
| **最终全分辨率** | NN QNN HTP FP16 → NaN 检查 → 通过则输出 | 传统 RCD / Markesteijn |
| **最终 NN 失败（init）** | 永久走传统（全场景） | N/A |
| **最终 NN 失败（NaN）** | 直接报错（不回退） | N/A |
| **降噪（所有路径）** | 现有小波降噪（保留） | 现有小波降噪（保留） |

降噪与 NN demosaic 是独立步骤：NN demosaic 输出 camRGB→sRGB 后，可继续接入现有小波降噪管线，或按现有流水线顺序处理。本方案不修改降噪路径。

---

## 8. 实施步骤

1. **vendor 依赖**
   - CMakeLists 新增 ExternalProject 下载：
     - Windows：`onnxruntime-win-x64-<latest-stable>.zip` + `DirectML.dll`（从 [NuGet Microsoft.AI.DirectML](https://www.nuget.org/packages/Microsoft.AI.DirectML)）
     - Android：`onnxruntime-android-aarch64-<latest-stable>.aar` + Maven `com.qualcomm.qti:qnn-runtime:2.34.0`
   - 构建期生成离线 context binary：x64 主机跑 `qnn-context-binary-generator` 对两个 `.onnx` 按 `soc_model` 各生成一份。
   - 两个 `.onnx` 与 context binary 进 `src-tauri/resources/models/xveon/`。

2. **`demosaic_nn_xveon.cpp`**
   - 单例 ORT session（按平台注册 QNN HTP / DirectML EP）。
   - 完整预处理：相位检测 → 归一化 → 高光重建 → per-site WB → mirror-pad → tile。
   - U-Net 推理（OpenMP 并行跨 tile）。
   - 梯形 ramp tile 混合 → camRGB→sRGB 矩阵 → 低端 clamp。

3. **`nn_nan_guard.cpp`**
   - NaN/Inf 扫描函数，编译时关 `-ffast-math`（CMakeLists 单独 `set_target_properties` 覆盖 `INTERPROCEDURAL_OPTIMIZATION` 与 fast-math 标志）。

4. **`demosaic_dispatch.cpp`**
   - 改为运行期路由：PREVIEW → 传统；FINAL + enable_nn → NN，NaN → 错误码。

5. **FFI**
   - `raProcessFileWithLUT` / `raProcessFile` 新增 `enableNnDemosaic` 参数（头文件 + 实现 + `.def` 导出 + Rust libloading 签名）。
   - 新增错误码 `RA_ERR_NN_NOT_INITIALIZED`、`RA_ERR_NN_NAN_OUTPUT`、`RA_ERR_NN_INFERENCE_FAILED`。

6. **Kotlin 白名单检测**
   - `MainActivity.onCreate` 检测 `Build.SOC_MODEL`，通过现有 JS Bridge 传 Rust（参考 `ImageProcessorBridge` 模式）。
   - 仅白名单机型执行 `System.loadLibrary("onnxruntime")` + `System.loadLibrary("QnnHtp")` 等。

7. **资源化打包**
   - `build.sh` 新增 `prepare_nn_resources()`（仿 `prepare_lut_resources()`）。
   - 模型与 context binary 进 `resources/`，运行时提取到 Lensfun DB 同目录。

8. **测试 gate**
   - **tile 接缝回归测试**：合成平滑梯度 CFA 图，对 tile 边界区域用比图像其余部分更严的方差阈值检测网格伪影。
   - **多真机加载稳定性测试**：SD 8 Gen 2 / Gen 3 / Elite 各一台真机，确认 QNN init 成功率与首图 wall-clock 合理。
   - **NaN 错误验证**：故意喂异常输入（如全零 CFA、极端值），确认返回 `RA_ERR_NN_NAN_OUTPUT` 而非崩溃或静默错结果。
   - **回退验证**：非白名单机型 / QNN init 失败时，确认全场景走传统 demosaic。

---

## 9. 版本与构建集成

按 [`AGENTS.md`](../AGENTS.md) 的版本仪式，本次集成涉及：

- `package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` / `README.md` 版本号同步（建议次版本号 +1，标记新功能）。
- 每个改动后用 `./build.sh windows android` 验证双平台编译。
- ORT 与 QNN runtime 版本在 CMakeLists 中显式 pin，升级需回归测试。

---

## 10. 决策记录（关键权衡）

| 决策点 | 选择 | 理由 |
|---|---|---|
| 模型选择 | x-veon（弃 RawNIND） | x-veon 双 CFA 统一 demosaic（含 X-Trans），fp16 全模型可用（QNN HTP 可加速），残差设计无需 `match_gain` |
| 加速范围 | 仅最终全分辨率输出 | 预览需亚秒响应，CPU/GPU NN 都做不到，预览走传统 |
| Android EP | QNN HTP only（窄口径） | QNN-direct 比 NNAPI 快 10–100×、无静默 CPU 降级、OEM 无关；窄口径避免跨厂商复杂度 |
| 机型支持 | 仅 SD 8 Gen 2+ 白名单 | SD 8 Gen 1 及更早不支持 FP16-on-HTP；其余厂商 NDA/驱动问题；非白名单走传统 |
| Windows EP | DirectML | 一包覆盖 NVIDIA/AMD/Intel，已达 CUDA 80–90% 性能，无 cuDNN 包袱 |
| 精度 | fp16 | x-veon fp16 PSNR 46dB，已超所有人眼/8-bit/16-bit 输出上限 |
| 可靠性机制 | 仅 init 失败回退 + NaN 报错 | QNN-direct + 窄口径 + bounded activation 下，重型守卫属过度防御 |
| 兜底语义 | NN 失败 → 传统（非 XNNPACK-NN） | 传统算法已保留，无校验需求时无需中间 NN 兜底 |
| NaN 失败行为 | 直接报错（不回退） | 调试期暴露问题优先；调用方可决定是否再走传统 |
| FFI | 复用 `raProcessFileWithLUT` 加参数 | 不新增接口，与 `enableLensCorrection` 风格一致 |
| 遥测 | 不提供 | 极简原则 |

---

## 11. 参考资料

- x-veon 仓库：<https://github.com/naorunaoru/x-veon>
- ONNX Runtime：<https://onnxruntime.ai/>
- QNN HTP FP16（Hexagon v73+）：<https://docs.qualcomm.com/doc/80-63442-10/topic/htp_backend.html>
- QNN runtime Maven：<https://developers.google.com/edge/litert/android/npu/qualcomm>
- DirectML EP：<https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html>
- DirectML System32 冲突修复：<https://github.com/microsoft/onnxruntime/issues/18831>
- DirectML 坏版本（1.19–1.20.0）：<https://github.com/microsoft/onnxruntime/issues/22815>
- `disable_cpu_ep_fallback`：<https://github.com/microsoft/onnxruntime/pull/16016>
- 作者的 Rust 传统 demosaic crate（Apache-2.0，可选参考）：<https://github.com/naorunaoru/demosaic>
