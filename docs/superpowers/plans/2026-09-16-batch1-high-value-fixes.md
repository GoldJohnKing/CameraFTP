# 批次 1：高价值低风险修复 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复经对抗性审查确认的 5 类高价值缺陷：虚拟网格行高、Windows 文件就绪检测、密码保存主线程阻塞、Android 备份面、启动/保存失败无反馈。

**Architecture:** 7 个互相独立的任务。前端 4 个（任务 1、5、6、7）走 vitest TDD；Rust 2 个（任务 2、3）走 cargo TDD；Android 1 个（任务 4）为配置改动走构建验证。每个任务独立可交付、可单独 review。

**Tech Stack:** React 18 + TypeScript(strict) + Zustand + TailwindCSS + vitest（前端）；Rust 2021 + tokio + tauri 2.11.5（后端）；Android Gradle/Kotlin（清单）。

**Spec:** 本对话中"对抗性审查最终核实报告"（2026-09-16）之修正后结论：F1（行高，含 max-w-md 加重）、R1/R4（Windows 就绪检测失效 + 双通道重复解析）、R2（save_auth_config 主线程 Argon2）、A3/A4（默认备份 + cleartext 悬空占位符）、F5/F6/F7/F8（失败无反馈/防重）。

## Global Constraints

- 构建验证一律 `./build.sh windows android`（内含 gen-types、前端构建、双端测试）；禁止裸 `cargo`（必须 `cargo.exe`）、禁止 `bun run build`/`npm`。
- 前端快速测试循环：`bun run test -- <文件名片段>`；Rust 快速循环：`cargo.exe test --lib -- <过滤词>`。
- 新建源文件必须带 SPDX 头：`CameraFTP - A Cross-platform FTP companion for camera photo transfer / Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn> / SPDX-License-Identifier: AGPL-3.0-or-later`（TS 用 `/** */` 块注释，Rust 用 `//`）。
- UI 文案使用简体中文，语气与现有文案一致（"端口 X 已被占用"风格）。
- TypeScript strict；Rust 遵循项目 `Result<T, AppError>` + `tracing` 约定。
- 不做超出本计划范围的"顺手重构"。

---

### Task 1: 虚拟网格动态行距（P0 正确性缺陷）

**背景**：`ROW_HEIGHT = 120` 硬编码（VirtualGalleryGrid.tsx:13），实际行距 = (网格内容宽 − 16)/3 + 6，随容器宽度变化：390dp 视口恰为 120，412dp≈127.33（底部 ~6% 不可达），360dp≈110（spacer 虚高、日期跳转偏移），`max-w-md` 封顶 448dp 时 ≈139.33（底部 ~13.9% 不可达）。修复方式：实测网格元素样式计算行距，测量失败时回退 120（保证 jsdom 测试行为不变）。

**Files:**
- Create: `src/utils/grid-metrics.ts`
- Create: `src/utils/__tests__/grid-metrics.test.ts`
- Modify: `src/components/VirtualGalleryGrid.tsx`（:12-16 常量区、:70-75 scrollToIndex、:96-97 totalHeight、:190-201 可见范围、:283 offsetY、:292-302 内层网格 div 挂 ref、新增测量 effect）

**Interfaces:**
- Produces: `computeGridMetrics(contentWidth: number, padLeft: number, padRight: number, colGap: number, rowGap: number, padTop: number, padBottom: number): GridMetrics`；`measureGridMetrics(el: HTMLElement): GridMetrics`；`DEFAULT_GRID_METRICS: GridMetrics`；`interface GridMetrics { pitch: number; padTop: number; padBottom: number }`。仅 VirtualGalleryGrid 消费。

- [ ] **Step 1: 写失败测试** — 新建 `src/utils/__tests__/grid-metrics.test.ts`：

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { computeGridMetrics, measureGridMetrics, DEFAULT_GRID_METRICS } from '../grid-metrics';

