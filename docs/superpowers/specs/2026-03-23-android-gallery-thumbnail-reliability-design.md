# Android 图库缩略图可靠性重构设计文档

**日期**: 2026-03-23
**版本**: 1.0
**范围**: Android 图库页面（缩略图加载链路）

---

## 1. 背景与问题定义

当前安卓图库缩略图链路在大图库场景下不可靠，主要表现为：

- 打开图库时前端卡死，数秒后才恢复响应。
- 上下快速滚动时，新视口缩略图补齐慢，长时间显示占位符。
- 视口内偶发缩略图不加载，必须再次滚动才能触发。

这些问题不是单点缺陷，而是多处设计叠加导致：全量渲染、同步缩略图拉取、频繁全量刷新、重清理策略、请求取消不足等。

---

## 2. 目标与验收标准（严格）

本次以“彻底稳定优先”为唯一目标，不做旧接口兼容。

### 2.1 核心 SLO

1. 首屏可交互时间（TTI）`P95 <= 500ms`
2. 首屏可见缩略图到位率：打开后 `1s 内 >= 95%`
3. 滚动停止后视口补齐时间 `P95 <= 300ms`

### 2.3 SLO 计量契约（Measurement Contract）

为保证验收可复现，统一以下口径：

1. **TTI 计量**
   - start: `gallery_open_start`（用户触发进入图库）
   - end: `gallery_first_interactive`（主线程完成首帧可交互渲染，且滚动容器可响应）

2. **首屏 1s 到位率**
   - 采样时刻：`gallery_first_interactive` 后第一个稳定布局帧
   - 稳定布局帧定义：连续 2 帧（>=16ms 间隔）可见索引范围一致
   - 分母：该帧内所有可见 tile 数（不含 overscan）
   - 分子：在 `gallery_open_start + 1000ms` 前变为 `ready` 的 tile 数

3. **停滚补齐 300ms**
   - start: `scroll_stop`
   - end: `viewport_fully_filled`
   - 分母：`scroll_stop` 时刻可见 tile 数
   - 判定：300ms 内全部可见 tile 为 `ready`

4. **统计分桶**
   - 冷缓存与热缓存分开统计
   - 低/中/高端机分开统计（不允许仅用混合平均）
   - 分档规则：低端（RAM<=4GB）、中端（4GB<RAM<=8GB）、高端（RAM>8GB）
   - 每个设备档位 `N >= 200` 样本才可判定通过

### 2.2 守护指标

- 视口漏图率（停留 >1s 仍占位）`< 0.5%`
- 缩略图任务取消有效率持续可观测
- 缓存命中率、排队长度、解码耗时稳定

---

## 3. 非目标

- 不覆盖 Windows 端图库实现。
- 不引入对旧 `GalleryAndroid` 缩略图接口的向后兼容。
- 不在本次引入远程动态配置或线上热修复体系（离线应用阶段不需要）。

---

## 4. 总体架构（方案 A）

采用“前后端协同重构”：

1. 前端窗口化渲染（虚拟网格）
2. Android 异步缩略图任务服务（队列 + 优先级 + 取消）
3. 媒体列表分页加载（游标）
4. 两级缓存（L1 内存 + L2 磁盘）与后台清理

### 4.1 架构分层

```
Frontend (React)
  ├─ useGalleryPager           # 仅负责分页媒体元数据
  ├─ VirtualGalleryGrid        # 仅负责窗口化渲染
  └─ useThumbnailScheduler     # 仅负责缩略图请求调度/取消/回填
           │
           │ window.GalleryAndroidV2
           ▼
Android Native (Kotlin)
  ├─ MediaPageProvider         # MediaStore 分页查询
  ├─ ThumbnailPipelineManager  # 入队、调度、并发、取消、去重
  ├─ ThumbnailDecoder          # 解码/缩放/压缩
  └─ ThumbnailCacheV2          # L1/L2 缓存与淘汰
```

---

## 5. 新接口协议（仅 V2）

本次直接切换到 `window.GalleryAndroidV2`，不保留旧接口并存。

### 5.1 媒体分页接口

```ts
type MediaCursor = string | null;

interface MediaPageRequest {
  cursor: MediaCursor;
  pageSize: number;
  sort: 'dateDesc';
}

interface MediaItemDto {
  mediaId: string;
  uri: string;
  dateModifiedMs: number;
  width: number | null;
  height: number | null;
  mimeType: string | null;
}

interface MediaPageResponse {
  items: MediaItemDto[];
  nextCursor: MediaCursor;
  revisionToken: string;
}
```

