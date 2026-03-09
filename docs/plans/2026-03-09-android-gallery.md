# Android Gallery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a gallery page for Android platform to display image thumbnails from storage path using native MediaStore for optimal performance.

**Architecture:** Android native layer (GalleryBridge.kt) uses MediaStore to query images and get system-cached thumbnails, exposes via JS Bridge. React frontend (GalleryCard.tsx) renders grid layout with lazy loading.

**Tech Stack:** Kotlin, MediaStore API, React, CSS Grid, Intersection Observer

---

## Task 1: Add GalleryImage Type Definition

**Files:**
- Modify: `src/types/global.ts`

**Step 1: Add GalleryImage interface and GalleryAndroid bridge type**

Add after line 108 (after `PermissionAndroid` interface):

```typescript
/**
 * Gallery image data returned by Android MediaStore
 */
export interface GalleryImage {
  id: number;
  path: string;
  filename: string;
  thumbnail: string; // base64 data URL
  dateModified: number;
}

/**
 * Android Gallery interface
 * Provides access to device image gallery via MediaStore
 */
interface GalleryAndroid {
  /**
   * Get all images from the specified directory
   * @param storagePath The directory path to scan for images
   * @returns JSON string of GalleryImage array
   */
  getGalleryImages(storagePath: string): Promise<string>;
}
```

**Step 2: Add GalleryAndroid to Window interface**

In the `declare global { interface Window` block, add after `PermissionAndroid`:

```typescript
    /**
     * Android Gallery JS Bridge
     */
    GalleryAndroid?: GalleryAndroid;
```

**Step 3: Commit**

```bash
git add src/types/global.ts
git commit -m "feat(types): add GalleryImage and GalleryAndroid bridge types"
```

---

## Task 2: Update ConfigStore for Gallery Tab

**Files:**
- Modify: `src/stores/configStore.ts:17`
- Modify: `src/stores/configStore.ts:26`
- Modify: `src/stores/configStore.ts:98`

**Step 1: Add 'gallery' to activeTab type**

Change line 17:
```typescript
// Before
activeTab: 'home' | 'config';
// After
activeTab: 'home' | 'gallery' | 'config';
```

**Step 2: Update setActiveTab parameter type**

Change line 26:
```typescript
// Before
setActiveTab: (tab: 'home' | 'config') => void;
// After
setActiveTab: (tab: 'home' | 'gallery' | 'config') => void;
```

**Step 3: Update setActiveTab implementation type**

Change line 98:
```typescript
// Before
setActiveTab: (tab: 'home' | 'config') => {
// After
setActiveTab: (tab: 'home' | 'gallery' | 'config') => {
```

**Step 4: Commit**

```bash
git add src/stores/configStore.ts
git commit -m "feat(store): add 'gallery' to activeTab type"
```

---

## Task 3: Update BottomNav with Gallery Tab

**Files:**
- Modify: `src/components/BottomNav.tsx`

**Step 1: Add Images icon import**

Change line 8:
```typescript
// Before
import { Home, Settings } from 'lucide-react';
// After
import { Home, Settings, Images } from 'lucide-react';
```

**Step 2: Add gallery tab button**

Insert after line 27 (after the home button's closing `</button>`):

```typescript
        
        <button
          onClick={() => setActiveTab('gallery')}
          className={`flex-1 flex flex-col items-center py-3 px-4 transition-colors ${
            activeTab === 'gallery'
              ? 'text-blue-600'
              : 'text-gray-500 hover:text-gray-700'
          }`}
        >
          <Images className="w-6 h-6" />
          <span className="text-xs mt-1 font-medium">图库</span>
        </button>
```

**Step 3: Commit**

```bash
git add src/components/BottomNav.tsx
git commit -m "feat(nav): add gallery tab to bottom navigation"
```

---

## Task 4: Create GalleryCard Component

**Files:**
- Create: `src/components/GalleryCard.tsx`

**Step 1: Create GalleryCard.tsx**

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useState, useRef } from 'react';
import { RefreshCw, ImageOff, Loader2 } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import type { GalleryImage } from '../types';

