# Android 前台自动显示新图片 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an Android-only option that automatically shows the newest received image in the built-in viewer while the app process is visible, and reuses the existing viewer instead of stacking a new viewer screen.

**Architecture:** Keep the decision logic in the frontend and keep Android lifecycle/viewer reuse in Kotlin. Extend the existing `androidImageViewer` config, add a focused frontend coordinator around `gallery-items-added`, and add a native bridge API that either opens or navigates the existing `ImageViewerActivity` instance to the newest image.

**Tech Stack:** Rust config + ts-rs bindings, React 18 + TypeScript + Vitest, Android Kotlin bridges/activities, existing `./build.sh gen-types`, `npm test`, and `./build.sh windows android` verification flow.

---

## File Structure Map

### Existing files to modify

- `src-tauri/src/config.rs` — source of truth for `AndroidImageViewerConfig` and `AppConfig` defaults.
- `src-tauri/bindings/AndroidImageViewerConfig.ts` — generated binding that will gain the new boolean field.
- `src-tauri/bindings/AppConfig.ts` — generated aggregate config type.
- `src/types/index.ts` — frontend type re-export surface.
- `src/components/ConfigCard.tsx` — Android image viewer settings UI.
- `src/components/GalleryCard.tsx` — current `gallery-items-added` listener; will host or call the new coordinator hook.
- `src/services/image-open.ts` — existing built-in/external viewer dispatch; extend with a reuse-aware Android path.
- `src/types/global.ts` — add bridge typings for app visibility and viewer reuse.
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` — track process-visible state for JS bridge use.
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt` — add open-or-navigate and visibility accessors.
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt` — support reusing the active viewer instead of always launching a new one.
- `src-tauri/gen/android/app/src/main/AndroidManifest.xml` — set `launchMode` only if required by the chosen reuse mechanism.
- `src/services/__tests__/image-open.test.ts` — extend Android built-in viewer tests.
- `src/stores/__tests__/configStore.test.ts` — add Android config persistence coverage if config handling needs a regression test.

### New files to create

- `src/hooks/useAndroidAutoOpenLatestPhoto.ts` — focused frontend coordinator that reacts to `gallery-items-added`.
- `src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx` — unit tests for coordinator conditions and newest-item selection.
- `src/components/__tests__/ConfigCard.android-image-viewer.test.tsx` — UI visibility test for the new Android setting.
- `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ImageViewerActivityReuseTest.kt` — JVM test for reuse/update logic extracted from `ImageViewerActivity` helper methods.

### Generated output

- Run `./build.sh gen-types` after changing `src-tauri/src/config.rs`.

---

### Task 1: Extend config model and generated types

**Files:**
- Modify: `src-tauri/src/config.rs:115-128`
- Modify: `src-tauri/src/config.rs:176-214`
- Modify: `src/types/index.ts`
- Regenerate: `src-tauri/bindings/AndroidImageViewerConfig.ts`
- Regenerate: `src-tauri/bindings/AppConfig.ts`
- Test: `src/stores/__tests__/configStore.test.ts`

- [ ] **Step 1: Write the failing config-store regression test**

Add an Android-shaped config fixture and assert the new field survives draft updates:

```ts
it('preserves android auto-open viewer flag in draft updates', () => {
  const androidConfig: AppConfig = {
    ...baseConfig,
    androidImageViewer: {
      openMethod: 'built-in-viewer',
      autoOpenLatestWhenVisible: true,
    },
  };

  useConfigStore.setState({
    ...useConfigStore.getState(),
    config: androidConfig,
    draft: androidConfig,
    platform: 'android',
  });

  useConfigStore.getState().updateDraft((draft) => ({
    ...draft,
    androidImageViewer: {
      ...draft.androidImageViewer!,
      openMethod: 'external-app',
    },
  }));

  expect(useConfigStore.getState().draft?.androidImageViewer).toEqual({
    openMethod: 'external-app',
    autoOpenLatestWhenVisible: true,
  });
});
```

- [ ] **Step 2: Run the targeted test to verify the new field is missing**

Run: `npm test -- src/stores/__tests__/configStore.test.ts`

Expected: TypeScript/Vitest failure mentioning `autoOpenLatestWhenVisible` missing from `AndroidImageViewerConfig`.

- [ ] **Step 3: Add the Rust config field and default**

Update `src-tauri/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AndroidImageViewerConfig {
    pub open_method: AndroidImageOpenMethod,
    pub auto_open_latest_when_visible: bool,
}