`revisionToken` 契约：

- 表示一次可枚举数据快照版本（来源于本地索引快照版本号）
- 任何分页请求若 token 不一致，返回 `stale_cursor` 错误
- 客户端收到 `stale_cursor` 后必须：
  1) 清空分页 cursor；
  2) 保留已加载项的 `mediaId` 去重集；
  3) 重新从第一页拉取并按 `mediaId` 幂等合并

方法：

- `listMediaPage(req: MediaPageRequest): Promise<MediaPageResponse>`

### 5.2 缩略图任务接口

```ts
interface ThumbRequest {
  requestId: string;
  mediaId: string;
  uri: string;
  dateModifiedMs: number;
  sizeBucket: 's' | 'm';
  priority: 'visible' | 'nearby' | 'prefetch';
  viewId: string;
}

interface ThumbResult {
  requestId: string;
  mediaId: string;
  status: 'ready' | 'failed' | 'cancelled';
  localPath?: string;
  errorCode?: string;
}
```

回调契约：

- 回调在主线程派发，但以 16ms 分帧批量投递，避免回调风暴。
- 结果顺序不保证；客户端必须按 `requestId`/`wantedKey` 幂等处理。
- 对已取消请求，允许收到迟到结果；客户端必须可安全忽略。

事件通道契约：

- 使用事件通道代替闭包订阅，避免 WebView JS Bridge 兼容性风险。
- Native 分发入口：`window.__galleryThumbDispatch(listenerId, batchJson)`。
- `batchJson` 为 `ThumbResult[]`，单批上限 64 条，超量分帧投递。
- WebView/Activity 重建后，旧 `listenerId` 全部失效，前端必须重新注册。
- backlog 超阈值时优先丢弃 prefetch 回调并上报 `queue_overflow`。

方法：

- `enqueueThumbnails(reqs: ThumbRequest[]): Promise<void>`
- `cancelThumbnailRequests(requestIds: string[]): Promise<void>`
- `cancelByView(viewId: string): Promise<void>`
- `registerThumbnailListener(viewId: string, listenerId: string): Promise<void>`
- `unregisterThumbnailListener(listenerId: string): Promise<void>`
- `invalidateMediaIds(mediaIds: string[]): Promise<void>`
- `getQueueStats(): Promise<{ pending: number; running: number; cacheHitRate: number }>`

---

## 6. 关键数据流

### 6.1 打开图库

1. 前端请求第一页媒体元数据（建议 120 项）
2. 虚拟网格先渲染视口窗口，立即可交互
3. 调度器提交视口 `visible` 高优先级缩略图任务
4. 命中缓存直接回填，未命中异步解码后回填
5. 后续分页增量拉取，不阻塞首屏

### 6.2 滚动

1. 仅视口和 overscan 范围内节点存在
2. 新进入视口项入队高优先级请求
3. 离开有效区域项取消请求
4. 停滚后进入补齐模式，优先确保视口内缩略图到位

### 6.3 删除/上传/刷新

1. 事件进入统一协调器做去抖与合并
2. 执行增量更新，不触发全量重扫
3. 通过 `invalidateMediaIds` 精准失效缓存

---

## 7. 前端设计

### 7.1 模块拆分

- `useGalleryPager`: 分页、游标、revision 管理
- `VirtualGalleryGrid`: 仅窗口化布局与渲染
- `useThumbnailScheduler`: 请求分级、批处理、取消、结果回填

### 7.2 虚拟化策略

- 固定 3 列网格，DOM 仅保留视口 + overscan（前后 2~3 屏）
- 列表总高度按行数计算，滚动条保持真实
- Tile 复用，避免大规模 mount/unmount

### 7.3 调度策略

- 优先级：`visible > nearby > prefetch`
- 调度 tick：50~80ms 批量合并提交
- 停滚 120ms 后触发“补齐模式”
- 300ms 补齐窗口内若不足，暂停 prefetch

硬约束：

- 全局 `pending` 上限：600
- 分级配额：`visible 50% / nearby 35% / prefetch 15%`
- `visible` 保底并发槽位：至少 1 个 worker
- `cancelled-before-run` 目标：P95 < 20ms
- `cancel-latency`（发起取消到不再占用 worker）目标：P95 < 100ms

Native 强制背压：

