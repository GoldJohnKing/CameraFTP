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
  dateModifiedSec: number;
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

方法：

- `listMediaPage(req: MediaPageRequest): Promise<MediaPageResponse>`

### 5.2 缩略图任务接口

```ts
interface ThumbRequest {
  requestId: string;
  mediaId: string;
  uri: string;
  dateModifiedSec: number;
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

方法：

- `enqueueThumbnails(reqs: ThumbRequest[]): Promise<void>`
- `cancelThumbnailRequests(requestIds: string[]): Promise<void>`
- `cancelByView(viewId: string): Promise<void>`
- `subscribeThumbnailResults(viewId: string, cb: (r: ThumbResult) => void): () => void`
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

### 8.3 解码策略

- 尺寸桶：`s`（约 180~220）/ `m`（约 320~380）
- 先读 bounds 计算采样率，再按桶目标缩放
- 固定中档压缩质量，优先稳定延迟
- 在解码阶段处理 EXIF 方向

### 8.4 缓存策略

- L1：`LruCache`（按字节上限）
- L2：磁盘缓存目录 `thumb/v2/<bucket>/<hash>.jpg`
- key：`sha1(mediaId:dateModifiedSec:sizeBucket)`
- 清理触发：启动延迟后台清理 + 阈值触发 LRU 淘汰
- 不在列表加载主路径执行全盘扫描清理

### 8.5 分页查询

- 排序：`dateModified desc, mediaId desc`
- 游标带上条记录双键，保证稳定翻页
- 不按 displayName 去重，避免误丢有效项

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

### 9.2 测试场景矩阵

1. 大图库冷启动（5k / 10k / 20k）
2. 高速滚动 10~20 秒
3. 停滚补齐（300ms 目标）
4. 前后台切换恢复
5. 上传/删除后的增量刷新
6. 低端机内存压力场景

### 9.3 门禁

每次重构提交后执行统一压测脚本，产出 P50/P95/P99 与漏图率报告。核心 SLO 任一不达标则失败。

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