impl Default for AndroidImageViewerConfig {
    fn default() -> Self {
        Self {
            open_method: AndroidImageOpenMethod::default(),
            auto_open_latest_when_visible: false,
        }
    }
}
```

- [ ] **Step 4: Regenerate the TypeScript bindings**

Run: `./build.sh gen-types`

Expected: `src-tauri/bindings/AndroidImageViewerConfig.ts` exports:

```ts
export interface AndroidImageViewerConfig {
  openMethod: AndroidImageOpenMethod;
  autoOpenLatestWhenVisible: boolean;
}
```

- [ ] **Step 5: Ensure frontend type exports still expose the generated config**

Keep or update `src/types/index.ts` so the Android viewer config stays exported from bindings:

```ts
export type { AndroidImageViewerConfig } from '../../src-tauri/bindings/AndroidImageViewerConfig';
export type { AppConfig } from '../../src-tauri/bindings/AppConfig';
```

- [ ] **Step 6: Re-run the targeted config-store test**

Run: `npm test -- src/stores/__tests__/configStore.test.ts`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/bindings/AndroidImageViewerConfig.ts src-tauri/bindings/AppConfig.ts src/types/index.ts src/stores/__tests__/configStore.test.ts
git commit -m "feat: add android auto-open viewer config"
```

---

### Task 2: Add the Android settings toggle and hide it for external viewer mode

**Files:**
- Modify: `src/components/ConfigCard.tsx:203-232`
- Create: `src/components/__tests__/ConfigCard.android-image-viewer.test.tsx`

- [ ] **Step 1: Write the failing UI test for the toggle visibility rules**

Create `src/components/__tests__/ConfigCard.android-image-viewer.test.tsx` with a focused Android config case:

```tsx
it('shows auto-open toggle only for built-in viewer mode', async () => {
  renderConfigCard({
    platform: 'android',
    draft: {
      ...androidConfig,
      androidImageViewer: {
        openMethod: 'built-in-viewer',
        autoOpenLatestWhenVisible: true,
      },
    },
  });

  expect(screen.getByText('前台接收新图片时自动显示')).toBeInTheDocument();

  await user.click(screen.getByRole('switch', { name: '使用外部应用打开图片' }));

  expect(screen.queryByText('前台接收新图片时自动显示')).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run the UI test to confirm the new control does not exist yet**

Run: `npm test -- src/components/__tests__/ConfigCard.android-image-viewer.test.tsx`

Expected: FAIL because the `前台接收新图片时自动显示` label is not rendered.

- [ ] **Step 3: Render and wire the new toggle in `ConfigCard`**

Extend the Android image viewer card:

```tsx
const isExternalViewer = draft.androidImageViewer.openMethod === 'external-app';

<div className="flex items-center justify-between">
  <div>
    <span className="text-sm font-medium text-gray-700">使用外部应用打开图片</span>
    <p className="text-xs text-gray-500 mt-0.5">使用第三方APP打开图片</p>
  </div>
  <ToggleSwitch
    enabled={isExternalViewer}
    onChange={(enabled) => {
      updateDraft(d => ({
        ...d,
        androidImageViewer: {
          ...d.androidImageViewer!,
          openMethod: enabled ? 'external-app' : 'built-in-viewer',
        },
      }));
    }}
    disabled={isLoading}
  />
</div>

{!isExternalViewer && (
  <div className="flex items-center justify-between">
    <div>
      <span className="text-sm font-medium text-gray-700">前台接收新图片时自动显示</span>
      <p className="text-xs text-gray-500 mt-0.5">仅内置图片查看器生效</p>
    </div>
    <ToggleSwitch
      enabled={draft.androidImageViewer.autoOpenLatestWhenVisible}
      onChange={(enabled) => {
        updateDraft(d => ({
          ...d,
          androidImageViewer: {
            ...d.androidImageViewer!,
            autoOpenLatestWhenVisible: enabled,
          },
        }));
      }}
      disabled={isLoading}
    />
  </div>
)}
```

- [ ] **Step 4: Re-run the UI test**

Run: `npm test -- src/components/__tests__/ConfigCard.android-image-viewer.test.tsx`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/components/ConfigCard.tsx src/components/__tests__/ConfigCard.android-image-viewer.test.tsx
git commit -m "feat: add android auto-open viewer setting"
```

---

### Task 3: Add the frontend coordinator and reuse-aware image-open service

**Files:**
- Create: `src/hooks/useAndroidAutoOpenLatestPhoto.ts`
- Create: `src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx`
- Modify: `src/components/GalleryCard.tsx:163-181`
- Modify: `src/services/image-open.ts:11-62`
- Modify: `src/types/global.ts:245-276`
- Test: `src/services/__tests__/image-open.test.ts`