- `maxQueued = 600`
- `maxInFlight = workerCount`
- 溢出时优先丢弃 `prefetch` queued 请求并上报 `queue_overflow`

### 7.4 防漏图机制

- 每个 tile 保存 `wantedKey = mediaId@dateModified@sizeBucket`
- 回填只接受匹配 `wantedKey` 的结果，防止错位
- 可见项守护扫描（200ms）对异常空白项补发请求

---

## 8. Android 设计

### 8.1 队列与线程模型

- Ingress：接收批量请求、去重、入优先级队列
- WorkerPool：固定 2~4 个后台线程做解码/压缩
- Dispatcher：批量推送结果到 WebView 回调

主线程不执行 bitmap 重解码。

### 8.2 任务状态机

`queued -> running -> ready | failed | cancelled`

规则：

- 同 key（`mediaId+dateModified+bucket`）去重合并
- queued 任务可直接取消
- running 任务支持软取消（不可中断阶段完成后丢弃）
- 失败分级（可重试/不可重试），有限退避

错误码与重试策略：

- `io_transient`: 可重试
- `decode_corrupt`: 不可重试，直接 failed
- `permission_denied`: 不可重试，直接 failed
- `oom_guard`: 不重试，触发降级模式
- `cancelled`: 不重试

重试矩阵（errorCode × priority）：

| errorCode | visible | nearby | prefetch |
|---|---:|---:|---:|
| io_transient | totalAttempts=2，退避 100ms | totalAttempts=2，退避 200ms | totalAttempts=1（不重试） |
| decode_corrupt | totalAttempts=1 | totalAttempts=1 | totalAttempts=1 |
| permission_denied | totalAttempts=1 | totalAttempts=1 | totalAttempts=1 |
| oom_guard | totalAttempts=1（触发降级） | totalAttempts=1（触发降级） | totalAttempts=1（触发降级） |
| cancelled | totalAttempts=1 | totalAttempts=1 | totalAttempts=1 |

### 8.3 解码策略

- 尺寸桶：`s`（约 180~220）/ `m`（约 320~380）
- 先读 bounds 计算采样率，再按桶目标缩放
- 固定中档压缩质量，优先稳定延迟
- 在解码阶段处理 EXIF 方向

### 8.4 缓存策略

- L1：`LruCache`（按字节上限）
- L2：磁盘缓存目录 `thumb/v2/<bucket>/<hash>.jpg`
- key：`sha1(mediaId:dateModifiedMs:sizeBucket:orientation:byteSize)`
- 清理触发：启动延迟后台清理 + 阈值触发 LRU 淘汰
- 不在列表加载主路径执行全盘扫描清理

容量与低存储策略：

- L1 默认上限：`min(32MB, heapClass * 0.08)`
- L2 默认上限：256MB（低端机 128MB）
- 可用存储 < 1GB 时，L2 进入紧缩模式（上限减半）
- 清理批次预算：每批次 <= 50ms，后台分段执行

优先级规则：

- 存在 `LocalTuningProfile` 时，profile 参数覆盖默认容量与并发。
- 仅在 profile 缺失时使用默认值。

### 8.5 分页查询

- 排序：`dateModified desc, mediaId desc`
- 游标带上条记录双键，保证稳定翻页
- 不按 displayName 去重，避免误丢有效项

一致性与去重：

- 客户端维护 `seenMediaIds`，分页合并时强制去重。
- 若 `stale_cursor`，执行整流重建：重拉第一页并保持选择状态按 `mediaId` 恢复。

### 8.6 本地调优参数配置（离线）

引入 `LocalTuningProfile`（本地静态配置，可在开发期调整）：

- `low-end`：并发 2，L2 128MB，prefetch 弱化
- `mid-end`：并发 3，L2 256MB
- `high-end`：并发 4，L2 384MB

每次启动记录生效参数快照到日志，便于回归比对。

---

## 9. 可观测性与验收

### 9.1 埋点

前端：

- `gallery_open_start`
- `gallery_first_interactive`
- `visible_thumbs_expected/ready`
- `scroll_stop`
- `viewport_fully_filled`
- `tile_stuck_placeholder_detected`

Android：

- `thumb_queue_enqueued/cancelled`
- `thumb_cache_l1_hit/l2_hit/decode_miss`
- `thumb_decode_duration_ms`
- `thumb_result_ready/failed/cancelled`
- `media_page_query_duration_ms`

埋点有效性与 fail-closed 规则：