export const GalleryCard = memo(function GalleryCard() {
  const { config } = useConfigStore();
  const [images, setImages] = useState<GalleryImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [visibleImages, setVisibleImages] = useState<Set<number>>(new Set());
  const observerRef = useRef<IntersectionObserver | null>(null);

  const loadImages = useCallback(async () => {
    if (!config?.savePath || !window.GalleryAndroid) {
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const result = await window.GalleryAndroid.getGalleryImages(config.savePath);
      const parsed = JSON.parse(result) as GalleryImage[];
      // Sort by dateModified descending (newest first)
      parsed.sort((a, b) => b.dateModified - a.dateModified);
      setImages(parsed);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load images');
      setImages([]);
    } finally {
      setIsLoading(false);
    }
  }, [config?.savePath]);

  // Load images on mount
  useEffect(() => {
    loadImages();
  }, [loadImages]);

  // Setup intersection observer for lazy loading
  useEffect(() => {
    observerRef.current = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const id = Number(entry.target.getAttribute('data-id'));
          if (entry.isIntersecting) {
            setVisibleImages((prev) => new Set(prev).add(id));
          }
        });
      },
      { rootMargin: '100px' }
    );

    return () => {
      observerRef.current?.disconnect();
    };
  }, []);

  // Observe image elements
  const imageRefCallback = useCallback((el: HTMLDivElement | null) => {
    if (el && observerRef.current) {
      observerRef.current.observe(el);
    }
  }, []);

  const handleImageClick = useCallback((image: GalleryImage) => {
    if (window.PermissionAndroid?.openImageWithChooser) {
      window.PermissionAndroid.openImageWithChooser(image.path);
    }
  }, []);

  const handleRefresh = useCallback(() => {
    loadImages();
  }, [loadImages]);

  // Not on Android
  if (!window.GalleryAndroid) {
    return null;
  }

  // Loading state
  if (isLoading && images.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <Loader2 className="w-8 h-8 text-blue-600 animate-spin" />
        <p className="mt-3 text-gray-500">加载中...</p>
      </div>
    );
  }

  // Empty state
  if (!isLoading && images.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <ImageOff className="w-12 h-12 text-gray-300" />
        <p className="mt-3 text-gray-500">暂无图片</p>
        <button
          onClick={handleRefresh}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
          刷新
        </button>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <p className="text-red-500">{error}</p>
        <button
          onClick={handleRefresh}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
          重试
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Header with refresh button */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900">
          图库 ({images.length})
        </h2>
        <button
          onClick={handleRefresh}
          disabled={isLoading}
          className="flex items-center gap-1 px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 hover:bg-gray-100 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
          刷新
        </button>
      </div>

      {/* Image grid */}
      <div className="grid grid-cols-3 gap-1">
        {images.map((image) => (
          <div
            key={image.id}
            data-id={image.id}
            ref={imageRefCallback}
            onClick={() => handleImageClick(image)}
            className="aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity"
          >
            {visibleImages.has(image.id) ? (
              <img
                src={image.thumbnail}
                alt={image.filename}
                className="w-full h-full object-cover"
                loading="lazy"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center">
                <div className="w-8 h-8 bg-gray-200 rounded animate-pulse" />
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
});
```

**Step 2: Commit**

```bash
git add src/components/GalleryCard.tsx
git commit -m "feat(ui): add GalleryCard component with lazy loading"
```

---

## Task 5: Integrate GalleryCard in App

**Files:**
- Modify: `src/App.tsx`

**Step 1: Add GalleryCard import**

Add after line 15:
```typescript
import { GalleryCard } from './components/GalleryCard';
```

**Step 2: Add gallery tab rendering**

In the main content section (around line 177-188), change:

```typescript
// Before
        <div className="space-y-4">
          {activeTab === 'home' ? (
            <>
              <ServerCard />
              <InfoCard />
              <LatestPhotoCard />
              <StatsCard />
            </>
          ) : (
            <ConfigCard />
          )}
        </div>

// After
        <div className="space-y-4">
          {activeTab === 'home' ? (
            <>
              <ServerCard />
              <InfoCard />
              <LatestPhotoCard />
              <StatsCard />
            </>
          ) : activeTab === 'gallery' ? (
            <GalleryCard />
          ) : (
            <ConfigCard />
          )}
        </div>
```

**Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat(app): integrate GalleryCard in main app"
```

---

## Task 6: Create GalleryBridge.kt

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`

**Step 1: Create GalleryBridge.kt**

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion.bridges

import android.annotation.SuppressLint
import android.content.Context
import android.database.Cursor
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.provider.MediaStore
import android.util.Base64
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

class GalleryBridge(private val context: Context) : BaseJsBridge(context as android.app.Activity) {

    companion object {
        private const val TAG = "GalleryBridge"
        private const val THUMBNAIL_QUALITY = 85
    }

    @android.webkit.JavascriptInterface
    fun getGalleryImages(storagePath: String): String {
        Log.d(TAG, "getGalleryImages: storagePath=$storagePath")
        
        val images = JSONArray()
        
        try {
            val imagesDir = File(storagePath)
            if (!imagesDir.exists() || !imagesDir.isDirectory) {
                Log.w(TAG, "Directory does not exist: $storagePath")
                return createResult(images)
            }

            // Query MediaStore for images in the specified directory
            val projection = arrayOf(
                MediaStore.Images.Media._ID,
                MediaStore.Images.Media.DISPLAY_NAME,
                MediaStore.Images.Media.DATA,
                MediaStore.Images.Media.DATE_MODIFIED
            )

            val selection = "${MediaStore.Images.Media.DATA} LIKE ?"
            val selectionArgs = arrayOf("$storagePath%")
            val sortOrder = "${MediaStore.Images.Media.DATE_MODIFIED} DESC"

            val cursor: Cursor? = context.contentResolver.query(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                projection,
                selection,
                selectionArgs,
                sortOrder
            )

            cursor?.use {
                val idColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
                val nameColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DISPLAY_NAME)
                val dataColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATA)
                val dateColumn = it.getColumnIndexOrThrow(MediaStore.Images.Media.DATE_MODIFIED)

                while (it.moveToNext()) {
                    val id = it.getLong(idColumn)
                    val name = it.getString(nameColumn)
                    val path = it.getString(dataColumn)
                    val dateModified = it.getLong(dateColumn) * 1000 // Convert to milliseconds

                    // Get thumbnail using MediaStore
                    val thumbnail = getThumbnail(id)

                    val imageJson = JSONObject().apply {
                        put("id", id)
                        put("path", path)
                        put("filename", name)
                        put("thumbnail", thumbnail)
                        put("dateModified", dateModified)
                    }
                    images.put(imageJson)
                }
            }

            Log.d(TAG, "getGalleryImages: found ${images.length()} images")
        } catch (e: Exception) {
            Log.e(TAG, "getGalleryImages error", e)
        }

        return createResult(images)
    }

    @SuppressLint("Recycle")
    private fun getThumbnail(imageId: Long): String {
        return try {
            // Try to get cached thumbnail from MediaStore first
            val thumbnail = MediaStore.Images.Thumbnails.getThumbnail(
                context.contentResolver,
                imageId,
                MediaStore.Images.Thumbnails.MINI_KIND,
                null
            )

            if (thumbnail != null) {
                bitmapToBase64(thumbnail)
            } else {
                // Fallback: create thumbnail manually
                createThumbnailManually(imageId)
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to get thumbnail for imageId=$imageId", e)
            ""
        }
    }

    private fun createThumbnailManually(imageId: Long): String {
        val projection = arrayOf(MediaStore.Images.Media.DATA)
        val selection = "${MediaStore.Images.Media._ID} = ?"
        val selectionArgs = arrayOf(imageId.toString())

        context.contentResolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            projection,
            selection,
            selectionArgs,
            null
        )?.use { cursor ->
            if (cursor.moveToFirst()) {
                val path = cursor.getString(cursor.getColumnIndexOrThrow(MediaStore.Images.Media.DATA))
                val file = File(path)
                if (file.exists()) {
                    val options = BitmapFactory.Options().apply {
                        inJustDecodeBounds = true
                    }
                    BitmapFactory.decodeFile(path, options)

                    // Calculate sample size for thumbnail (~512px)
                    val sampleSize = calculateSampleSize(options.outWidth, options.outHeight, 512, 384)
                    options.inJustDecodeBounds = false
                    options.inSampleSize = sampleSize

                    val bitmap = BitmapFactory.decodeFile(path, options)
                    return bitmap?.let { bitmapToBase64(it) } ?: ""
                }
            }
        }
        return ""
    }

    private fun calculateSampleSize(width: Int, height: Int, reqWidth: Int, reqHeight: Int): Int {
        var sampleSize = 1
        if (height > reqHeight || width > reqWidth) {
            val halfHeight = height / 2
            val halfWidth = width / 2
            while (halfHeight / sampleSize >= reqHeight && halfWidth / sampleSize >= reqWidth) {
                sampleSize *= 2
            }
        }
        return sampleSize
    }

    private fun bitmapToBase64(bitmap: Bitmap): String {
        val outputStream = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.JPEG, THUMBNAIL_QUALITY, outputStream)
        val byteArray = outputStream.toByteArray()
        val base64 = Base64.encodeToString(byteArray, Base64.NO_WRAP)
        return "data:image/jpeg;base64,$base64"
    }

    private fun createResult(images: JSONArray): String {
        return JSONObject().apply {
            put("images", images)
        }.toString()
    }
}
```

**Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt
git commit -m "feat(android): add GalleryBridge for MediaStore image access"
```

---

## Task 7: Register GalleryBridge in MainActivity

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Add import**

Add after line 18:
```kotlin
import com.gjk.cameraftpcompanion.bridges.GalleryBridge
```

**Step 2: Add bridge field**

Add after line 35:
```kotlin
    private var galleryBridge: GalleryBridge? = null
```

**Step 3: Initialize bridge in onCreate**

Add after line 55:
```kotlin
        galleryBridge = GalleryBridge(this)
```

**Step 4: Register bridge in onWebViewCreate**

Add after line 72:
```kotlin
        addJsBridge(webView, galleryBridge, "GalleryAndroid")
```

**Step 5: Cleanup in onDestroy**

Add after line 162:
```kotlin
        galleryBridge = null
```

**Step 6: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "feat(android): register GalleryBridge in MainActivity"
```

---

## Task 8: Update global.ts result parsing

**Files:**
- Modify: `src/components/GalleryCard.tsx:48`

**Step 1: Fix JSON parsing for wrapped result**

The Kotlin code returns `{ "images": [...] }`, so update the parsing in GalleryCard.tsx:

Change line 48:
```typescript
// Before
      const parsed = JSON.parse(result) as GalleryImage[];
// After
      const response = JSON.parse(result) as { images: GalleryImage[] };
      const parsed = response.images;
```

**Step 2: Commit**

```bash
git add src/components/GalleryCard.tsx
git commit -m "fix(gallery): parse wrapped JSON response from GalleryBridge"
```

---

## Task 9: Build and Verify

**Step 1: Build Android**

```bash
./build.sh android
```

Expected: Build succeeds without errors

**Step 2: Manual testing checklist**

- [ ] Gallery tab appears in bottom navigation on Android
- [ ] Tapping gallery tab shows the gallery page
- [ ] Images load and display in 3-column grid
- [ ] Thumbnails appear correctly
- [ ] Tapping an image opens the system image chooser
- [ ] Refresh button reloads images
- [ ] Empty state shows when no images
- [ ] Loading state shows while fetching
- [ ] Error state shows on failure

---

## File Changes Summary

| File | Action |
|------|--------|
| `src/types/global.ts` | Modify - Add GalleryImage and GalleryAndroid types |
| `src/stores/configStore.ts` | Modify - Add 'gallery' to activeTab type |
| `src/components/BottomNav.tsx` | Modify - Add gallery tab button |
| `src/components/GalleryCard.tsx` | Create - Gallery page component |
| `src/App.tsx` | Modify - Integrate GalleryCard |
| `src-tauri/gen/android/.../bridges/GalleryBridge.kt` | Create - Android MediaStore bridge |
| `src-tauri/gen/android/.../MainActivity.kt` | Modify - Register GalleryBridge |