- [ ] **Step 1: Write the failing coordinator test for newest-item auto-open**

Create `src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx`:

```tsx
it('auto-opens the newest added item when android app is visible and built-in viewer is enabled', async () => {
  const openImagePreview = vi.fn().mockResolvedValue(undefined);
  const isVisible = vi.fn().mockReturnValue(true);

  window.ImageViewerAndroid = {
    openViewer: vi.fn(),
    openOrNavigateTo: vi.fn(),
    closeViewer: vi.fn(),
    onExifResult: vi.fn(),
    resolveFilePath: vi.fn(),
    isAppVisible: isVisible,
  };

  renderHookHarness({
    items: [oldItem],
    config: {
      androidImageViewer: {
        openMethod: 'built-in-viewer',
        autoOpenLatestWhenVisible: true,
      },
    },
    openImagePreview,
  });

  window.dispatchEvent(new CustomEvent('gallery-items-added', {
    detail: { items: [midItem, newestItem], timestamp: Date.now() },
  }));

  await flush();

  expect(openImagePreview).toHaveBeenCalledWith({
    filePath: newestItem.uri,
    openMethod: 'built-in-viewer',
    allUris: [newestItem.uri, midItem.uri, oldItem.uri],
    preferReuse: true,
  });
});
```

- [ ] **Step 2: Write the failing service test for the reuse-aware bridge call**

Extend `src/services/__tests__/image-open.test.ts` with:

```ts
it('uses openOrNavigateTo for Android built-in auto-open reuse flow', async () => {
  const openOrNavigateTo = vi.fn();

  window.ImageViewerAndroid = {
    openViewer: vi.fn(),
    openOrNavigateTo,
    closeViewer: vi.fn(),
    onExifResult: vi.fn(),
    resolveFilePath: vi.fn().mockReturnValue('content://media/9'),
    isAppVisible: vi.fn().mockReturnValue(true),
  };

  vi.mocked(invoke).mockResolvedValueOnce(null);

  await openImagePreview({
    filePath: 'content://media/9',
    openMethod: 'built-in-viewer',
    allUris: ['content://media/9', 'content://media/8'],
    preferReuse: true,
  });

  expect(openOrNavigateTo).toHaveBeenCalledWith(
    'content://media/9',
    JSON.stringify(['content://media/9', 'content://media/8']),
  );
});
```

- [ ] **Step 3: Run the two targeted tests to verify they fail**

Run: `npm test -- src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx src/services/__tests__/image-open.test.ts`

Expected: FAIL because the hook file and `preferReuse`/`openOrNavigateTo` path do not exist yet.

- [ ] **Step 4: Extend the global Android bridge types**

Update `src/types/global.ts`:

```ts
interface ImageViewerAndroid {
  openViewer(uri: string, allUrisJson: string): boolean;
  openOrNavigateTo(uri: string, allUrisJson: string): boolean;
  closeViewer(): boolean;
  onExifResult(exifJson: string | null): void;
  resolveFilePath(uri: string): string | null;
  isAppVisible(): boolean;
}
```

- [ ] **Step 5: Add the reuse-aware image open path**

Update `src/services/image-open.ts`:

```ts
interface OpenImagePreviewParams {
  filePath: string;
  openMethod?: string;
  allUris?: string[];
  getAllUris?: () => Promise<string[]>;
  preferReuse?: boolean;
}

export async function openImagePreview({
  filePath,
  openMethod,
  allUris,
  getAllUris,
  preferReuse = false,
}: OpenImagePreviewParams): Promise<void> {
  if (openMethod === 'built-in-viewer' && window.ImageViewerAndroid?.openViewer) {
    const resolvedUris = allUris ?? (getAllUris ? await getAllUris() : await getMediaStoreUris());
    const viewerUris = resolvedUris.length > 0 ? resolvedUris : [filePath];

    if (preferReuse && window.ImageViewerAndroid.openOrNavigateTo) {
      window.ImageViewerAndroid.openOrNavigateTo(filePath, JSON.stringify(viewerUris));
    } else {
      window.ImageViewerAndroid.openViewer(filePath, JSON.stringify(viewerUris));
    }

    void sendExifToViewer(filePath);
    return;
  }

  if (window.PermissionAndroid?.openImageWithChooser) {
    window.PermissionAndroid.openImageWithChooser(filePath);
    return;
  }

  await invoke('open_preview_window', { filePath });
}
```

- [ ] **Step 6: Create the Android auto-open coordinator hook**

Create `src/hooks/useAndroidAutoOpenLatestPhoto.ts`:

```ts
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import type { MediaItemDto } from '../types';
import { openImagePreview } from '../services/image-open';

interface UseAndroidAutoOpenLatestPhotoParams {
  items: MediaItemDto[];
  openMethod?: string;
  autoOpenLatestWhenVisible?: boolean;
}

export function useAndroidAutoOpenLatestPhoto({
  items,
  openMethod,
  autoOpenLatestWhenVisible,
}: UseAndroidAutoOpenLatestPhotoParams): void {
  useEffect(() => {
    if (openMethod !== 'built-in-viewer' || !autoOpenLatestWhenVisible) {
      return;
    }

    const handleItemsAdded = (event: Event) => {
      const customEvent = event as CustomEvent<{ items: MediaItemDto[]; timestamp: number }>;
      const addedItems = customEvent.detail?.items ?? [];
      const newestItem = addedItems.at(-1);

      if (!newestItem?.uri || !window.ImageViewerAndroid?.isAppVisible?.()) {
        return;
      }

      const allUris = [
        ...addedItems.map((item) => item.uri),
        ...items.map((item) => item.uri).filter((uri) => !addedItems.some((item) => item.uri === uri)),
      ];

      void openImagePreview({
        filePath: newestItem.uri,
        openMethod,
        allUris,
        preferReuse: true,
      });
    };

    window.addEventListener('gallery-items-added', handleItemsAdded);
    return () => {
      window.removeEventListener('gallery-items-added', handleItemsAdded);
    };
  }, [items, openMethod, autoOpenLatestWhenVisible]);
}
```

- [ ] **Step 7: Use the coordinator from `GalleryCard` after the incremental add listener**

Integrate it with the current gallery list state:

```tsx
const androidViewerConfig = draft?.androidImageViewer;

useAndroidAutoOpenLatestPhoto({
  items: pager.items,
  openMethod: androidViewerConfig?.openMethod,
  autoOpenLatestWhenVisible: androidViewerConfig?.autoOpenLatestWhenVisible,
});
```

Keep the existing `pager.addItems(items)` listener in place so the hook only decides whether to auto-open.

- [ ] **Step 8: Re-run the targeted tests**

Run: `npm test -- src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx src/services/__tests__/image-open.test.ts`

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/hooks/useAndroidAutoOpenLatestPhoto.ts src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx src/components/GalleryCard.tsx src/services/image-open.ts src/types/global.ts src/services/__tests__/image-open.test.ts
git commit -m "feat: auto-open newest android photo in viewer"
```

---

### Task 4: Add Android visibility tracking and viewer reuse in native code

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt:90-129`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt:292-296`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt:15-84`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt:47-82`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt:109-147`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerAdapter.kt`
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml:72-77`
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ImageViewerActivityReuseTest.kt`

- [ ] **Step 1: Write the failing JVM test for in-place viewer updates**

Create `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ImageViewerActivityReuseTest.kt` around a new companion helper on `ImageViewerActivity`:

```kotlin
@Test
fun applyViewerUpdate_replacesUrisAndMovesToTargetIndex() {
    val initialUris = mutableListOf("content://1", "content://0")
    val updatedUris = listOf("content://2", "content://1", "content://0")

    val result = ImageViewerActivity.applyViewerUpdate(
        currentUris = initialUris,
        requestedUris = updatedUris,
        requestedTargetUri = "content://2",
    )

    assertEquals(updatedUris, result.uris)
    assertEquals(0, result.targetIndex)
}
```

- [ ] **Step 2: Run the Android unit test to verify the helper does not exist yet**

Run: `./gradlew testDebugUnitTest --tests com.gjk.cameraftpcompanion.ImageViewerActivityReuseTest`

Workdir: `/mnt/d/GitRepos/camera-ftp-companion/src-tauri/gen/android`

Expected: FAIL because `ImageViewerActivity.applyViewerUpdate(...)` does not exist yet.

- [ ] **Step 3: Track process-visible state in `MainActivity`**

Add a simple boolean and lifecycle hooks:

```kotlin
companion object {
    @Volatile
    var isAppVisible: Boolean = false
        private set
}

override fun onStart() {
    super.onStart()
    isAppVisible = true
}

override fun onStop() {
    isAppVisible = false
    super.onStop()
}
```

This matches the requested “process visible” behavior better than `onResume`/`onPause` for split-screen and multi-window visibility.

- [ ] **Step 4: Expose visibility and reuse-aware open API from `ImageViewerBridge`**

Extend `ImageViewerBridge.kt`:

```kotlin
@JavascriptInterface
fun isAppVisible(): Boolean = MainActivity.isAppVisible

