# Gallery Refresh on Activity Resume

## Summary

When users delete images in Android's native ImageViewerActivity and return to the gallery, deleted images still appear in the list because the WebView (running the gallery UI) is paused and cannot receive refresh events.

This spec adds automatic gallery refresh when MainActivity resumes, ensuring the gallery list is always up-to-date.

## Problem

**Current Flow:**
```
Gallery (WebView) → ImageViewerActivity (Native)
                         ↓
                   Delete image
                         ↓
                   Send refresh event
                         ↓
                   WebView is paused, cannot receive event
                         ↓
                   Return to Gallery
                         ↓
                   Deleted image still shows ❌
```

## Solution

**Add `onResume()` lifecycle callback in MainActivity that dispatches refresh events to the WebView.**

**New Flow:**
```
Gallery (WebView) → ImageViewerActivity (Native)
                         ↓
                   Delete image
                         ↓
                   Return to Gallery
                         ↓
                   MainActivity.onResume()
                         ↓
                   Dispatch gallery-refresh-requested event
                         ↓
                   GalleryCard receives event, calls handleRefresh()
                         ↓
                   Deleted image removed from list ✓
```

## Implementation

### 1. MainActivity.kt

Add `onResume()` lifecycle callback:

```kotlin
override fun onResume() {
    super.onResume()
    // Notify WebView to refresh gallery (may be returning from ImageViewerActivity after deletion)
    val refreshPayload = "{\"reason\":\"activity-resume\",\"timestamp\":${System.currentTimeMillis()}}"
    emitWindowEvent("gallery-refresh-requested", refreshPayload)
    emitWindowEvent("latest-photo-refresh-requested", refreshPayload)
}
```

**Location:** `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

### 2. GalleryCard.tsx

Add event listener for `gallery-refresh-requested`:

```typescript
// Listen for gallery refresh events (from MainActivity.onResume or ImageViewerActivity post-deletion)
useEffect(() => {
  const handleGalleryRefresh = () => {
    void handleRefresh();
  };
  window.addEventListener('gallery-refresh-requested', handleGalleryRefresh);
  return () => {
    window.removeEventListener('gallery-refresh-requested', handleGalleryRefresh);
  };
}, [handleRefresh]);
```

**Location:** `src/components/GalleryCard.tsx`

### 3. gallery-refresh.ts

Add new refresh reason type:

```typescript
export type MediaLibraryRefreshReason =
  | 'manual'
  | 'upload'
  | 'delete'
  | 'permission-granted'
  | 'media-store-ready'
  | 'activity-resume';  // New
```

**Location:** `src/utils/gallery-refresh.ts`

## Trade-offs

| Aspect | Assessment |
|--------|------------|
| **Simplicity** | ✅ Leverages Android lifecycle, minimal code |
| **Accuracy** | ⚠️ Refreshes every resume, even without deletions |
| **Performance** | ✅ Refresh is fast, uses existing pagination |
| **Reliability** | ✅ Guaranteed refresh when returning to gallery |

## Files Changed

1. `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` - Add `onResume()`
2. `src/components/GalleryCard.tsx` - Add event listener
3. `src/utils/gallery-refresh.ts` - Add `activity-resume` reason

## Testing

1. Open gallery, tap an image to open ImageViewerActivity
2. Delete the image in ImageViewerActivity
3. Press back to return to gallery
4. Verify: deleted image is no longer in the list
5. Also test: returning without deleting should still work (refresh is harmless)
