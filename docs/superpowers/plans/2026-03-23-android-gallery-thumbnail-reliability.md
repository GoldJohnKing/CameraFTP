# Android 图库缩略图可靠性重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构 Android 图库缩略图加载链路，消除卡死、漏图与补图延迟，满足严格 SLO（TTI、首屏到位率、停滚补齐）。

**Architecture:** 采用 V2-only 全链路：前端分页数据源 + 虚拟网格 + 缩略图调度器；Android 侧分页提供器 + 异步缩略图队列 + L1/L2 缓存 + 事件批量回调。移除旧的同步 `getThumbnail` 与每次列表全量清理路径。

**Tech Stack:** React 18 + TypeScript 5 + Vitest；Android Kotlin + WebView JS Bridge + Robolectric；build 验证使用 `./build.sh windows android`。

---

## File Structure（先锁定边界）

### Frontend（create/modify）

- Create: `src/types/gallery-v2.ts`
  - V2 媒体分页 DTO、缩略图请求/结果 DTO、错误码与解析器。
- Modify: `src/types/global.ts`
  - 增加 `GalleryAndroidV2` bridge 类型，移除对旧缩略图 API 的依赖声明。
- Create: `src/services/gallery-media-v2.ts`
  - `listMediaPage`、`enqueue/cancel`、`cancelByView`、`invalidateMediaIds`、`register/unregister listener`、`getQueueStats` 适配层（统一 Promise 语义）。
- Create: `src/services/__tests__/gallery-media-v2.test.ts`
  - bridge 调用、JSON 解析、`stale_cursor`、接口全覆盖测试。
- Create: `src/hooks/useGalleryPager.ts`
  - 游标分页、刷新重建、`revisionToken/stale_cursor` 处理。
- Create: `src/hooks/useThumbnailScheduler.ts`
  - visible/nearby/prefetch 请求分级、取消、wantedKey 幂等回填。
- Create: `src/components/VirtualGalleryGrid.tsx`
  - 3 列窗口化渲染，overscan，可见范围输出。
- Create: `src/components/__tests__/VirtualGalleryGrid.test.tsx`
  - 仅渲染窗口范围、滚动后范围变更测试。
- Modify: `src/components/GalleryCard.tsx`
  - 迁移到 pager + virtual grid + scheduler。
- Modify: `src/hooks/useGalleryLibrary.ts`
  - 从“全量列表 hook”缩减为协调层，去除旧全量扫描假设。
- Modify: `src/services/latest-photo.ts`
  - 移除 Android 上“顺带全量图库扫描”路径。
- Modify: `src/utils/gallery-refresh.ts`
  - 收敛刷新触发，避免重复拉全量。
- Create: `src/utils/gallery-telemetry.ts`
  - 统一记录 SLO 事件与 invalid_sample。

### Android（create/modify）

- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProvider.kt`
  - MediaStore 游标分页查询，双键排序（`dateModified desc, mediaId desc`）。
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2.kt`
  - 统一 key 生成（`mediaId/dateModifiedMs/sizeBucket/orientation/byteSize`）。
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt`
  - L1/L2 缓存、容量上限、分批清理。
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt`
  - 三优先级队列、背压、取消、重试矩阵、批量事件回调。
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt`
  - JS 接口导出（分页、enqueue/cancel、cancelByView、listener 注册、invalidate、stats）。
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
  - 注入 `GalleryAndroidV2`，维护 listener 生命周期。
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
  - 删除/废弃旧缩略图主路径（保留仅非缩略图能力或移除注入）。

### Android tests（create/modify）

- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProviderTest.kt`
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2Test.kt`
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManagerTest.kt`
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt`
  - 删除旧缩略图行为断言，新增 V2 事件契约断言。

### Docs（modify/create）

- Modify: `docs/superpowers/specs/2026-03-23-android-gallery-thumbnail-reliability-design.md`
  - 若实现中发现小范围偏差，同步回写约束。
- Create: `docs/perf/gallery-thumbnail-v2-baseline.md`
  - 固化 P50/P95/P99 与 invalid_sample 统计模板。
- Create: `scripts/perf/validate-gallery-slo.mjs`
  - 解析埋点数据并按 spec 执行 fail-closed 门禁。
- Create: `scripts/perf/assert-no-legacy-thumbnail-paths.mjs`
  - 自动阻断旧缩略图路径回流。

---

### Task 1: 建立 V2 类型与服务适配层（Frontend foundation)

**Files:**
- Create: `src/types/gallery-v2.ts`
- Modify: `src/types/global.ts`
- Create: `src/services/gallery-media-v2.ts`
- Test: `src/services/__tests__/gallery-media-v2.test.ts`

- [ ] **Step 1: 写失败测试（服务层）**

```ts
import { describe, expect, it, vi } from 'vitest';
import { listMediaPageV2 } from '../gallery-media-v2';

