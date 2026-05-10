# Image Viewer Menu & Color Grading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an overflow menu button to the Android native image viewer bottom bar, containing AI Edit and Color Grading options, with a WebView overlay color grading dialog.

**Architecture:** Replace the existing `btn_ai_edit` ImageButton with a `btn_menu` overflow button. Tapping it shows a `PopupWindow` with two items. Color Grading opens a WebView overlay dialog (same pattern as AI Edit). Processing is triggered via JS bridge to the main WebView.

**Tech Stack:** Kotlin (Android Native), WebView inline HTML/CSS/JS, Tauri IPC (via JS `invoke()`)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `res/drawable/ic_menu_overflow.xml` | Create | Three-dot vertical overflow icon |
| `res/drawable/ic_color_grading.xml` | Create | Palette icon for color grading menu item |
| `res/drawable/menu_popup_bg.xml` | Create | Dark rounded background for popup |
| `res/layout/popup_image_menu.xml` | Create | Popup menu layout with 2 items |
| `res/layout/activity_image_viewer.xml` | Modify | Replace `btn_ai_edit` with `btn_menu` |
| `res/layout-land/activity_image_viewer.xml` | Modify | Same change for landscape |
| `ImageViewerActivity.kt` | Modify | Menu popup, color grading dialog, RAW detection, updated bindings |
| `src/types/global.ts` | Modify | Add `__tauriTriggerColorGrading` type declaration |
| `src/App.tsx` | Modify | Register `__tauriTriggerColorGrading` handler |

---

### Task 1: Create drawable resources

**Files:**
- Create: `src-tauri/gen/android/app/src/main/res/drawable/ic_menu_overflow.xml`
- Create: `src-tauri/gen/android/app/src/main/res/drawable/ic_color_grading.xml`
- Create: `src-tauri/gen/android/app/src/main/res/drawable/menu_popup_bg.xml`

- [ ] **Step 1: Create overflow menu icon**

Create `src-tauri/gen/android/app/src/main/res/drawable/ic_menu_overflow.xml`:

```xml
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:fillColor="#D1D5DB"
        android:pathData="M12,5m-2,0a2,2 0,1 1,4 0a2,2 0,1 1,-4 0" />
    <path
        android:fillColor="#D1D5DB"
        android:pathData="M12,12m-2,0a2,2 0,1 1,4 0a2,2 0,1 1,-4 0" />
    <path
        android:fillColor="#D1D5DB"
        android:pathData="M12,19m-2,0a2,2 0,1 1,4 0a2,2 0,1 1,-4 0" />
</vector>
```

- [ ] **Step 2: Create color grading palette icon**

Create `src-tauri/gen/android/app/src/main/res/drawable/ic_color_grading.xml`:

```xml
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:strokeColor="#D1D5DB"
        android:strokeWidth="2"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:fillColor="#00000000"
        android:pathData="M2.5,2.5l6.3,14.2c0.2,0.5 0.8,0.7 1.3,0.5l2.4,-1c0.5,-0.2 0.7,-0.8 0.5,-1.3L6.7,0.7" />
    <path
        android:strokeColor="#D1D5DB"
        android:strokeWidth="2"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:fillColor="#00000000"
        android:pathData="M12.8,7.2c1.6,-1.6 4.1,-1.6 5.7,0c1.6,1.6 1.6,4.1 0,5.7" />
    <path
        android:strokeColor="#D1D5DB"
        android:strokeWidth="2"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:fillColor="#00000000"
        android:pathData="M15.5,4.5c2.9,-1.3 6.2,0 7.5,2.9c1.3,2.9 0,6.2 -2.9,7.5" />
    <path
        android:strokeColor="#D1D5DB"
        android:strokeWidth="2"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:fillColor="#00000000"
        android:pathData="M6.7,0.7l-4.2,1.8" />
</vector>
```

- [ ] **Step 3: Create popup menu background**

Create `src-tauri/gen/android/app/src/main/res/drawable/menu_popup_bg.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="#1F2937" />
    <corners android:radius="12dp" />
</shape>
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/gen/android/app/src/main/res/drawable/ic_menu_overflow.xml \
        src-tauri/gen/android/app/src/main/res/drawable/ic_color_grading.xml \
        src-tauri/gen/android/app/src/main/res/drawable/menu_popup_bg.xml
git commit -m "feat(android): add drawable resources for image viewer menu"
```

---

### Task 2: Create popup menu layout