- SLO 样本必须具备完整事件对，否则记为 `invalid_sample`。
  - TTI：必须同时有 `gallery_open_start` 与 `gallery_first_interactive`
  - 首屏到位率：必须同时有 `visible_thumbs_expected` 与 `visible_thumbs_ready`
  - 停滚补齐：必须同时有 `scroll_stop` 与 `viewport_fully_filled` 或超时标记
- `invalid_sample` 比例 > 2% 直接判定该轮测试失败（不进入 SLO 计算）。
- 缺失事件一律按失败处理，不允许静默丢弃后继续计算达标率。

### 9.2 测试场景矩阵

1. 大图库冷启动（5k / 10k / 20k）
2. 高速滚动 10~20 秒
3. 停滚补齐（300ms 目标）
4. 前后台切换恢复
5. 上传/删除后的增量刷新
6. 低端机内存压力场景
7. Activity 重建（旋转/系统回收）
8. WebView 重建后旧 `viewId` 任务回收
9. 运行中权限撤销
10. 缓存目录不可写与存储空间不足

### 9.3 门禁

每次重构提交后执行统一压测脚本，产出 P50/P95/P99 与漏图率报告。核心 SLO 任一不达标则失败。

压测协议固定：

- 数据集：5k/10k/20k，按近 30 天高密度分布 + 历史长尾混合
- 滚动脚本：固定轨迹（上/下 fling + 停顿），每轮 180s
- 每机型每场景 5 轮，去除首次预热后统计
- 冷/热缓存分别出报告

样本量与判定规则：

- 低/中/高端每个档位分别统计，禁止跨档位混合判定。
- 每个档位有效样本 `N >= 200` 才允许出最终结论。
- 若 5 轮不足 200，则自动补轮直到达到 `N >= 200` 或达到上限 12 轮。
- 达到 12 轮仍不足 200，判定为测试设计失败（门禁失败）。

最终通过条件（全部满足）：

1. 三个档位都达到有效样本下限；
2. 三个档位在冷/热缓存报告中均满足核心 SLO；
3. `invalid_sample` 比例在每个档位都 <= 2%。

---

## 10. 分阶段实施计划

### M1（1 周）：新链路打通

- 建立 `GalleryAndroidV2` 接口与分页查询
- 建立 `ThumbnailPipelineManager`
- 前端接入分页数据流
- 埋点打通

### M2（1 周）：前端性能主改

- 上线 `VirtualGalleryGrid`
- 上线 `useThumbnailScheduler`
- 移除旧全量渲染与同步缩略图路径

### M3（0.5~1 周）：性能调优

- 并发、桶尺寸、压缩参数、缓存阈值调优
- 异常退避、OOM 安全模式完善
- SLO 对齐

### M4（0.5 周）：收口与回归

- 删除 V1 代码
- 清理无效刷新触发链
- 固化压测基线与文档

---

## 11. 风险与对策

1. 分页游标边界错漏
   - 对策：双键排序 + 翻页一致性测试

2. 设备差异导致解码波动
   - 对策：并发分档、尺寸桶降级策略

3. 虚拟化与选择/长按交互冲突
   - 对策：交互状态以 `mediaId` 持久化，不依赖节点常驻

4. 回调过密导致主线程抖动
   - 对策：结果回调分帧批处理

5. 缓存索引与磁盘不一致
   - 对策：启动后后台一致性修复任务（限时批次）

---

## 12. 本次明确删除的旧设计

- 同步 `getThumbnail(path)` 直接重解码路径
- 列表变化时全盘 `cleanupThumbnailsNotInList` 清理路径
- 以 displayName 去重的媒体聚合方式
- 全量 `images.map(...)` 渲染整个图库节点

---

## 13. 结论

本方案通过“分页元数据 + 虚拟化渲染 + 异步缩略图队列 + 两级缓存”重构整个链路，直接对准当前卡死、补图慢、漏图三类故障根因。该方案改动面较大，但能以最短总路径达到严格性能目标，并为后续图库能力扩展提供稳定基础。

---

## 14. 实施就绪检查（Ready for Planning）

- [x] 严格 SLO 与计量口径明确
- [x] 分页一致性与 `revisionToken` 失效恢复定义
- [x] 队列上限、背压和取消 SLA 明确
- [x] 错误码与重试策略明确
- [x] 缓存容量、低存储/OOM 降级策略明确
- [x] 生命周期与异常场景测试覆盖