describe('gallery-media-v2', () => {
  it('exports all V2 contract methods', () => {
    expect(typeof listMediaPageV2).toBe('function');
    expect(typeof enqueueThumbnailsV2).toBe('function');
    expect(typeof cancelThumbnailRequestsV2).toBe('function');
    expect(typeof cancelByViewV2).toBe('function');
    expect(typeof registerThumbnailListenerV2).toBe('function');
    expect(typeof unregisterThumbnailListenerV2).toBe('function');
    expect(typeof invalidateMediaIdsV2).toBe('function');
    expect(typeof getQueueStatsV2).toBe('function');
  });

  it('maps bridge JSON to MediaPageResponse', async () => {
    window.GalleryAndroidV2 = {
      listMediaPage: vi.fn().mockResolvedValue(JSON.stringify({ items: [], nextCursor: null, revisionToken: 'r1' })),
    } as unknown as GalleryAndroidV2;
    const page = await listMediaPageV2({ cursor: null, pageSize: 120, sort: 'dateDesc' });
    expect(page.revisionToken).toBe('r1');
  });

  it('exposes full v2 bridge methods', async () => {
    window.GalleryAndroidV2 = {
      listMediaPage: vi.fn().mockResolvedValue('{"items":[],"nextCursor":null,"revisionToken":"r1"}'),
      cancelByView: vi.fn().mockResolvedValue(true),
      invalidateMediaIds: vi.fn().mockResolvedValue(true),
      getQueueStats: vi.fn().mockResolvedValue('{"pending":0,"running":0,"cacheHitRate":1}'),
    } as unknown as GalleryAndroidV2;
    await expect(cancelByViewV2('view-1')).resolves.toBeUndefined();
  });

  it('matches spec signatures with Promise semantics', async () => {
    await expect(enqueueThumbnailsV2([])).resolves.toBeUndefined();
    await expect(unregisterThumbnailListenerV2('listener-1')).resolves.toBeUndefined();
    await expect(cancelThumbnailRequestsV2([])).resolves.toBeUndefined();
    await expect(invalidateMediaIdsV2([])).resolves.toBeUndefined();
    await expect(getQueueStatsV2()).resolves.toEqual({ pending: expect.any(Number), running: expect.any(Number), cacheHitRate: expect.any(Number) });
  });
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/services/__tests__/gallery-media-v2.test.ts`
Expected: FAIL（缺少 `gallery-media-v2.ts` 或导出）

- [ ] **Step 3: 最小实现 `gallery-v2.ts` 与 `gallery-media-v2.ts`**

```ts
export interface MediaPageRequest { cursor: string | null; pageSize: number; sort: 'dateDesc'; }
export interface MediaItemDto { mediaId: string; uri: string; dateModifiedMs: number; width: number | null; height: number | null; mimeType: string | null; }
export interface MediaPageResponse { items: MediaItemDto[]; nextCursor: string | null; revisionToken: string; }

export async function listMediaPageV2(req: MediaPageRequest): Promise<MediaPageResponse> {
  const raw = await window.GalleryAndroidV2?.listMediaPage(JSON.stringify(req));
  return JSON.parse(raw ?? '{"items":[],"nextCursor":null,"revisionToken":""}') as MediaPageResponse;
}
```

- [ ] **Step 4: 扩展 `global.ts` bridge 类型并跑测试**

Run: `npm test -- src/services/__tests__/gallery-media-v2.test.ts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/types/gallery-v2.ts src/types/global.ts src/services/gallery-media-v2.ts src/services/__tests__/gallery-media-v2.test.ts
git commit -m "feat(gallery): add v2 bridge types and media adapter"
```

---

### Task 2: 实现 Android 分页查询提供器

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProvider.kt`
- Test: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProviderTest.kt`

- [ ] **Step 1: 写失败测试（排序与游标）**

```kotlin
@Test
fun page_query_sorts_by_date_modified_desc_then_media_id_desc() {
    val rows = listOf(/* fake rows */)
    val sorted = MediaPageProvider.sortRows(rows)
    assertEquals(sorted, sorted.sortedWith(compareByDescending<Row>{ it.dateModifiedMs }.thenByDescending { it.mediaId }))
}
```

- [ ] **Step 2: 运行失败测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*MediaPageProviderTest"`
Expected: FAIL（类不存在）

- [ ] **Step 3: 最小实现 `MediaPageProvider`**

```kotlin
data class MediaPageCursor(val dateModifiedMs: Long, val mediaId: Long)
data class MediaPageItem(val mediaId: String, val uri: String, val dateModifiedMs: Long, val width: Int?, val height: Int?, val mimeType: String?)
data class MediaPageResult(val items: List<MediaPageItem>, val nextCursor: String?, val revisionToken: String)

class MediaPageProvider(private val context: Context) {
    fun listPage(cursor: String?, pageSize: Int): MediaPageResult { /* query + sort + cursor */ TODO() }
}
```

- [ ] **Step 4: 跑测试确保通过**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*MediaPageProviderTest"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProvider.kt src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/MediaPageProviderTest.kt
git commit -m "feat(android): add mediasotre cursor paging provider for gallery v2"
```

---

### Task 3: 实现 V2 缓存 key 与缓存层

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2.kt`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt`
- Test: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2Test.kt`

- [ ] **Step 1: 写失败测试（key 稳定性）**

```kotlin
@Test
fun same_input_generates_same_key() {
    val a = ThumbnailKeyV2.of("1", 1000L, "s", 0, 100L)
    val b = ThumbnailKeyV2.of("1", 1000L, "s", 0, 100L)
    assertEquals(a, b)
}
```

- [ ] **Step 2: 运行失败测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*ThumbnailKeyV2Test"`
Expected: FAIL

- [ ] **Step 3: 最小实现 key + L1/L2 缓存**

```kotlin
object ThumbnailKeyV2 {
    fun of(mediaId: String, dateModifiedMs: Long, sizeBucket: String, orientation: Int, byteSize: Long): String {
        return sha1("$mediaId:$dateModifiedMs:$sizeBucket:$orientation:$byteSize")
    }
}
```

- [ ] **Step 4: 跑测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*ThumbnailKeyV2Test"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailKeyV2Test.kt
git commit -m "feat(android): add v2 thumbnail key and cache layers"
```

---

### Task 4: 实现异步缩略图队列与背压

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt`
- Test: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManagerTest.kt`

- [ ] **Step 1: 写失败测试（优先级与取消）**

```kotlin
@Test
fun visible_jobs_are_scheduled_before_prefetch() {
    val manager = ThumbnailPipelineManager(/* fakes */)
    manager.enqueue(prefetchReq)
    manager.enqueue(visibleReq)
    assertEquals("visible", manager.nextJobForTest()?.priority)
}
```

- [ ] **Step 2: 运行失败测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*ThumbnailPipelineManagerTest"`
Expected: FAIL

- [ ] **Step 3: 最小实现三队列 + 背压 + 重试矩阵**

```kotlin
class ThumbnailPipelineManager {
    private val visible = ArrayDeque<Job>()
    private val nearby = ArrayDeque<Job>()
    private val prefetch = ArrayDeque<Job>()
    private val maxQueued = 600

    fun enqueue(job: Job) { /* dedupe + overflow drop prefetch */ }
    fun cancel(requestId: String) { /* queued remove or mark running cancelled */ }
}
```

- [ ] **Step 3.1: 补失败码×优先级重试矩阵测试**

```kotlin
@Test
fun prefetch_io_transient_has_no_retry() {
    // io_transient + prefetch => totalAttempts=1
}

@Test
fun queue_quota_split_respects_50_35_15() {
    // visible/nearby/prefetch pending share
}

@Test
fun visible_has_reserved_worker_slot() {
    // visible must not starve when nearby/prefetch are busy
}

@Test
fun overflow_drops_prefetch_first_and_emits_queue_overflow() {
    // enqueue beyond maxQueued and assert drop policy
}

@Test
fun callback_batch_size_is_capped_to_64_and_frame_split_16ms() {
    // dispatcher slices result batches by 64 and posts by frame
}

@Test
fun cancel_latency_respects_p95_budget_in_fake_clock() {
    // with controlled clock, queued cancel should complete under target budget
}
```

- [ ] **Step 4: 跑测试通过并补充失败码测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*ThumbnailPipelineManagerTest"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManagerTest.kt
git commit -m "feat(android): add v2 thumbnail pipeline with priority and backpressure"
```

---

### Task 5: 增加 GalleryBridgeV2 与事件通道

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt`

- [ ] **Step 1: 写失败测试（listener 注册/失效）**

```kotlin
@Test
fun unregister_listener_stops_dispatch() {
    val bridge = GalleryBridgeV2(fakeContext)
    bridge.registerThumbnailListener("view-1", "listener-1")
    bridge.unregisterThumbnailListener("listener-1")
    assertFalse(bridge.hasListenerForTest("listener-1"))
}

@Test
fun cancel_by_view_cancels_all_requests_for_view() {
    // register + enqueue(view-1/view-2) then cancelByView(view-1)
}

@Test
fun invalidate_media_ids_removes_cached_entries() {
    // call invalidate and verify cache no longer serves stale key
}
```

- [ ] **Step 2: 运行失败测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*GalleryBridgeTest"`
Expected: FAIL

- [ ] **Step 3: 最小实现 Bridge V2 + MainActivity 注入**

```kotlin
@JavascriptInterface
fun registerThumbnailListener(viewIdJson: String, listenerId: String) { /* ... */ }

@JavascriptInterface
fun cancelByView(viewId: String) { /* ... */ }

@JavascriptInterface
fun invalidateMediaIds(mediaIdsJson: String) { /* ... */ }

private fun dispatchThumbBatch(listenerId: String, payload: String) {
    val script = "window.__galleryThumbDispatch('${'$'}listenerId', ${'$'}payload)"
    activity.runOnUiThread { activity.getWebView()?.evaluateJavascript(script, null) }
}
```

```ts
// Frontend adapter keeps spec Promise signatures.
export async function registerThumbnailListenerV2(viewId: string, listenerId: string): Promise<void> { /* ... */ }
export async function cancelByViewV2(viewId: string): Promise<void> { /* ... */ }
export async function invalidateMediaIdsV2(mediaIds: string[]): Promise<void> { /* ... */ }
```

- [ ] **Step 4: 跑 bridge 测试**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest --tests "*GalleryBridgeTest"`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeTest.kt
git commit -m "feat(android): wire gallery bridge v2 with batched event dispatch"
```

---

### Task 6: 前端分页 Hook（useGalleryPager）

**Files:**
- Create: `src/hooks/useGalleryPager.ts`
- Modify: `src/hooks/useGalleryLibrary.ts`
- Test: `src/hooks/__tests__/useGalleryPager.test.ts`

- [ ] **Step 1: 写失败测试（stale_cursor 重建）**

```ts
it('restarts from first page when stale_cursor returned', async () => {
  // first load ok, second returns stale_cursor, expect reload from cursor null
});

it('deduplicates by seenMediaIds during stale cursor rebuild', async () => {
  // ensure no duplicates when rebuilding from page 1
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/hooks/__tests__/useGalleryPager.test.ts`
Expected: FAIL

- [ ] **Step 3: 最小实现 pager**

```ts
export function useGalleryPager() {
  const [items, setItems] = useState<MediaItemDto[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const loadNextPage = useCallback(async () => { /* call listMediaPageV2 */ }, [cursor]);
  return { items, loadNextPage };
}
```

- [ ] **Step 4: 跑测试**

Run: `npm test -- src/hooks/__tests__/useGalleryPager.test.ts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/hooks/useGalleryPager.ts src/hooks/useGalleryLibrary.ts src/hooks/__tests__/useGalleryPager.test.ts
git commit -m "feat(gallery): add v2 cursor pager hook"
```

---

### Task 7: 实现虚拟网格组件

**Files:**
- Create: `src/components/VirtualGalleryGrid.tsx`
- Test: `src/components/__tests__/VirtualGalleryGrid.test.tsx`

- [ ] **Step 1: 写失败测试（仅渲染窗口项）**

```tsx
it('renders only visible + overscan cells', () => {
  render(<VirtualGalleryGrid items={bigList} />);
  expect(screen.queryByTestId('tile-9999')).toBeNull();
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/components/__tests__/VirtualGalleryGrid.test.tsx`
Expected: FAIL

- [ ] **Step 3: 最小实现组件**

```tsx
export function VirtualGalleryGrid({ items, rowHeight, overscanRows, renderItem }: Props) {
  // compute startRow/endRow from scrollTop and viewportHeight
  // render absolute-positioned windowed rows
}
```

- [ ] **Step 4: 跑测试**

Run: `npm test -- src/components/__tests__/VirtualGalleryGrid.test.tsx`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/components/VirtualGalleryGrid.tsx src/components/__tests__/VirtualGalleryGrid.test.tsx
git commit -m "feat(gallery): add virtualized grid component"
```

---

### Task 8: 实现缩略图调度 Hook（useThumbnailScheduler）

**Files:**
- Create: `src/hooks/useThumbnailScheduler.ts`
- Test: `src/hooks/__tests__/useThumbnailScheduler.test.ts`

- [ ] **Step 1: 写失败测试（visible 优先 + 离屏取消）**

```ts
it('enqueues visible first and cancels out-of-range requests', async () => {
  // feed visible and prefetch ranges, verify enqueue/cancel calls
});

it('retries only by errorCode x priority matrix', async () => {
  // io_transient visible=2 attempts, prefetch=1 attempt
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/hooks/__tests__/useThumbnailScheduler.test.ts`
Expected: FAIL

- [ ] **Step 3: 最小实现调度逻辑**

```ts
export function useThumbnailScheduler() {
  const thumbByMediaId = useRef(new Map<string, string>());
  const updateViewport = useCallback((visibleIds: string[], nearbyIds: string[]) => { /* enqueue/cancel */ }, []);
  return { thumbByMediaId, updateViewport };
}
```

- [ ] **Step 4: 跑测试**

Run: `npm test -- src/hooks/__tests__/useThumbnailScheduler.test.ts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/hooks/useThumbnailScheduler.ts src/hooks/__tests__/useThumbnailScheduler.test.ts
git commit -m "feat(gallery): add v2 thumbnail scheduler with viewport priorities"
```

---

### Task 9: 组装到 GalleryCard，替换旧网格链路

**Files:**
- Modify: `src/components/GalleryCard.tsx`
- Modify: `src/hooks/useGalleryGrid.ts`（删除或降级为过渡辅助）

- [ ] **Step 1: 写失败测试（可交互渲染不依赖全量 map）**

```ts
it('does not mount all gallery tiles for large list', () => {
  // render with 10k items and assert mounted tiles bounded
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/components/__tests__/GalleryCard.virtualized.test.tsx`
Expected: FAIL

- [ ] **Step 3: 切换组件到新链路**

```tsx
<VirtualGalleryGrid
  items={pagedItems}
  onRangeChange={scheduler.updateViewport}
  renderItem={(item) => <GalleryTile thumbnail={scheduler.thumbnailMap.get(item.mediaId)} />}
/>
```

- [ ] **Step 4: 跑前端相关测试**

Run: `npm test -- src/components/__tests__/GalleryCard.virtualized.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts src/hooks/__tests__/useGalleryPager.test.ts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/components/GalleryCard.tsx src/hooks/useGalleryGrid.ts src/components/__tests__/GalleryCard.virtualized.test.tsx
git commit -m "refactor(gallery): migrate gallery card to v2 pager and virtual grid"
```

---

### Task 10: 刷新链路与 latest-photo 解耦

**Files:**
- Modify: `src/utils/gallery-refresh.ts`
- Modify: `src/services/latest-photo.ts`
- Test: `src/utils/__tests__/gallery-refresh.test.ts`

- [ ] **Step 1: 写失败测试（刷新触发合并）**

```ts
it('coalesces burst refresh events into single reload', async () => {
  // emit many events, expect one pager reload call
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/utils/__tests__/gallery-refresh.test.ts`
Expected: FAIL

- [ ] **Step 3: 最小实现合并调度与 latest-photo 解耦**

- [ ] **Step 4: 跑测试**

Run: `npm test -- src/utils/__tests__/gallery-refresh.test.ts`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/utils/gallery-refresh.ts src/services/latest-photo.ts src/utils/__tests__/gallery-refresh.test.ts
git commit -m "fix(gallery): coalesce refresh events and remove duplicate full scan path"
```

---

### Task 11: 可观测性与门禁脚本固化

**Files:**
- Create: `src/utils/gallery-telemetry.ts`
- Test: `src/utils/__tests__/gallery-telemetry.test.ts`
- Create: `scripts/perf/validate-gallery-slo.mjs`
- Create: `docs/perf/gallery-thumbnail-v2-baseline.md`
- Modify: `docs/superpowers/specs/2026-03-23-android-gallery-thumbnail-reliability-design.md`（如需补充实现差异）

- [ ] **Step 1: 写失败测试（事件对完整性与 invalid_sample）**

```ts
it('marks sample invalid when required event pair is missing', () => {
  // only gallery_open_start, missing gallery_first_interactive => invalid_sample
});
```

- [ ] **Step 2: 运行失败测试**

Run: `npm test -- src/utils/__tests__/gallery-telemetry.test.ts`
Expected: FAIL

- [ ] **Step 3: 实现 telemetry 记录器与 fail-closed 规则**

```ts
export function finalizeSample(sample: Partial<SloSample>): SloSampleResult {
  if (!sample.galleryOpenStart || !sample.galleryFirstInteractive) {
    return { valid: false, reason: 'missing_pair' };
  }
  return { valid: true, reason: null };
}
```

- [ ] **Step 4: 添加门禁脚本（分桶统计 + N>=200 + invalid_sample<=2%）**

Run: `node scripts/perf/validate-gallery-slo.mjs --input docs/perf/sample-gallery-events.json`
Expected: exit code 0 only when all buckets pass

- [ ] **Step 4.1: 增加门禁脚本测试（缺事件应 fail-closed）**

Run: `node scripts/perf/validate-gallery-slo.mjs --input docs/perf/sample-missing-events.json`
Expected: non-zero exit（invalid_sample 超阈值）

- [ ] **Step 5: 添加统计模板（按低/中/高档、冷/热缓存）**

- [ ] **Step 6: 手工跑一次全流程并填写样例报告**

Run: `./build.sh windows android`
Expected: BUILD SUCCESS（Windows + Android）

- [ ] **Step 7: 提交**

```bash
git add src/utils/gallery-telemetry.ts src/utils/__tests__/gallery-telemetry.test.ts scripts/perf/validate-gallery-slo.mjs docs/perf/gallery-thumbnail-v2-baseline.md docs/superpowers/specs/2026-03-23-android-gallery-thumbnail-reliability-design.md
git commit -m "feat(gallery): add slo telemetry and fail-closed gate validation"
```

---

### Task 12: 删除旧缩略图路径并最终验证

**Files:**
- Modify: `src/types/global.ts`
- Modify: `src/services/gallery-media.ts`
- Modify: `src/hooks/useGalleryGrid.ts`（删除旧实现）
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
- Create: `scripts/perf/assert-no-legacy-thumbnail-paths.mjs`

- [ ] **Step 1: 删除旧 `getThumbnail/cleanupThumbnailsNotInList` 前后端调用路径**

- [ ] **Step 2: 修复编译错误与测试断言**

- [ ] **Step 3: 跑前端单测**

Run: `npm test`
Expected: PASS

- [ ] **Step 4: 跑 Android 单测**

Run: `./src-tauri/gen/android/gradlew -p ./src-tauri/gen/android/app testUniversalDebugUnitTest`
Expected: PASS

- [ ] **Step 4.1: 运行最终 SLO 门禁（真实采样）**

Run: `node scripts/perf/validate-gallery-slo.mjs --input docs/perf/gallery-v2-real-run.json`
Expected: PASS（低/中/高 × 冷/热全部满足 N>=200、invalid_sample<=2%、3项核心SLO）

- [ ] **Step 5: 强制双平台构建验证（必做）**

Run: `./build.sh windows android`
Expected: BUILD SUCCESS（两个目标均通过）

- [ ] **Step 6: 提交收口**

```bash
git add src/types/global.ts src/services/gallery-media.ts src/hooks/useGalleryGrid.ts src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt
git commit -m "refactor(gallery): remove legacy thumbnail pipeline and complete v2 migration"
```

- [ ] **Step 7: 全仓收敛检查（无旧缩略图调用）**

Run: `node scripts/perf/assert-no-legacy-thumbnail-paths.mjs`
Expected: PASS（若发现 `getThumbnail`/`cleanupThumbnailsNotInList`/legacy bridge wiring 则非零退出）

---

## 依赖顺序与并行建议

- 串行必需：Task 1 → Task 2 → Task 3 → Task 4 → Task 5（先打通 V2 协议与 Native 能力）
- 可并行：Task 6/7/8（前端分页、虚拟化、调度器）
- 串行收口：Task 9 → Task 10 → Task 11 → Task 12

---

## 最终验收清单

- [ ] 三个核心 SLO 达标（按 spec 的分桶与样本规则）
- [ ] invalid_sample 比例每档位 `<= 2%`
- [ ] 无“视口停留 >1s 仍占位”的系统性漏图
- [ ] 不再存在旧同步缩略图和全盘清理路径
- [ ] `./build.sh windows android` 成功