**Files:**
- Create: `src-tauri/gen/android/app/src/main/res/layout/popup_image_menu.xml`

- [ ] **Step 1: Create the popup menu layout**

Create `src-tauri/gen/android/app/src/main/res/layout/popup_image_menu.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
    android:id="@+id/menu_container"
    android:layout_width="wrap_content"
    android:layout_height="wrap_content"
    android:background="@drawable/menu_popup_bg"
    android:orientation="vertical"
    android:paddingVertical="6dp"
    android:minWidth="168dp">

    <LinearLayout
        android:id="@+id/menu_item_ai_edit"
        android:layout_width="match_parent"
        android:layout_height="44dp"
        android:orientation="horizontal"
        android:gravity="center_vertical"
        android:paddingHorizontal="14dp"
        android:clickable="true"
        android:focusable="true"
        android:background="?android:attr/selectableItemBackground">

        <ImageView
            android:layout_width="20dp"
            android:layout_height="20dp"
            android:src="@drawable/ic_ai_edit"
            android:importantForAccessibility="no" />

        <TextView
            android:layout_width="wrap_content"
            android:layout_height="wrap_content"
            android:text="AI修图"
            android:textColor="#E5E7EB"
            android:textSize="15sp"
            android:layout_marginStart="10dp" />
    </LinearLayout>

    <LinearLayout
        android:id="@+id/menu_item_color_grading"
        android:layout_width="match_parent"
        android:layout_height="44dp"
        android:orientation="horizontal"
        android:gravity="center_vertical"
        android:paddingHorizontal="14dp"
        android:clickable="true"
        android:focusable="true"
        android:background="?android:attr/selectableItemBackground">

        <ImageView
            android:id="@+id/menu_item_color_grading_icon"
            android:layout_width="20dp"
            android:layout_height="20dp"
            android:src="@drawable/ic_color_grading"
            android:importantForAccessibility="no" />

        <TextView
            android:id="@+id/menu_item_color_grading_text"
            android:layout_width="wrap_content"
            android:layout_height="wrap_content"
            android:text="调色"
            android:textColor="#E5E7EB"
            android:textSize="15sp"
            android:layout_marginStart="10dp" />
    </LinearLayout>

</LinearLayout>
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/res/layout/popup_image_menu.xml
git commit -m "feat(android): add popup menu layout for image viewer actions"
```

---

### Task 3: Update layout XMLs — replace AI Edit button with menu button

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/res/layout/activity_image_viewer.xml:164-174`
- Modify: `src-tauri/gen/android/app/src/main/res/layout-land/activity_image_viewer.xml:172-182`

- [ ] **Step 1: Update portrait layout**

In `src-tauri/gen/android/app/src/main/res/layout/activity_image_viewer.xml`, replace the `btn_ai_edit` ImageButton (lines 164-174) with:

```xml
            <ImageButton
                android:id="@+id/btn_menu"
                android:layout_width="44dp"
                android:layout_height="44dp"
                android:src="@drawable/ic_menu_overflow"
                android:background="@drawable/btn_circle_bg"
                android:contentDescription="菜单"
                android:padding="10dp"
                android:scaleType="fitCenter"
                android:layout_marginStart="4dp"
                android:layout_marginEnd="4dp" />
```

- [ ] **Step 2: Update landscape layout**

In `src-tauri/gen/android/app/src/main/res/layout-land/activity_image_viewer.xml`, replace the `btn_ai_edit` ImageButton (lines 172-182) with the same snippet:

```xml
            <ImageButton
                android:id="@+id/btn_menu"
                android:layout_width="44dp"
                android:layout_height="44dp"
                android:src="@drawable/ic_menu_overflow"
                android:background="@drawable/btn_circle_bg"
                android:contentDescription="菜单"
                android:padding="10dp"
                android:scaleType="fitCenter"
                android:layout_marginStart="4dp"
                android:layout_marginEnd="4dp" />
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/res/layout/activity_image_viewer.xml \
        src-tauri/gen/android/app/src/main/res/layout-land/activity_image_viewer.xml
git commit -m "feat(android): replace AI edit button with overflow menu button"
```

---

### Task 4: Add frontend JS handler for color grading trigger

**Files:**
- Modify: `src/types/global.ts:287` (add type declaration before closing `}`)
- Modify: `src/App.tsx:84-90` (add handler registration and cleanup)

- [ ] **Step 1: Add type declaration**

In `src/types/global.ts`, before the closing `}` of the `Window` interface (line 287), add:

```typescript
    /**
     * Triggers color grading for a single file.
     * Called by native ImageViewerActivity after user confirms the color grading dialog.
     */
    __tauriTriggerColorGrading?: (filePath: string, lutId: string) => Promise<void>;