@JavascriptInterface
fun openOrNavigateTo(uri: String, allUrisJson: String): Boolean {
    return try {
        val allUris = JSONArray(allUrisJson).let { json ->
            (0 until json.length()).map { json.getString(it) }
        }
        val targetIndex = allUris.indexOf(uri)
        if (targetIndex == -1) {
            Log.e(TAG, "openOrNavigateTo: target URI not found in list")
            return false
        }

        ImageViewerActivity.navigateOrStart(activity, allUris, targetIndex)
        true
    } catch (e: Exception) {
        Log.e(TAG, "openOrNavigateTo error", e)
        false
    }
}
```

- [ ] **Step 5: Add a reuse-first entry point in `ImageViewerActivity`**

Extend the companion object:

```kotlin
fun navigateOrStart(context: Context, uris: List<String>, targetIndex: Int) {
    instance?.navigateTo(uris, targetIndex)?.let { reused ->
        if (reused) return
    }
    start(context, uris, targetIndex)
}
```

Add an instance method that updates the adapter instead of relaunching:

```kotlin
fun navigateTo(updatedUris: List<String>, targetIndex: Int): Boolean {
    if (isFinishing || isDestroyed) {
        return false
    }

    uris.clear()
    uris.addAll(updatedUris)
    currentIndex = targetIndex.coerceIn(0, uris.lastIndex)
    (viewPager.adapter as? ImageViewerAdapter)?.replaceItems(uris)
    viewPager.setCurrentItem(currentIndex, false)
    updateUI()
    return true
}
```

Update `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerAdapter.kt` in the same task with a focused replacement API:

```kotlin
fun replaceItems(updatedUris: List<String>) {
    uris.clear()
    uris.addAll(updatedUris)
    notifyDataSetChanged()
}
```

- [ ] **Step 6: Only add `launchMode` if reuse through `instance` is insufficient**

Prefer keeping the manifest untouched. If testing shows Android still creates a second `ImageViewerActivity`, set:

```xml
<activity
    android:name=".ImageViewerActivity"
    android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
    android:exported="false"
    android:launchMode="singleTask"
    android:theme="@style/Theme.MaterialComponents.DayNight.NoActionBar" />
```

Do not add `launchMode` unless manual verification proves it is required.

- [ ] **Step 7: Re-run the Android unit test**

Run: `./gradlew testDebugUnitTest --tests com.gjk.cameraftpcompanion.ImageViewerActivityReuseTest`

Workdir: `/mnt/d/GitRepos/camera-ftp-companion/src-tauri/gen/android`

Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt src-tauri/gen/android/app/src/main/AndroidManifest.xml src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ImageViewerActivityReuseTest.kt
git commit -m "feat: reuse android image viewer for newest photos"
```

---

### Task 5: End-to-end verification

**Files:**
- Verify only

- [ ] **Step 1: Run all targeted frontend tests**

Run: `npm test -- src/stores/__tests__/configStore.test.ts src/components/__tests__/ConfigCard.android-image-viewer.test.tsx src/hooks/__tests__/useAndroidAutoOpenLatestPhoto.test.tsx src/services/__tests__/image-open.test.ts`

Expected: PASS

- [ ] **Step 2: Run Android unit tests for reuse logic**

Run: `./gradlew testDebugUnitTest --tests com.gjk.cameraftpcompanion.ImageViewerActivityReuseTest`

Workdir: `/mnt/d/GitRepos/camera-ftp-companion/src-tauri/gen/android`

Expected: PASS

- [ ] **Step 3: Run required full build verification**

Run: `./build.sh windows android`

Expected: both Windows and Android build successfully.

- [ ] **Step 4: Manual Android behavior check**

Verify all of the following on device/emulator:

```text
1. 内置查看器 + 开关开启 + App 可见：收到新图自动显示最新图片
2. 内置查看器已打开：收到新图直接翻到最新图片，不新增页面层级
3. 外部查看器模式：开关隐藏，收到新图不自动显示
4. App 不可见：收到新图不自动显示
5. 分屏/多窗口可见：收到新图仍自动显示
```

- [ ] **Step 5: Final commit**

```bash
git add .
git commit -m "feat: auto-open latest android photo in built-in viewer"
```

---

## Self-Review Checklist

- Spec coverage: config toggle, conditional visibility, app-visible-only behavior, newest-item targeting, in-place viewer reuse, and Android verification are all covered by Tasks 1-5.
- Placeholder scan: no `TODO`/`TBD` placeholders remain.
- Type consistency: use `autoOpenLatestWhenVisible` in TS and `auto_open_latest_when_visible` in Rust only, relying on existing serde/ts-rs camelCase output.