describe('computeGridMetrics', () => {
  // 网格实际 Tailwind 类：px-0.5(2px×2) gap-1.5(6px) pt-1(4px) pb-1.5(6px)，3 列 aspect-square。
  // 网格内容宽 = min(视口, 448) − 32（GalleryCard px-4）。
  it('390dp 视口（网格宽 358）等于旧行高 120', () => {
    expect(computeGridMetrics(358, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(120, 5);
  });

  it('412dp 视口（网格宽 380）≈127.333', () => {
    expect(computeGridMetrics(380, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(127 + 1 / 3, 2);
  });

  it('360dp 视口（网格宽 328）=110', () => {
    expect(computeGridMetrics(328, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(110, 5);
  });

  it('448dp 封顶（网格宽 416）≈139.333', () => {
    expect(computeGridMetrics(416, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(139 + 1 / 3, 2);
  });

  it('透传上下 padding', () => {
    expect(computeGridMetrics(358, 2, 2, 6, 6, 4, 6)).toEqual({ pitch: 120, padTop: 4, padBottom: 6 });
  });
});

describe('measureGridMetrics', () => {
  it('从内联样式读取（jsdom 可解析内联样式）', () => {
    const el = document.createElement('div');
    el.style.cssText = 'width:380px;padding:4px 2px 6px;column-gap:6px;row-gap:6px';
    expect(measureGridMetrics(el).pitch).toBeCloseTo(127 + 1 / 3, 2);
  });

  it('宽度不可解析（jsdom 无布局/未测量）时回退默认值', () => {
    const el = document.createElement('div');
    expect(measureGridMetrics(el)).toEqual(DEFAULT_GRID_METRICS);
  });
});
```

- [ ] **Step 2: 确认失败** — Run: `bun run test -- grid-metrics`
  Expected: FAIL（`Cannot find module '../grid-metrics'` 或等价的模块解析错误）

- [ ] **Step 3: 实现** — 新建 `src/utils/grid-metrics.ts`：

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export interface GridMetrics {
  /** 行顶到下一行顶的垂直距离（单元格高度 + 行间距），px */
  pitch: number;
  /** 网格内部上 padding（首行偏移），px */
  padTop: number;
  /** 网格内部下 padding，px */
  padBottom: number;
}

/** 测量前的回退值（同时是 jsdom 测试环境下的稳定值，保持旧行为） */
export const DEFAULT_GRID_METRICS: GridMetrics = { pitch: 120, padTop: 4, padBottom: 6 };

/**
 * 3 列 aspect-square 网格的纯几何计算：
 * 单元格宽 = (contentWidth − 左右 padding − 2 × 列间距) / 3
 * pitch    = 单元格高（= 宽） + 行间距
 */
export function computeGridMetrics(
  contentWidth: number,
  padLeft: number,
  padRight: number,
  colGap: number,
  rowGap: number,
  padTop: number,
  padBottom: number,
): GridMetrics {
  const cellWidth = (contentWidth - padLeft - padRight - 2 * colGap) / 3;
  return { pitch: cellWidth + rowGap, padTop, padBottom };
}

/**
 * 从真实 DOM 元素读取计算样式并得出行距。
 * 用 getComputedStyle().width（浏览器返回布局后的 used value），
 * 而非 getBoundingClientRect（jsdom 下恒为 0，内联样式可被 computed style 解析）。
 */
export function measureGridMetrics(el: HTMLElement): GridMetrics {
  const cs = window.getComputedStyle(el);
  const num = (v: string): number => parseFloat(v) || 0;
  const width = num(cs.width);
  if (!(width > 0)) {
    return DEFAULT_GRID_METRICS;
  }
  return computeGridMetrics(
    width,
    num(cs.paddingLeft),
    num(cs.paddingRight),
    num(cs.columnGap),
    num(cs.rowGap),
    num(cs.paddingTop),
    num(cs.paddingBottom),
  );
}
```

- [ ] **Step 4: 确认通过** — Run: `bun run test -- grid-metrics`
  Expected: PASS（8 个用例全绿）

- [ ] **Step 5: 接入组件** — 修改 `src/components/VirtualGalleryGrid.tsx`：

  1. 顶部 import 增加：`import { DEFAULT_GRID_METRICS, measureGridMetrics, type GridMetrics } from '../utils/grid-metrics';`
  2. 删除 `const ROW_HEIGHT = 120;`（:13）。
  3. 组件内新增（放在 `containerRef` 声明之后）：

```tsx
  const innerRef = useRef<HTMLDivElement>(null);
  // 行距来自实测（cell 是 aspect-square，行高随容器宽度变化；曾硬编码 120
  // 导致非 390dp 视口底部不可达/日期跳转偏移）。测量失败回退默认值。
  const [metrics, setMetrics] = useState<GridMetrics>(DEFAULT_GRID_METRICS);
  const metricsRef = useRef<GridMetrics>(DEFAULT_GRID_METRICS);

  useEffect(() => {
    const el = innerRef.current;
    if (!el) return;
    const apply = () => {
      const next = measureGridMetrics(el);
      const drifted =
        Math.abs(next.pitch - metricsRef.current.pitch) > 0.5 ||
        Math.abs(next.padTop - metricsRef.current.padTop) > 0.5 ||
        Math.abs(next.padBottom - metricsRef.current.padBottom) > 0.5;
      if (drifted) {
        metricsRef.current = next;
        setMetrics(next);
      }
    };
    apply();
    const observer = new ResizeObserver(apply);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
```

  4. `scrollToIndex`（:70-75）改为读 ref（imperative handle 不因 metrics 变化重建）：

```tsx
  useImperativeHandle(ref, () => ({
    scrollToIndex(index: number) {
      const row = Math.floor(index / COLUMNS);
      containerRef.current?.scrollTo({ top: metricsRef.current.padTop + row * metricsRef.current.pitch });
    },
  }), [containerRef]);
```

  5. `totalHeight`（:97）：`const totalHeight = metrics.padTop + totalRows * metrics.pitch + metrics.padBottom;`
  6. 可见范围 useMemo（:190-201）：`ROW_HEIGHT` 全部替换为 `metrics.pitch`，依赖数组追加 `metrics`（即 `[scrollTop, containerHeight, totalRows, metrics]`）。
  7. `offsetY`（:283）：`const offsetY = metrics.padTop + startRow * metrics.pitch;`
  8. 内层网格 div（:293，已有 `data-testid="virtual-grid-inner"`）加 `ref={innerRef}`。

- [ ] **Step 6: 回归** — Run: `bun run test -- VirtualGalleryGrid` 再 Run: `bun run test`
  Expected: 全部 PASS（jsdom 下测量回退默认值，既有测试行为不变；若个别测试文件未提供 ResizeObserver mock 而报错，按该文件既有 mock 方式补齐——容器 observer 早已存在，通常 setup 已覆盖）

- [ ] **Step 7: Commit**

```bash
git add src/utils/grid-metrics.ts src/utils/__tests__/grid-metrics.test.ts src/components/VirtualGalleryGrid.tsx
git commit -m "fix(gallery): measure virtual grid row pitch responsively

Hardcoded ROW_HEIGHT=120 only matched 390dp viewports; on 412dp/428dp the
bottom ~6-14% of photos were unreachable and date-jump scrollToIndex drifted
by up to ~27 rows. Pitch is now derived from the rendered grid's computed
styles with a 120px fallback (jsdom-safe)."
```

- [ ] **Step 8: 人工冒烟（Windows）** — `./build.sh windows` 后运行应用：拖拽窗口到不同宽度，确认能滚动到最旧照片、日期跳转高亮脉冲正常。

---

### Task 2: 文件就绪稳定性探测 + 索引查重前置与 EXIF 回填（P0 + P1）

**背景**：`wait_for_file_ready` 用 `File::open` 成功判断就绪，但 Windows 上默认共享模式使其对"写入中"文件也成功（utils/fs.rs:78-93，`WouldBlock` 分支为死代码）；notify 8.x Windows 后端无 Close 事件（已核实）。watcher 在文件写入中途索引 → EXIF 解析失败 → 条目陈旧，且 Put 事件的第二次 `add_file` 因查重在 EXIF 解析之后只做了"丢弃式跳过"。修复三件套：稳定性探测（主修）+ 查重前置（省一次全文件 EXIF 解析）+ 陈旧条目回填（兜底网络抖动导致的假稳定）。

**Files:**
- Modify: `src-tauri/src/utils/fs.rs`（:9-93 重写 `wait_for_file_ready`，删除 `is_file_readable`，新增常量与测试）
- Modify: `src-tauri/src/file_index/service.rs`（:258-305 `add_file`）

**Interfaces:**
- Consumes: 现有 `is_supported_image`、`get_file_info`、`adjust_current_index_after_removal`、`emit_file_index_changed`（service.rs 内既有）。
- Produces: `pub(crate) const FILE_READY_STABLE_WINDOW: Duration`（utils/fs.rs，供测试断言）；`add_file` 对外签名不变。

- [ ] **Step 1: 写失败测试（fs）** — 在 `src-tauri/src/utils/fs.rs` 的 `mod tests` 中追加：

```rust
    #[tokio::test]
    async fn wait_for_file_ready_waits_for_stability_window() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(b"test content").expect("write content");
        file.flush().expect("flush");
        let path = file.path().to_path_buf();

        let start = Instant::now();
        let result = wait_for_file_ready(&path, Duration::from_secs(2)).await;
        assert!(result, "stable file should become ready");
        // 不能"open 成功即返回"：必须等到稳定性窗口结束
        assert!(
            start.elapsed() >= FILE_READY_STABLE_WINDOW,
            "ready only after the stability window, took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn wait_for_file_ready_not_ready_while_file_keeps_growing() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let path = temp_dir.path().join("growing.jpg");
        std::fs::write(&path, b"head").expect("initial write");

        // 模拟持续写入：每 40ms 追加一次共 ~600ms（间隔远小于稳定窗口）
        let writer_path = path.clone();
        let writer = tokio::spawn(async move {
            for i in 0..15u32 {
                tokio::time::sleep(Duration::from_millis(40)).await;
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&writer_path)
                    .expect("open append");
                f.write_all(format!("-chunk{}", i).as_bytes())
                    .expect("append chunk");
            }
        });

        let start = Instant::now();
        let result = wait_for_file_ready(&path, Duration::from_secs(5)).await;
        let elapsed = start.elapsed();
        writer.await.expect("writer task finishes");

        assert!(result, "file eventually becomes stable");
        assert!(
            elapsed >= Duration::from_millis(600),
            "must not report ready while file is growing (took {:?})",
            elapsed
        );
    }
```

- [ ] **Step 2: 确认失败** — Run: `cargo.exe test --lib utils::fs`
  Expected: 两个新用例 FAIL（当前实现在文件存在时可打开时立即返回 true，`elapsed < 稳定窗口`；growing 用例同理提前 ready）

- [ ] **Step 3: 实现（fs）** — 将 `src-tauri/src/utils/fs.rs` 的 :9-93 区段（imports、`wait_for_file_ready`、`is_file_readable`）替换为：

```rust
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, trace};

/// (大小, mtime) 保持不变的持续时长，超过即视为写入完成。
/// 取 200ms：足以覆盖连续写入的分段间隙，又不明显拖慢单个文件的索引时机。
pub(crate) const FILE_READY_STABLE_WINDOW: Duration = Duration::from_millis(200);

/// 等待文件写入完成（大小与修改时间稳定）
///
/// 通过轮询 metadata 的 (len, mtime) 签名判断写入是否结束。
/// 为什么不用 File::open 成功与否判断：Windows 上 std 默认以
/// FILE_SHARE_READ|WRITE|DELETE 打开，写方持有句柄时 open 仍然成功；
/// notify 8.x 的 Windows 后端（ReadDirectoryChangesW）也不派发 Close
/// 事件，因此"稳定性探测"是唯一可靠的写完信号。
pub async fn wait_for_file_ready(path: &Path, max_wait: Duration) -> bool {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(20);

    let mut last_sig: Option<(u64, Option<SystemTime>)> = None;
    let mut last_change = Instant::now();

    while start.elapsed() < max_wait {
        match tokio::fs::metadata(path).await {
            Ok(md) => {
                let sig = (md.len(), md.modified().ok());
                if sig == last_sig {
                    if last_change.elapsed() >= FILE_READY_STABLE_WINDOW {
                        trace!(
                            "File stable after {:?}: {:?}",
                            start.elapsed(),
                            path
                        );
                        return true;
                    }
                } else {
                    last_sig = Some(sig);
                    last_change = Instant::now();
                }
            }
            Err(_) => {
                // 文件尚未创建（或刚被删除）：重置稳定性基准
                last_sig = None;
                last_change = Instant::now();
            }
        }
        tokio::time::sleep(poll_interval).await;
    }

    debug!(
        "Timeout waiting for file ready after {:?}: {:?}",
        start.elapsed(),
        path
    );
    false
}
```

  （`is_path_writable` 与既有测试保持不动。）

- [ ] **Step 4: 确认通过** — Run: `cargo.exe test --lib utils::fs`
  Expected: PASS（新 2 例 + 既有 4 例）

- [ ] **Step 5: 写失败测试（service 回填）** — 在 `src-tauri/src/file_index/service.rs` 的 `mod tests` 中（`add_file_skips_duplicate_path` 之后）追加：

```rust
    #[tokio::test]
    async fn add_file_backfills_exif_for_stale_entry() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(config_path);
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));

        let file_path = save_path.join("backfill.jpg");

        // 1) 无 EXIF 的普通 JPEG 先入索引（模拟"写入中途索引"的陈旧条目）
        let plain = image::RgbImage::from_pixel(2, 2, image::Rgb([64u8, 64, 64]));
        let mut buf: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(plain)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .expect("encode plain jpeg");
        std::fs::write(&file_path, &buf).expect("write plain jpeg");
        // mtime 拨到一年前，确保陈旧条目与回填后的 sort_time 差异明显
        let old_system = std::time::SystemTime::now()
            - std::time::Duration::from_secs(365 * 24 * 3600);
        filetime::set_file_mtime(
            &file_path,
            filetime::FileTime::from_system_time(old_system),
        )
        .expect("set old mtime");

        service.add_file(file_path.clone()).await.expect("first add");
        let files = service.get_files().await;
        assert_eq!(files.len(), 1);
        assert!(files[0].exif_time.is_none(), "plain jpeg entry has no exif");

        // 2) 文件补上 EXIF（等价于完整写入后 Put 事件再次触发 add_file）
        std::fs::write(
            &file_path,
            crate::image_utils::build_exif_jpeg("2024:06:01 12:00:00", 1),
        )
        .expect("overwrite with exif jpeg");

        service.add_file(file_path.clone()).await.expect("second add");

        let files = service.get_files().await;
        assert_eq!(files.len(), 1, "backfill must replace, not duplicate");
        assert!(
            files[0].exif_time.is_some(),
            "stale entry must be backfilled with EXIF time"
        );
    }
```

  说明：`build_exif_jpeg` 为 `image_utils.rs:245` 的 `pub(crate)` 测试辅助（生成真实 EXIF JPEG 字节）；`filetime` 已是 dev-dependency。

- [ ] **Step 6: 确认失败** — Run: `cargo.exe test --lib file_index`
  Expected: 新用例 FAIL（第二次 add_file 被旧查重逻辑跳过，`exif_time` 仍为 None）

- [ ] **Step 7: 实现（service）** — 将 `service.rs:258-305` 的 `add_file` 整体替换为：

```rust
    /// 添加新文件（FTP 上传/watcher 事件调用）
    ///
    /// watcher（Created，经稳定性探测放行）与 FTP Put 监听器（文件完全
    /// 写入后触发）会对同一上传文件各调一次，因此：
    /// 1. EXIF 解析（spawn_blocking 全文件扫描）之前先做廉价查重；
    /// 2. 已存在但缺 EXIF 的陈旧条目（写入中途索引的遗留）重新解析回填。
    pub async fn add_file(&self, path: PathBuf) -> Result<(), AppError> {
        if !crate::image_utils::is_supported_image(&path) {
            return Ok(()); // 跳过非图片文件
        }

        // 廉价查重前置：已索引且不缺 EXIF → 直接返回，避免重复解析
        {
            let index = self.index.read().await;
            if index.contains_path(&path) {
                let needs_backfill = index
                    .files()
                    .iter()
                    .any(|f| f.path == path && f.exif_time.is_none());
                if !needs_backfill {
                    trace!("File already indexed, skipping: {:?}", path);
                    return Ok(());
                }
            }
        }

        let metadata = tokio::fs::metadata(&path).await
            .map_err(|e| AppError::Other(format!("Failed to get metadata: {}", e)))?;

        let file_info = self.get_file_info(&path, &metadata).await?;

        // 原子检查-插入（写锁内防 TOCTOU 竞态）
        let mut index = self.index.write().await;

        if let Some(pos) = index.files().iter().position(|f| f.path == path) {
            let existing_has_exif = index.files()[pos].exif_time.is_some();
            if existing_has_exif || file_info.exif_time.is_none() {
                // 已有条目不劣于新解析结果：跳过（并发重复，或新解析失败）
                trace!("File already indexed, skipping: {:?}", path);
                return Ok(());
            }
            // 回填：移除缺 EXIF 的陈旧条目，让新 file_info 按排序位置重新插入
            let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
            files.remove(pos);
            let new_len = files.len();
            Self::adjust_current_index_after_removal(&mut index.current_index, pos, new_len);
            index.path_set.remove(&path);
            info!("Backfilled EXIF for stale index entry: {:?}", path);
        }

        // Insert into sorted position using copy-on-write (Arc::make_mut)
        {
            let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
            // sort_time 降序，相同则 modified_time 降序（新文件优先）
            let insert_pos = files.iter()
                .position(|f| {
                    f.sort_time < file_info.sort_time ||
                    (f.sort_time == file_info.sort_time && f.modified_time < file_info.modified_time)
                })
                .unwrap_or(files.len());

            files.insert(insert_pos, file_info);

            if let Some(current) = index.current_index {
                if insert_pos <= current {
                    index.current_index = Some(current + 1);
                }
            }
        }

        index.path_set.insert(path.clone());
        drop(index);
        info!("Added file to index: {:?}", path);

        // 发射文件索引变化事件
        self.emit_file_index_changed().await;

        Ok(())
    }
```

- [ ] **Step 8: 确认通过** — Run: `cargo.exe test --lib file_index`
  Expected: PASS（含既有 `add_file_skips_duplicate_path`：两次 add 后仍为 1 条；以及全部 watcher/listeners 相关测试）

- [ ] **Step 9: 全量 Rust 回归 + 双平台构建** — Run: `cargo.exe test --lib`，随后 Run: `./build.sh windows android`
  Expected: 全部 PASS / 构建成功

- [ ] **Step 10: Commit（两笔）**

```bash
git add src-tauri/src/utils/fs.rs
git commit -m "fix(file-index): detect upload completion via size/mtime stability

File::open succeeds on in-flight writes on Windows (default share mode)
and notify's Windows backend emits no Close events, so readiness probing
by openability was a no-op. Poll (len, mtime) signature stability instead."
git add src-tauri/src/file_index/service.rs
git commit -m "perf(file-index): dedup before EXIF parse and backfill stale entries

add_file parsed EXIF (full-file spawn_blocking scan) before the duplicate
check on every FTP upload; entries indexed mid-write kept missing EXIF
forever. Skip duplicates up front and re-parse stale entries to backfill."
```

---

### Task 3: save_auth_config 移出主线程（P0，桌面主线程阻塞 / Android IPC 串行化）

**背景**：非 async 命令在桌面端跑主线程（Tauri v2 语义，已核实）、Android 端跑 WebView 请求线程。Argon2id(m=64MB,t=3,p=4) 桌面实测 ~90ms（旧 CPU 更久）、移动端约 0.5s。FTP 认证路径（ftp/server.rs:71-75）已用 spawn_blocking，此处补齐。

**Files:**
- Modify: `src-tauri/src/commands/config.rs:117-127`

**Interfaces:**
- Consumes: `save_auth_config_with_service(&ConfigService, bool, String, String) -> Result<(), AppError>`（config.rs 既有，签名不变）。
- Produces: `save_auth_config` 变为 async；lib.rs 注册与前端 invoke 调用无需任何改动。

- [ ] **Step 1: 实现** — 将 :117-127 替换为：

```rust
/// 保存认证配置（使用 Argon2id 哈希密码）
#[command]
#[instrument(skip(config_service, password))]
pub async fn save_auth_config(
    config_service: State<'_, Arc<ConfigService>>,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    // Argon2id(m=64MB,t=3,p=4) 是重 CPU 计算：同步命令在桌面端跑在主线程
    // （Tauri v2 语义）会阻塞 UI 事件循环，Android 端跑在 WebView 请求
    // 线程会串行化其它 IPC。与 FTP 认证路径（ftp/server.rs）保持一致，
    // 使用 spawn_blocking。
    let service = Arc::clone(config_service.inner());
    tokio::task::spawn_blocking(move || {
        save_auth_config_with_service(service.as_ref(), anonymous, username, password)
    })
    .await
    .map_err(|e| AppError::Other(format!("save_auth_config worker panicked: {}", e)))?
}
```

- [ ] **Step 2: 适配既有测试** — Run: `grep -n "save_auth_config" src-tauri/src/commands/config.rs src-tauri/src/config.rs`
  若测试代码直接调用 `save_auth_config(...)`（非经 tauri State），将其改为 `#[tokio::test]` + `.await`；若无直接调用则跳过。

- [ ] **Step 3: 验证** — Run: `cargo.exe test --lib commands`，随后 Run: `./build.sh windows android`
  Expected: 测试 PASS / 双平台构建成功

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/config.rs
git commit -m "perf(config): move save_auth_config Argon2 hashing off the main thread

Sync commands run on the main thread on desktop (Tauri v2) and on the
WebView request thread on Android; hash in spawn_blocking like the FTP
auth path already does."
```

---

### Task 4: Android 备份收紧 + cleartext 占位符接线（P1 安全）

**背景**：未设 `allowBackup`/`dataExtractionRules` → 默认全域备份（已核实官方语义：Android 12+ 未定义规则时 cloud backup 与设备迁移均包含 `files/` 域），Rust 侧私有 `config.json`（含 argon2 哈希与火山引擎 API Key）会进云备份。gradle 设置的 `usesCleartextTraffic` 占位符从未被 manifest 引用（死配置）。

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml:43-47`
- Create: `src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml`

**Interfaces:**
- Consumes: `build.gradle.kts:124,152` 既有的 `manifestPlaceholders["usesCleartextTraffic"]`（default "false" / debug "true"）。
- Produces: 无代码接口；行为变更 = 备份/迁移不再携带应用数据，debug 构建允许明文 HTTP。

- [ ] **Step 1: 修改 application 标签** — AndroidManifest.xml:43-47 改为：

```xml
    <application
        android:label="@string/app_name"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:theme="@style/Theme.MaterialComponents.DayNight.NoActionBar"
        android:allowBackup="false"
        android:fullBackupContent="false"
        android:dataExtractionRules="@xml/data_extraction_rules"
        android:usesCleartextTraffic="${usesCleartextTraffic}">
```

- [ ] **Step 2: 新建排除规则** — `src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml`（风格对齐同目录 file_paths.xml，不加许可头）：

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- 应用数据含 FTP 凭据哈希与 AI 服务 API Key（Rust 侧私有 config.json），
     云备份与设备迁移均显式排除全部数据域；allowBackup=false 之上再加一层
     显式规则，防未来翻转 allowBackup 时静默回退。 -->
<data-extraction-rules>
    <cloud-backup>
        <exclude domain="file" path="." />
        <exclude domain="sharedpref" path="." />
        <exclude domain="database" path="." />
        <exclude domain="external" path="." />
    </cloud-backup>
    <device-transfer>
        <exclude domain="file" path="." />
        <exclude domain="sharedpref" path="." />
        <exclude domain="database" path="." />
        <exclude domain="external" path="." />
    </device-transfer>
</data-extraction-rules>
```

- [ ] **Step 3: 验证构建** — Run: `./build.sh android`
  Expected: 构建成功（含 Kotlin 测试）。可选复核：检查 `src-tauri/gen/android/app/build/intermediates/merged_manifests/` 下合并清单含 `android:allowBackup="false"` 且占位符已被替换为 false。

- [ ] **Step 4: Commit**

```bash
git add src-tauri/gen/android/app/src/main/AndroidManifest.xml src-tauri/gen/android/app/src/main/res/xml/data_extraction_rules.xml
git commit -m "security(android): disable backup and wire cleartext placeholder

Default backup rules cover the files/ domain, shipping the argon2 hash and
AI API key to cloud backups. Also wire the dangling usesCleartextTraffic
manifest placeholder (debug=true/default=false) that was never referenced."
```

---

### Task 5: 启动失败保持权限对话框打开 + startServer 并发防重（P1）

**背景**：`PermissionDialog` 的 `onAllGranted()` 未 await/catch，而 `continueAfterPermissionsGranted`（serverStore.ts:102-105）`rethrow: true` → unhandled rejection 且对话框照样关闭（F5）。`startServer` 无 in-flight 防重，UI 按钮与托盘事件可并发触发——后端幂等不会双开，但第二个调用者会收到误导性的 `ServerAlreadyRunning` 错误提示（F6 修正口径）。

**Files:**
- Modify: `src/components/PermissionDialog.tsx`
- Modify: `src/stores/serverStore.ts:71-87,102-105`
- Test: `src/components/__tests__/PermissionDialog.test.tsx`（追加用例）
- Test: `src/stores/__tests__/serverStore.characterization.test.ts`（追加用例）

**Interfaces:**
- Consumes: `onAllGranted: () => void | Promise<void>`（App.tsx:87 传入 `continueAfterPermissionsGranted`，返回 Promise，兼容）。
- Produces: `PermissionDialogProps.onAllGranted` 类型放宽为 `() => void | Promise<void>`；`ServerState.startServer/continueAfterPermissionsGranted` 签名不变、新增 isLoading 早退语义。

- [ ] **Step 1: 写失败测试（组件）** — 在 `src/components/__tests__/PermissionDialog.test.tsx` 现有 describe 中追加（复用该文件顶部已有的 mock/render 导入方式；`usePermissionStore` 为 zustand 真 store，可直接 setState 控权）：

```tsx
  it('启动失败时保持打开并显示错误，不调用 onClose', async () => {
    const { usePermissionStore } = await import('../../stores/permissionStore');
    usePermissionStore.setState({ allGranted: true });
    const onAllGranted = vi.fn().mockRejectedValue(new Error('boom'));
    const onClose = vi.fn();

    render(<PermissionDialog isOpen onClose={onClose} onAllGranted={onAllGranted} />);
    fireEvent.click(screen.getByRole('button', { name: '开始服务' }));

    expect(await screen.findByText(/服务启动失败/)).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('启动成功后关闭对话框', async () => {
    const { usePermissionStore } = await import('../../stores/permissionStore');
    usePermissionStore.setState({ allGranted: true });
    const onAllGranted = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(<PermissionDialog isOpen onClose={onClose} onAllGranted={onAllGranted} />);
    fireEvent.click(screen.getByRole('button', { name: '开始服务' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
```

  注：若该文件已 `vi.mock` permission store，则以文件内既有 mock 控制方式设置 `allGranted: true` 替代上面的 setState。

- [ ] **Step 2: 确认失败** — Run: `bun run test -- PermissionDialog`
  Expected: 第一个用例 FAIL（当前实现同步 onClose，无错误提示文案）

- [ ] **Step 3: 实现（组件）** — `src/components/PermissionDialog.tsx`：

  1. import 行：`import { useEffect, useCallback, useState } from 'react';`
  2. Props 接口：`onAllGranted: () => void | Promise<void>;`
  3. 组件内（handleContinue 替换为）：

```tsx
  const [isStarting, setIsStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  // Handle continue button
  const handleContinue = useCallback(async () => {
    if (!allGranted || isStarting) return;
    setIsStarting(true);
    setStartError(null);
    try {
      // continueAfterPermissionsGranted 内部 rethrow（executeAsync rethrow:true），
      // 启动失败时保持对话框打开供重试；详细错误已由 serverStore.error
      // 经首页 ServerCard 的 ErrorMessage 展示。
      await onAllGranted();
      onClose();
    } catch {
      setStartError('服务启动失败，请重试');
    } finally {
      setIsStarting(false);
    }
  }, [allGranted, isStarting, onAllGranted, onClose]);
```

  4. footer 改为（包一层列容器并在失败时显示错误行）：

```tsx
      footer={
        <div className="flex flex-col gap-2 w-full">
          <div className="flex gap-3 w-full">
            <button
              onClick={onClose}
              className="flex-1 px-4 py-3 bg-gray-100 text-gray-700 rounded-xl hover:bg-gray-200"
            >
              取消
            </button>
            <button
              onClick={() => { void handleContinue(); }}
              disabled={!allGranted || isStarting}
              className={`flex-1 px-4 py-3 rounded-xl font-medium ${
                allGranted
                  ? 'bg-blue-500 text-white hover:bg-blue-600'
                  : 'bg-gray-200 text-gray-400 cursor-not-allowed'
              }`}
            >
              {isStarting ? '启动中…' : allGranted ? '开始服务' : '请授予权限'}
            </button>
          </div>
          {startError && (
            <p className="text-xs text-red-600 text-center" data-testid="permission-start-error">
              {startError}
            </p>
          )}
        </div>
      }
```

- [ ] **Step 4: 确认通过** — Run: `bun run test -- PermissionDialog`
  Expected: PASS（新 2 例 + 既有 gating 用例）

- [ ] **Step 5: 写失败测试（store 防重）** — 在 `src/stores/__tests__/serverStore.characterization.test.ts` 追加：

```ts
  it('启动进行中时忽略并发调用（防 UI+托盘双触发）', async () => {
    useServerStore.setState({ isLoading: true, showPermissionDialog: false });
    const result = await useServerStore.getState().startServer();
    expect(result).toBe(false);
    expect(useServerStore.getState().showPermissionDialog).toBe(false);
  });
```

- [ ] **Step 6: 确认失败** — Run: `bun run test -- serverStore`
  Expected: FAIL（无防重时 startServer 继续走 permissionBridge/invoke 路径，测试环境下抛错或行为不符）

- [ ] **Step 7: 实现（store）** — `serverStore.ts` 两处加防重：

```ts
  startServer: async () => {
    // 防重：UI 按钮与托盘事件可能并发触发；后端 start_server 幂等，
    // 但并发调用会让第二个调用者收到误导性的 ServerAlreadyRunning 错误。
    if (get().isLoading) return false;

    const permissions = await permissionBridge.checkAll();
```

```ts
  continueAfterPermissionsGranted: async () => {
    if (get().isLoading) return;
    set({ showPermissionDialog: false });
    await doStartServer(set, get);
  },
```

- [ ] **Step 8: 回归** — Run: `bun run test`
  Expected: 全绿（含 App 相关 characterization 测试）

- [ ] **Step 9: Commit**

```bash
git add src/components/PermissionDialog.tsx src/stores/serverStore.ts src/components/__tests__/PermissionDialog.test.tsx src/stores/__tests__/serverStore.characterization.test.ts
git commit -m "fix(start-flow): keep permission dialog open on start failure; guard concurrent start

onAllGranted rejected unhandled and the dialog closed regardless; surface
a retryable error inline. Also early-return while a start is in flight so
the UI and the tray event cannot race into a misleading failure toast."
```

---

### Task 6: 密码保存失败给出反馈并保持编辑态（P1）

**背景**：`handlePasswordBlur` 的 catch 仅 console.error 并退出编辑模式（AdvancedConnectionConfig.tsx:204-207），用户输入的密码"看似已保存实际丢弃"；且 `configStore.saveAuthConfig` 不走 executeAsync、无 toast（F7）。

**Files:**
- Modify: `src/components/AdvancedConnectionConfig.tsx`（imports + :204-207 catch 块）
- Create（若不存在同名测试）: `src/components/__tests__/AdvancedConnectionConfig.password.test.tsx`

**Interfaces:**
- Consumes: `toast`（sonner）、`formatError`（../utils/error，:15 既有）。
- Produces: 无接口变更。

- [ ] **Step 1: 写失败测试** — 新建 `src/components/__tests__/AdvancedConnectionConfig.password.test.tsx`（先确认该目录下无既有 AdvancedConnectionConfig 测试文件，有则并入）：

```tsx
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { toast } from 'sonner';
import { AdvancedConnectionConfigPanel } from '../AdvancedConnectionConfig';
import type { AdvancedConnectionConfig as AdvancedConnectionConfigType } from '../../types';

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }));

const saveAuthConfigMock = vi.fn();
vi.mock('../../stores/configStore', () => ({
  useConfigStore: (selector: (s: { saveAuthConfig: unknown }) => unknown) =>
    selector({ saveAuthConfig: saveAuthConfigMock }),
}));

const baseConfig = {
  port: 2121,
  auth: { anonymous: false, username: 'user', passwordHash: '' },
} as unknown as AdvancedConnectionConfigType;

function renderPanel() {
  return render(
    <AdvancedConnectionConfigPanel
      config={baseConfig}
      port={2121}
      platform="windows"
      isLoading={false}
      onUpdate={vi.fn()}
    />,
  );
}

describe('AdvancedConnectionConfigPanel password save failure', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('保存失败时 toast 报错并保持编辑态（输入不丢失）', async () => {
    renderPanel();
    const input = screen.getByPlaceholderText('输入密码');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'secret123' } });
    saveAuthConfigMock.mockRejectedValueOnce(new Error('disk full'));
    fireEvent.blur(input);

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('密码保存失败')),
    );
    expect((input as HTMLInputElement).value).toBe('secret123');
  });

  it('保存成功后正常退出编辑模式', async () => {
    renderPanel();
    const input = screen.getByPlaceholderText('输入密码');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'secret123' } });
    saveAuthConfigMock.mockResolvedValueOnce(undefined);
    fireEvent.blur(input);

    await waitFor(() => expect((input as HTMLInputElement).value).toBe(''));
    expect(toast.error).not.toHaveBeenCalled();
  });
});
```

  注：若 `AdvancedConnectionConfigType` 的实际字段与上方不符，以 `src/types`（ts-rs 生成再导出）为准调整 `baseConfig`。

- [ ] **Step 2: 确认失败** — Run: `bun run test -- AdvancedConnectionConfig`
  Expected: 第一个用例 FAIL（当前 catch 不 toast 且清空编辑态）

- [ ] **Step 3: 实现** — `AdvancedConnectionConfig.tsx`：

  1. import 增加：`import { toast } from 'sonner';` 与 `import { formatError } from '../utils/error';`
  2. `handlePasswordBlur` 的 catch（:204-207）替换为：

```tsx
    } catch (error) {
      console.error('Failed to save auth config:', error);
      toast.error('密码保存失败：' + formatError(error));
      // 保持编辑模式与已输入内容，用户可修改后重试失焦保存；
      // 清空输入再失焦即可放弃修改退出编辑。
    }
```

  （删除原 catch 中的 `setIsEditingPassword(false);`；try 成功分支不动。）

- [ ] **Step 4: 确认通过** — Run: `bun run test -- AdvancedConnectionConfig`
  Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/AdvancedConnectionConfig.tsx src/components/__tests__/AdvancedConnectionConfig.password.test.tsx
git commit -m "fix(config-ui): surface password save failures instead of silently dropping

The blur handler logged the error and left edit mode, making a failed
Argon2/persist look like a successful save. Toast the failure and keep
the typed value for retry."
```

---

### Task 7: 端口检查区分"异常"与"被占用"（P1）

**背景**：`usePortCheck.checkPort` catch 返回 `{ available: false }`（usePortCheck.ts:49-50），组件把 IPC 异常与真实占用共用"端口 X 已被占用"文案（AdvancedConnectionConfig.tsx:155-158 → :84-85），误导用户（F8）。

**Files:**
- Modify: `src/hooks/usePortCheck.ts`（:14-17 接口 + :43-54 实现）
- Modify: `src/components/AdvancedConnectionConfig.tsx`（:25-29 类型、:76-87 文案、:155-159 处理）
- Test: `src/hooks/__tests__/usePortCheck.test.ts`（追加用例）
- Test: `src/components/__tests__/AdvancedConnectionConfig.password.test.tsx`（Task 6 建立的文件中追加，或新建 `AdvancedConnectionConfig.port.test.tsx`）

**Interfaces:**
- Produces: `export interface PortCheckResult { available: boolean; error?: string }`；`UsePortCheckResult.checkPort: (port: number) => Promise<PortCheckResult>`；`PortValidationError` 新增变体 `{ type: 'port_check_failed' }`。

- [ ] **Step 1: 写失败测试（hook）** — 在 `src/hooks/__tests__/usePortCheck.test.ts` 追加（复用该文件顶部既有的 invoke mock 方式；若其 mock 变量名不同，按文件内的名字调整下面两处调用）：

```ts
  it('IPC 异常时返回 error 而非误报占用', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('ipc down'));
    const { result } = renderHook(() => usePortCheck());

    const outcome = await result.current.checkPort(2121);

    expect(outcome.available).toBe(false);
    expect(outcome.error).toBe('ipc down');
  });

  it('正常路径不携带 error 字段', async () => {
    mockedInvoke.mockResolvedValueOnce(true);
    const { result } = renderHook(() => usePortCheck());

    const outcome = await result.current.checkPort(2121);

    expect(outcome.available).toBe(true);
    expect(outcome.error).toBeUndefined();
  });
```

- [ ] **Step 2: 确认失败** — Run: `bun run test -- usePortCheck`
  Expected: 第一个用例 FAIL（当前实现无 error 字段）

- [ ] **Step 3: 实现（hook）** — `usePortCheck.ts`：

  1. 接口区（:14-17）替换为：

```typescript
export interface PortCheckResult {
  available: boolean;
  /** IPC/命令异常（区别于端口被占用）。存在时 available 恒为 false。 */
  error?: string;
}

interface UsePortCheckResult {
  checkPort: (port: number) => Promise<PortCheckResult>;
  isChecking: boolean;
}
```

  2. `checkPort` 的 catch（:49-50）替换为：

```typescript
    } catch (e) {
      return { available: false, error: e instanceof Error ? e.message : String(e) };
    } finally {
```

- [ ] **Step 4: 确认通过** — Run: `bun run test -- usePortCheck`
  Expected: PASS

- [ ] **Step 5: 写失败测试（组件）** — 在 Task 6 的测试文件中追加 describe（需再 mock usePortCheck；在文件顶部 vi.mock 区增加）：

```tsx
const checkPortMock = vi.fn();
vi.mock('../../hooks/usePortCheck', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../hooks/usePortCheck')>();
  return { ...actual, usePortCheck: () => ({ checkPort: checkPortMock, isChecking: false }) };
});
```

```tsx
describe('AdvancedConnectionConfigPanel port check errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('检查异常时显示检查失败而非占用', async () => {
    checkPortMock.mockResolvedValueOnce({ available: false, error: 'ipc down' });
    renderPanel();
    const portInput = screen.getByPlaceholderText('1-65535');

    fireEvent.change(portInput, { target: { value: '3000' } });
    fireEvent.blur(portInput);

    expect(await screen.findByText(/端口检查失败/)).toBeInTheDocument();
    expect(screen.queryByText(/已被占用/)).not.toBeInTheDocument();
  });

  it('真实占用仍显示占用文案', async () => {
    checkPortMock.mockResolvedValueOnce({ available: false });
    renderPanel();
    const portInput = screen.getByPlaceholderText('1-65535');

    fireEvent.change(portInput, { target: { value: '3000' } });
    fireEvent.blur(portInput);

    expect(await screen.findByText(/端口 3000 已被占用/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: 确认失败** — Run: `bun run test -- AdvancedConnectionConfig`
  Expected: 第一个用例 FAIL（无"端口检查失败"文案）

- [ ] **Step 7: 实现（组件）** — `AdvancedConnectionConfig.tsx`：

  1. `PortValidationError`（:25-29）追加变体：`| { type: 'port_check_failed' }`
  2. `getPortErrorMessage`（:76-87）switch 追加：

```typescript
      case 'port_check_failed':
        return '端口检查失败，无法确认端口状态，请重试';
```

  3. `handlePortBlur`（:155-159）替换为：

```typescript
    const checkResult = await checkPort(parsedPort.port);
    if (checkResult.error) {
      setPortError({ type: 'port_check_failed' });
      return;
    }
    if (!checkResult.available) {
      setPortError({ type: 'port_in_use', port: parsedPort.port });
      return;
    }
```

- [ ] **Step 8: 全量回归** — Run: `bun run test`，随后 Run: `./build.sh windows android`
  Expected: 全绿 / 双平台构建成功

- [ ] **Step 9: Commit**

```bash
git add src/hooks/usePortCheck.ts src/components/AdvancedConnectionConfig.tsx src/hooks/__tests__/usePortCheck.test.ts src/components/__tests__/AdvancedConnectionConfig.password.test.tsx
git commit -m "fix(config-ui): distinguish port-check errors from port-in-use

IPC failures were reported as 'port occupied'; add an error channel to
checkPort and a dedicated message so users are not misled into changing
ports that are actually free."
```

---

## 批次收尾

- [ ] 全量验证：`./build.sh windows android`（gen-types + 前端构建 + Rust/前端测试 + 双平台编译一次通过）
- [ ] 人工冒烟（Windows 真机）：改密码无卡顿、错误端口提示、图库滚到底、日期跳转。
- [ ] 人工冒烟（Android）：权限对话框启动失败路径（可临时停用后端模拟）、备份行为无需肉眼验证（构建期语义）。

## 风险与回退

- Task 1：jsdom 下测量回退默认值，既有测试理论上零影响；真机上 ResizeObserver 触发时机晚于首帧时首帧用回退值渲染，随即校正（无布局跳变风险：首帧本就无滚动）。
- Task 2：稳定性探测使每个新文件索引延迟 ~220ms（可接受）；watcher 5s 超时跳过 + Put 兜底的既有语义未变。
- Task 4：已有 debug 安装会因 allowBackup 变化提示重新安装属预期；`dataExtractionRules` 在 minSdk 35 下必受支持。
- 全部任务按 task 独立成 commit，可单独 revert。