```

- [ ] **Step 2: Register handler in App.tsx**

In `src/App.tsx`, after line 83 (`w.__tauriCancelAiEdit = async () => { ... }`) and before the `return () => {` cleanup, add:

```typescript
    w.__tauriTriggerColorGrading = async (filePath: string, lutId: string) => {
      const { enqueueColorGrading } = await import('./hooks/useColorGradingProgress');
      await enqueueColorGrading([filePath], lutId);
    };
```

- [ ] **Step 3: Add cleanup**

In the `return () => { ... }` cleanup block in the same `useEffect`, add after `delete w.__tauriCancelAiEdit;`:

```typescript
      delete w.__tauriTriggerColorGrading;
```

- [ ] **Step 4: Commit**

```bash
git add src/types/global.ts src/App.tsx
git commit -m "feat: add __tauriTriggerColorGrading JS handler for native Android"
```

---

### Task 5: Update ImageViewerActivity — bindings, RAW detection, menu popup, color grading dialog

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt`

This is the largest task. It modifies `ImageViewerActivity.kt` with several distinct changes.

- [ ] **Step 1: Update field declarations (lines 156-174)**

Replace `btnAiEdit` declaration with `btnMenu`, add popup and color grading fields:

```kotlin
    // Change line 156:
    // FROM: private lateinit var btnAiEdit: ImageButton
    // TO:
    private lateinit var btnMenu: ImageButton
```

Add after line 174 (`private var promptWebView: WebView? = null`):

```kotlin
    private var menuPopupWindow: android.widget.PopupWindow? = null
    private var colorGradingWebView: WebView? = null
```

- [ ] **Step 2: Update bindViews() (line 210)**

Replace `btnAiEdit = findViewById(R.id.btn_ai_edit)` with:

```kotlin
        btnMenu = findViewById(R.id.btn_menu)
```

- [ ] **Step 3: Add RAW detection helper**

Add this method anywhere in the class body (e.g., after `resolveUriToFilePath` around line 918):

```kotlin
    private fun isRawImage(uriString: String): Boolean {
        val uri = Uri.parse(uriString)
        val mimeType = contentResolver.getType(uri)
        return mimeType?.startsWith("image/x-") == true
    }
```

- [ ] **Step 4: Replace setupButtons() (lines 308-329)**

Replace the entire `setupButtons()` method with:

```kotlin
    private fun setupButtons() {
        btnMenu.setOnClickListener {
            showImageMenu()
        }

        btnRotate.setOnClickListener {
            isLandscape = !isLandscape
            requestedOrientation = if (isLandscape) {
                ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
            } else {
                ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
            }
        }

        btnDelete.setOnClickListener {
            if (uris.isNotEmpty()) {
                deleteCurrentImage()
            }
        }
    }
```

- [ ] **Step 5: Add showImageMenu() method**

Add after `setupButtons()`:

```kotlin
    private fun showImageMenu() {
        menuPopupWindow?.dismiss()

        val popupView = layoutInflater.inflate(R.layout.popup_image_menu, null)
        val menuItemAiEdit = popupView.findViewById<LinearLayout>(R.id.menu_item_ai_edit)
        val menuItemColorGrading = popupView.findViewById<LinearLayout>(R.id.menu_item_color_grading)
        val cgIcon = popupView.findViewById<android.widget.ImageView>(R.id.menu_item_color_grading_icon)
        val cgText = popupView.findViewById<TextView>(R.id.menu_item_color_grading_text)

        val isRaw = uris.isNotEmpty() && currentIndex in uris.indices && isRawImage(uris[currentIndex])

        if (!isRaw) {
            menuItemColorGrading.isEnabled = false
            menuItemColorGrading.isClickable = false
            cgIcon.alpha = 0.35f
            cgText.alpha = 0.35f
        }

        menuItemAiEdit.setOnClickListener {
            menuPopupWindow?.dismiss()
            if (uris.isNotEmpty() && currentIndex in uris.indices) {
                triggerAiEditForCurrentImage()
            }
        }

        menuItemColorGrading.setOnClickListener {
            menuPopupWindow?.dismiss()
            if (uris.isNotEmpty() && currentIndex in uris.indices) {
                triggerColorGradingForCurrentImage()
            }
        }

        val popup = android.widget.PopupWindow(
            popupView,
            android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
            true
        )
        popup.setBackgroundDrawable(null)
        popup.isOutsideTouchable = true
        popup.isFocusable = true
        popup.elevation = 16f

        popup.setOnDismissListener {
            menuPopupWindow = null
        }

        menuPopupWindow = popup
        popup.showAsDropDown(btnMenu, 0, -(btnMenu.height + popupView.layoutParams.height + 8.dpToPx()))
    }
```

Note: `8.dpToPx()` uses the existing `dpToPx()` extension at line 304. The negative Y offset positions the popup above the button.

However, since the popup's height isn't known before layout, we need to use a post-layout approach. Replace the last two lines with a safer approach:

```kotlin
        menuPopupWindow = popup
        popupView.measure(
            android.view.View.MeasureSpec.UNSPECIFIED,
            android.view.View.MeasureSpec.UNSPECIFIED
        )
        val yOffset = -(btnMenu.height + popupView.measuredHeight + 8.dpToPx())
        popup.showAsDropDown(btnMenu, 0, yOffset)
    }
```

- [ ] **Step 6: Add triggerColorGradingForCurrentImage() and showColorGradingOverlay()**

Add these methods after `showImageMenu()`:

```kotlin
    private fun triggerColorGradingForCurrentImage() {
        val uriString = uris.getOrNull(currentIndex) ?: return
        val filePath = resolveUriToFilePath(uriString)

        if (filePath == null) {
            Log.w(TAG, "Cannot resolve file path for URI: $uriString")
            return
        }

        showColorGradingOverlay(filePath)
    }

    private fun showColorGradingOverlay(filePath: String) {
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LOCKED
        val rootView = findViewById<FrameLayout>(android.R.id.content)

        dismissColorGradingWebView()

        val presets = listOf(
            "acros" to "ACROS",
            "astia" to "Astia",
            "classic-chrome" to "Classic Chrome",
            "classic-neg" to "Classic Neg",
            "eterna" to "ETERNA",
            "eterna-bb" to "ETERNA Bleach Bypass",
            "pro-neg-std" to "PRO Neg. Std",
            "provia" to "Provia",
            "reala-ace" to "REALA ACE",
            "velvia" to "Velvia",
            "flog2c-709" to "F-Log2C \u2192 Rec.709",
        )
        val firstId = presets.first().first
        val firstLabel = presets.first().second
        val presetOptionsHtml = presets.joinToString("") { (value, label) ->
            """<div class="dropdown-opt${if (value == firstId) " selected" else ""}" data-value="$value">$label</div>"""
        }

        val html = """
            <!DOCTYPE html>
            <html>
            <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1">
            <style>
              * { margin: 0; padding: 0; box-sizing: border-box; }
              body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; }
              .overlay {
                position: fixed; inset: 0;
                background: rgba(0,0,0,0.5);
                display: flex; align-items: center; justify-content: center;
                padding: 16px; z-index: 50;
              }
              .card {
                background: #fff; border-radius: 12px; width: 100%; max-width: 448px;
                box-shadow: 0 25px 50px -12px rgba(0,0,0,0.25);
                display: flex; flex-direction: column; max-height: 90vh;
              }
              .header {
                display: flex; align-items: center; justify-content: space-between;
                padding: 16px; border-bottom: 1px solid #e5e7eb;
              }
              .title-group { display: flex; flex-direction: column; }
              .title { font-size: 18px; font-weight: 600; color: #111827; }
              .subtitle { font-size: 14px; color: #6b7280; margin-top: 2px; }
              .close-btn {
                padding: 8px; border: none; background: none; cursor: pointer;
                color: #9ca3af; border-radius: 8px;
              }
              .close-btn:hover { color: #4b5563; background: #f3f4f6; }
              .close-btn svg { width: 20px; height: 20px; }
              .content { padding: 16px; overflow-y: auto; }
              .field-group { margin-bottom: 0; }
              .field-label { font-size: 14px; font-weight: 500; color: #374151; margin-bottom: 8px; }
              .dropdown { position: relative; }
              .dropdown-btn {
                width: 100%; padding: 8px 12px; border: 1px solid #e5e7eb;
                border-radius: 8px; font-size: 14px; color: #374151;
                background: #fff; outline: none; cursor: pointer;
                display: flex; align-items: center; justify-content: space-between;
                text-align: left; -webkit-user-select: none; user-select: none;
                -webkit-tap-highlight-color: transparent;
              }
              .dropdown-btn:hover { border-color: #d1d5db; }
              .dropdown-btn .chevron {
                width: 16px; height: 16px; color: #9ca3af;
                transition: transform 0.2s; flex-shrink: 0;
              }
              .dropdown-btn.open .chevron { transform: rotate(180deg); }
              .dropdown-panel {
                position: absolute; left: 0; right: 0;
                margin-top: 4px; background: #fff; border: 1px solid #e5e7eb;
                border-radius: 8px; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.1), 0 4px 6px -4px rgba(0,0,0,0.1);
                padding: 4px 0; z-index: 10; max-height: 240px; overflow-y: auto;
                opacity: 0; transform: scaleY(0.95) translateY(-4px);
                transform-origin: top; pointer-events: none;
                transition: opacity 0.15s ease, transform 0.15s ease;
              }
              .dropdown-panel.open {
                opacity: 1; transform: scaleY(1) translateY(0);
                pointer-events: auto;
              }
              .dropdown-opt {
                padding: 8px 12px; font-size: 14px;
                color: #374151; cursor: pointer;
                -webkit-tap-highlight-color: transparent;
              }
              .dropdown-opt:hover { background: #f9fafb; }
              .dropdown-opt.selected { background: #f5f3ff; color: #7c3aed; font-weight: 500; }
              .footer {
                display: flex; align-items: center; justify-content: flex-end;
                padding: 16px; border-top: 1px solid #e5e7eb; gap: 8px;
              }
              .btn {
                padding: 8px 16px; border-radius: 8px; font-size: 14px;
                font-weight: 500; border: none; cursor: pointer;
              }
              .btn-cancel { background: #f3f4f6; color: #374151; }
              .btn-cancel:hover { background: #e5e7eb; }
              .btn-confirm { background: #7c3aed; color: #fff; }
              .btn-confirm:hover { background: #6d28d9; }
              .header-icon { color: #7c3aed; flex-shrink: 0; }
            </style>
            </head>
            <body>
            <div class="overlay" onclick="if(event.target===this)NativeBridge.onCancel()">
              <div class="card">
                <div class="header">
                  <div style="display:flex;align-items:center;gap:12px">
                    <div style="width:40px;height:40px;background:#f5f3ff;border-radius:8px;display:flex;align-items:center;justify-content:center"><svg class="header-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="2.5"/><circle cx="17.5" cy="10.5" r="2.5"/><circle cx="8.5" cy="7.5" r="2.5"/><circle cx="6.5" cy="12.5" r="2.5"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.9 0 1.5-.7 1.5-1.5 0-.4-.1-.7-.4-1-.3-.3-.4-.7-.4-1.1 0-.8.7-1.5 1.5-1.5H16c3.3 0 6-2.7 6-6 0-5.5-4.5-9-10-9Z"/></svg></div>
                    <div class="title-group">
                      <div class="title">调色</div>
                      <div class="subtitle">使用胶片模拟调色处理 RAW 照片</div>
                    </div>
                  </div>
                  <button class="close-btn" onclick="NativeBridge.onCancel()">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                  </button>
                </div>
                <div class="content">
                  <div class="field-group">
                    <div class="field-label">调色预设</div>
                    <div class="dropdown" id="presetDropdown">
                      <button class="dropdown-btn" type="button" onclick="toggleDropdown()">
                        <span id="presetLabel">$firstLabel</span>
                        <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                      </button>
                      <div class="dropdown-panel" id="presetPanel">$presetOptionsHtml</div>
                    </div>
                  </div>
                </div>
                <div class="footer">
                  <button class="btn btn-cancel" onclick="NativeBridge.onCancel()">取消</button>
                  <button class="btn btn-confirm" onclick="onConfirm()">应用</button>
                </div>
              </div>
            </div>
            <script>
              var selectedPreset = '$firstId';
              function toggleDropdown() {
                var panel = document.getElementById('presetPanel');
                var btn = panel.previousElementSibling;
                var isOpen = panel.classList.contains('open');
                if (isOpen) {
                  panel.classList.remove('open');
                  btn.classList.remove('open');
                } else {
                  panel.classList.add('open');
                  btn.classList.add('open');
                }
              }
              function closeDropdown() {
                var panel = document.getElementById('presetPanel');
                var btn = panel.previousElementSibling;
                panel.classList.remove('open');
                btn.classList.remove('open');
              }
              document.getElementById('presetPanel').addEventListener('click', function(e) {
                var opt = e.target.closest('.dropdown-opt');
                if (!opt) return;
                selectedPreset = opt.getAttribute('data-value');
                document.getElementById('presetLabel').textContent = opt.textContent;
                var allOpts = this.querySelectorAll('.dropdown-opt');
                for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
                opt.classList.add('selected');
                closeDropdown();
              });
              document.addEventListener('click', function(e) {
                if (!document.getElementById('presetDropdown').contains(e.target)) {
                  closeDropdown();
                }
              });
              function onConfirm() {
                NativeBridge.onConfirm(selectedPreset);
              }
            </script>
            </body>
            </html>
        """.trimIndent()

        val webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            setBackgroundColor(0)
            isVerticalScrollBarEnabled = false
            isHorizontalScrollBarEnabled = false
            addJavascriptInterface(object {
                @JavascriptInterface
                fun onConfirm(lutId: String) {
                    runOnUiThread {
                        dismissColorGradingWebView()
                        dispatchColorGrading(filePath, lutId)
                    }
                }
                @JavascriptInterface
                fun onCancel() {
                    runOnUiThread { dismissColorGradingWebView() }
                }
            }, "NativeBridge")
            loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        }

        val overlayParams = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT,
            FrameLayout.LayoutParams.MATCH_PARENT
        )
        rootView.addView(webView, overlayParams)
        colorGradingWebView = webView
    }

    private fun dismissColorGradingWebView() {
        colorGradingWebView?.let {
            (it.parent as? FrameLayout)?.removeView(it)
            it.destroy()
        }
        colorGradingWebView = null
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    }

    private fun dispatchColorGrading(filePath: String, lutId: String) {
        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available for color grading")
            return
        }

        val escapedFilePath = filePath.replace("\\", "\\\\").replace("'", "\\'")
        val escapedLutId = lutId.replace("\\", "\\\\").replace("'", "\\'")
        val js = """
            (function(){
                if(window.__tauriTriggerColorGrading){
                    window.__tauriTriggerColorGrading('$escapedFilePath','$escapedLutId');
                    return 'ok';
                }
                return 'no_handler';
            })();
        """.trimIndent()

        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                if (result?.trim()?.removeSurrounding("\"") == "no_handler") {
                    runOnUiThread {
                        Log.w(TAG, "Color grading failed: frontend handler not available")
                    }
                }
            }
        }
    }
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt
git commit -m "feat(android): add overflow menu with AI edit and color grading to image viewer"
```

---

### Task 6: Build and verify

**Files:** None (verification only)

- [ ] **Step 1: Build both platforms**

Run: `./build.sh windows android`
Expected: Both Windows and Android builds succeed.

- [ ] **Step 2: Verify no regressions**

Check the build output for any warnings or errors related to the changed files. If there are compile errors, fix them.

- [ ] **Step 3: Commit any fixes if needed**

If fixes were required, commit with an appropriate message.

---

## Self-Review

**Spec coverage:**
- ✅ Menu button replaces AI edit button in bottom bar → Task 3
- ✅ Popup with AI Edit and Color Grading options → Task 2 + Task 5
- ✅ Color Grading disabled for non-RAW → Task 5 (alpha reduction + not clickable)
- ✅ RAW detection via ContentResolver MIME type → Task 5 (`isRawImage()`)
- ✅ Color grading WebView overlay dialog → Task 5 (`showColorGradingOverlay()`)
- ✅ Preset dropdown matching WebView version → Task 5 (11 presets)
- ✅ Frontend JS handler for processing trigger → Task 4
- ✅ Both portrait and landscape layouts → Task 3
- ✅ Dialog style consistent with AI Edit (white card, same CSS patterns) → Task 5

**Placeholder scan:** No TBD, TODO, or incomplete sections found.

**Type consistency:**
- `__tauriTriggerColorGrading(filePath: string, lutId: string)` — declared in `global.ts`, registered in `App.tsx`, called from `ImageViewerActivity.kt` via `evaluateJavascript`. All match.
- `enqueueColorGrading([filePath], lutId)` — matches the existing signature in `useColorGradingProgress.ts:136`.
- `btnMenu` used consistently in layout XMLs and Kotlin code.
- `isRawImage()` uses same `image/x-` prefix as frontend.
