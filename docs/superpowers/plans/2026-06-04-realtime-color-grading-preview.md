# Real-time Color Grading Preview — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real-time color grading preview screen to the Android ImageViewerActivity, replacing the current dialog-only flow with a full-screen Activity that shows live LUT/exposure changes.

**Architecture:** New `ColorGradingActivity` hosts a full-screen WebView loading `color_grading_preview.html`. IPC uses the existing forwarding pattern: WebView → `@JavascriptInterface` → Kotlin `evaluateJavascript` → MainActivity's Tauri WebView → `invoke()` → Rust. Image display uses `shouldInterceptRequest` with custom `preview://` scheme to stream JPEG files directly via `FileInputStream`.

**Tech Stack:** Kotlin, Android WebView, HTML/CSS/JS, Tauri IPC, Rust (existing commands only)

**Design Spec:** `docs/superpowers/specs/2026-06-04-realtime-color-grading-preview-design.md`

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| **Create** | `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt` | Activity + WebView + NativeBridge + shouldInterceptRequest |
| **Create** | `src-tauri/gen/android/app/src/main/assets/color_grading_preview.html` | Full-screen dark-themed color grading UI with state machine |
| **Modify** | `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt:474-522` | Replace `triggerColorGradingForCurrentImage()` to launch new Activity |
| **Modify** | `src-tauri/gen/android/app/src/main/AndroidManifest.xml:74-80` | Register `ColorGradingActivity` |
| **Modify** | `src/App.tsx:79-99,110-121` | Add 4 new global JS functions + cleanup |
| **Modify** | `src/types/global.ts:377-378` | Add type declarations for new global functions |

---

### Task 1: Register ColorGradingActivity in AndroidManifest

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/AndroidManifest.xml:80` (after ImageViewerActivity block)

- [ ] **Step 1: Add activity registration**

Insert after the ImageViewerActivity `</activity>` closing tag (line 80):

```xml
        <!-- Color Grading Preview Activity -->
        <activity
            android:name=".ColorGradingActivity"
            android:configChanges="orientation|screenSize|smallestScreenSize|density|keyboard|keyboardHidden|navigation"
            android:exported="false"
            android:theme="@style/Theme.MaterialComponents.DayNight.NoActionBar"
            android:windowSoftInputMode="adjustNothing" />
```

Use the same `configChanges` and theme as ImageViewerActivity for consistency.

- [ ] **Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/AndroidManifest.xml
git commit -m "feat: register ColorGradingActivity in AndroidManifest"
```

---

### Task 2: Add global JS functions in App.tsx

**Files:**
- Modify: `src/App.tsx:79-99` (add new functions after existing `__tauriCancelColorGrading`)
- Modify: `src/App.tsx:110-121` (add cleanup for new functions)

- [ ] **Step 1: Add 4 new global JS functions**

In `src/App.tsx`, after the existing `__tauriCancelColorGrading` function (line 108), add:

```typescript
    w.__tauriBeginColorGradingPreview = async (filePath: string) => {
      await invoke('begin_color_grading_preview', { imagePath: filePath });
    };

    w.__tauriApplyColorGradingPreview = async (lutId: string, meteringMode: string, evOffset: number) => {
      return await invoke<string>('apply_color_grading_preview', {
        lutId, meteringMode, evOffset,
        enableLensCorrection: true,
      });
    };

    w.__tauriEndColorGradingPreview = async () => {
      await invoke('end_color_grading_preview');
    };

    w.__tauriSaveColorGradingLastUsed = (lutId: string, meteringMode: string, evOffset: number) => {
      updateDraft(d => ({
        ...d,
        colorGradingLastUsed: { presetId: lutId, meteringMode, evOffset },
      }));
    };
```

- [ ] **Step 2: Add cleanup for the 4 new functions**

In the cleanup return function (after line 120 `delete w.__tauriCancelColorGrading;`), add:

```typescript
      delete w.__tauriBeginColorGradingPreview;
      delete w.__tauriApplyColorGradingPreview;
      delete w.__tauriEndColorGradingPreview;
      delete w.__tauriSaveColorGradingLastUsed;
```

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat: add global JS functions for real-time color grading preview IPC"
```

---

### Task 3: Add type declarations in global.ts

**Files:**
- Modify: `src/types/global.ts:377-378` (before `__requestExifForPositions`)

- [ ] **Step 1: Add type declarations**

Insert before line 379 (`__requestExifForPositions` declaration):

```typescript
    /**
     * Begins a color grading preview session by decoding the RAW file.
     * Called by native ColorGradingActivity to start real-time preview.
     */
    __tauriBeginColorGradingPreview?: (filePath: string) => Promise<void>;

    /**
     * Applies color grading parameters to the current preview session.
     * Returns the local file path of the generated JPEG preview.
     * Called by native ColorGradingActivity on each parameter change.
     */
    __tauriApplyColorGradingPreview?: (lutId: string, meteringMode: string, evOffset: number) => Promise<string>;

    /**
     * Ends the current color grading preview session, cleaning up resources.
     * Called by native ColorGradingActivity when leaving or saving.
     */
    __tauriEndColorGradingPreview?: () => Promise<void>;

    /**
     * Saves the color grading parameters as the last-used defaults.
     * Called by native ColorGradingActivity when saving.
     */
    __tauriSaveColorGradingLastUsed?: (lutId: string, meteringMode: string, evOffset: number) => void;
```

- [ ] **Step 2: Commit**

```bash
git add src/types/global.ts
git commit -m "feat: add type declarations for color grading preview IPC functions"
```

---

### Task 4: Create color_grading_preview.html

**Files:**
- Create: `src-tauri/gen/android/app/src/main/assets/color_grading_preview.html`

- [ ] **Step 1: Create the full-screen color grading UI**

Create the file with the following complete content. This is a self-contained HTML page with dark theme, state machine (LOADING → READY → ADJUSTING → SAVING), dropdown menus for LUT/metering, EV slider, and NativeBridge integration:

```html
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #111; color: #e0e0e0; height: 100vh; overflow: hidden; display: flex; flex-direction: column; }

  /* Top bar */
  .top-bar {
    display: flex; align-items: center; padding: 12px 16px;
    border-bottom: 1px solid #333; flex-shrink: 0; min-height: 48px;
  }
  .back-btn {
    color: #fff; font-size: 16px; cursor: pointer; display: flex;
    align-items: center; gap: 4px; background: none; border: none;
    padding: 4px 8px; border-radius: 8px; -webkit-user-select: none;
  }
  .back-btn:active { background: rgba(255,255,255,0.1); }
  .back-btn.disabled { color: #555; pointer-events: none; }
  .top-bar-title { flex: 1; text-align: center; font-weight: 600; font-size: 17px; }
  .top-bar-spacer { width: 60px; }

  /* Preview area */
  .preview-area {
    flex: 1; display: flex; align-items: center; justify-content: center;
    overflow: hidden; position: relative; min-height: 0;
  }
  .preview-area img {
    max-width: 100%; max-height: 100%; object-fit: contain;
  }

  /* Loading overlay */
  .loading-overlay {
    position: absolute; inset: 0; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 16px;
    background: #111; z-index: 10;
  }
  .loading-overlay.hidden { display: none; }
  .spinner {
    width: 48px; height: 48px; border: 3px solid #333;
    border-top-color: #a78bfa; border-radius: 50%;
    animation: spin 1s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .loading-text { color: #999; font-size: 14px; }
  .loading-hint { color: #666; font-size: 12px; }

  /* Error overlay */
  .error-overlay {
    position: absolute; inset: 0; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 16px;
    background: #111; z-index: 10;
  }
  .error-overlay.hidden { display: none; }
  .error-text { color: #f87171; font-size: 14px; text-align: center; padding: 0 32px; }
  .error-actions { display: flex; gap: 12px; }
  .btn-error {
    padding: 10px 20px; border-radius: 8px; border: none; font-size: 14px;
    font-weight: 500; cursor: pointer;
  }
  .btn-error-retry { background: #a78bfa; color: #fff; }
  .btn-error-back { background: #333; color: #ccc; }

  /* Controls panel */
  .controls {
    padding: 16px; background: #1a1a1a; border-top: 1px solid #333;
    flex-shrink: 0;
  }
  .controls.disabled { opacity: 0.3; pointer-events: none; }
  .controls-row { display: flex; gap: 8px; margin-bottom: 12px; }
  .dropdown { position: relative; flex: 1; }
  .dropdown-btn {
    width: 100%; padding: 10px 12px; border: 1px solid #333;
    border-radius: 8px; font-size: 13px; color: #e0e0e0;
    background: #252525; outline: none; cursor: pointer;
    display: flex; align-items: center; justify-content: space-between;
    text-align: left; -webkit-user-select: none;
  }
  .dropdown-btn .chevron {
    width: 14px; height: 14px; color: #666;
    transition: transform 0.2s; flex-shrink: 0;
  }
  .dropdown-btn.open .chevron { transform: rotate(180deg); }
  .dropdown-panel {
    position: absolute; left: 0; right: 0; bottom: 100%;
    margin-bottom: 4px; background: #2a2a2a; border: 1px solid #444;
    border-radius: 8px; box-shadow: 0 -10px 25px -5px rgba(0,0,0,0.5);
    padding: 4px 0; z-index: 20; max-height: 200px; overflow-y: auto;
    opacity: 0; transform: scaleY(0.95) translateY(4px);
    transform-origin: bottom; pointer-events: none;
    transition: opacity 0.15s ease, transform 0.15s ease;
  }
  .dropdown-panel.open {
    opacity: 1; transform: scaleY(1) translateY(0);
    pointer-events: auto;
  }
  .dropdown-opt {
    padding: 10px 12px; font-size: 13px;
    color: #ccc; cursor: pointer;
  }
  .dropdown-opt:active { background: #333; }
  .dropdown-opt.selected { color: #a78bfa; font-weight: 500; }

  /* Slider */
  .slider-group { margin-bottom: 12px; }
  .slider-header {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 8px;
  }
  .slider-label { font-size: 13px; color: #999; }
  .slider-value { font-size: 13px; font-family: monospace; color: #60a5fa; }
  input[type="range"] {
    -webkit-appearance: none; width: 100%; height: 4px;
    background: #333; border-radius: 2px; outline: none;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; width: 20px; height: 20px;
    background: #a78bfa; border-radius: 50%; cursor: pointer;
  }

  /* Action buttons */
  .actions { display: flex; gap: 8px; }
  .btn-action {
    padding: 10px 0; border-radius: 8px; border: none; font-size: 14px;
    font-weight: 600; cursor: pointer; -webkit-user-select: none;
  }
  .btn-reset {
    flex: 1; background: #252525; color: #999;
  }
  .btn-reset:active { background: #333; }
  .btn-save {
    flex: 2; background: #a78bfa; color: #fff;
  }
  .btn-save:active { background: #8b5cf6; }
  .btn-save.saving { background: #555; pointer-events: none; }
</style>
</head>
<body>

<!-- Top bar -->
<div class="top-bar">
  <button class="back-btn" id="backBtn" onclick="onBack()">
    ← 返回
  </button>
  <div class="top-bar-title">调色</div>
  <div class="top-bar-spacer"></div>
</div>

<!-- Preview area -->
<div class="preview-area">
  <img id="previewImg" style="display:none" src="" alt="Preview">
  <div class="loading-overlay" id="loadingOverlay">
    <div class="spinner"></div>
    <div class="loading-text">正在解码 RAW 图片...</div>
    <div class="loading-hint">请勿返回，否则将中断解码</div>
  </div>
  <div class="error-overlay hidden" id="errorOverlay">
    <div class="error-text" id="errorText">解码失败</div>
    <div class="error-actions">
      <button class="btn-error btn-error-back" onclick="NativeBridge.cancelPreview()">返回</button>
      <button class="btn-error btn-error-retry" onclick="retryBegin()">重试</button>
    </div>
  </div>
</div>

<!-- Controls panel -->
<div class="controls disabled" id="controls">
  <div class="controls-row">
    <!-- LUT dropdown -->
    <div class="dropdown" id="lutDropdown">
      <button class="dropdown-btn" type="button" onclick="toggleDropdown('lut')">
        <span id="lutLabel">Provia</span>
        <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
      </button>
      <div class="dropdown-panel" id="lutPanel"></div>
    </div>
    <!-- Metering dropdown -->
    <div class="dropdown" id="meteringDropdown">
      <button class="dropdown-btn" type="button" onclick="toggleDropdown('metering')">
        <span id="meteringLabel">高光保护</span>
        <svg class="chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
      </button>
      <div class="dropdown-panel" id="meteringPanel">
        <div class="dropdown-opt" data-value="highlight-safe" onclick="selectMetering(this)">高光保护</div>
        <div class="dropdown-opt" data-value="matrix" onclick="selectMetering(this)">矩阵测光</div>
        <div class="dropdown-opt" data-value="center-weighted" onclick="selectMetering(this)">中央重点测光</div>
        <div class="dropdown-opt" data-value="average" onclick="selectMetering(this)">平均测光</div>
        <div class="dropdown-opt" data-value="hybrid" onclick="selectMetering(this)">混合测光</div>
      </div>
    </div>
  </div>
  <div class="slider-group">
    <div class="slider-header">
      <span class="slider-label">曝光偏移</span>
      <span class="slider-value" id="evValue">0.0 EV</span>
    </div>
    <input type="range" id="evSlider" min="-5.0" max="5.0" step="0.1" value="0" oninput="onEvChange()">
  </div>
  <div class="actions">
    <button class="btn-action btn-reset" onclick="onReset()">重置</button>
    <button class="btn-action btn-save" id="saveBtn" onclick="onSave()">保存</button>
  </div>
</div>

<script>
  // --- State ---
  var state = 'LOADING'; // LOADING | READY | ADJUSTING | SAVING | ERROR
  var filePath = '';
  var presets = [];
  var selectedLut = 'provia';
  var selectedMetering = 'highlight-safe';
  var currentEv = 0.0;
  var applyPending = false;

  // --- Config ---
  var METERING_DEFAULTS = { 'highlight-safe': '高光保护', 'matrix': '矩阵测光', 'center-weighted': '中央重点测光', 'average': '平均测光', 'hybrid': '混合测光' };

  // --- Init ---
  function init() {
    var config = JSON.parse(NativeBridge.getConfig() || '{}');
    presets = config.presets || [];
    filePath = config.filePath || '';

    // Populate LUT panel
    var lutPanel = document.getElementById('lutPanel');
    var lutLabel = document.getElementById('lutLabel');
    if (presets.length > 0) {
      var lastUsed = config.lastUsed || {};
      var defaultLut = lastUsed.presetId || presets[0][0];
      var defaultMetering = lastUsed.meteringMode || 'highlight-safe';
      var defaultEv = lastUsed.evOffset || 0;

      selectedLut = presets.some(function(p) { return p[0] === defaultLut; }) ? defaultLut : presets[0][0];
      selectedMetering = METERING_DEFAULTS[defaultMetering] ? defaultMetering : 'highlight-safe';
      currentEv = typeof defaultEv === 'number' ? defaultEv : 0;

      var html = '';
      for (var i = 0; i < presets.length; i++) {
        var sel = presets[i][0] === selectedLut ? ' selected' : '';
        html += '<div class="dropdown-opt' + sel + '" data-value="' + presets[i][0] + '" onclick="selectLut(this)">' + presets[i][1] + '</div>';
      }
      lutPanel.innerHTML = html;
      var found = presets.find(function(p) { return p[0] === selectedLut; });
      lutLabel.textContent = found ? found[1] : presets[0][1];

      document.getElementById('meteringLabel').textContent = METERING_DEFAULTS[selectedMetering];
      // Mark selected metering
      var meteringOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
      for (var j = 0; j < meteringOpts.length; j++) {
        if (meteringOpts[j].getAttribute('data-value') === selectedMetering) {
          meteringOpts[j].classList.add('selected');
        } else {
          meteringOpts[j].classList.remove('selected');
        }
      }

      document.getElementById('evSlider').value = currentEv;
      updateEvDisplay();
    }

    // Begin preview
    NativeBridge.beginPreview(filePath);
  }

  // --- Dropdowns ---
  function toggleDropdown(name) {
    var panelId = name === 'lut' ? 'lutPanel' : 'meteringPanel';
    var panel = document.getElementById(panelId);
    var btn = panel.previousElementSibling;
    var isOpen = panel.classList.contains('open');
    closeAllDropdowns();
    if (!isOpen) {
      panel.classList.add('open');
      btn.classList.add('open');
    }
  }
  function closeAllDropdowns() {
    var panels = document.querySelectorAll('.dropdown-panel');
    var btns = document.querySelectorAll('.dropdown-btn');
    for (var i = 0; i < panels.length; i++) { panels[i].classList.remove('open'); }
    for (var j = 0; j < btns.length; j++) { btns[j].classList.remove('open'); }
  }
  document.addEventListener('click', function(e) {
    if (!e.target.closest('.dropdown')) closeAllDropdowns();
  });

  function selectLut(opt) {
    selectedLut = opt.getAttribute('data-value');
    document.getElementById('lutLabel').textContent = opt.textContent;
    var allOpts = document.getElementById('lutPanel').querySelectorAll('.dropdown-opt');
    for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
    opt.classList.add('selected');
    closeAllDropdowns();
    requestApply();
  }
  function selectMetering(opt) {
    selectedMetering = opt.getAttribute('data-value');
    document.getElementById('meteringLabel').textContent = opt.textContent;
    var allOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
    for (var i = 0; i < allOpts.length; i++) allOpts[i].classList.remove('selected');
    opt.classList.add('selected');
    closeAllDropdowns();
    requestApply();
  }

  // --- EV slider ---
  function onEvChange() {
    currentEv = parseFloat(document.getElementById('evSlider').value);
    updateEvDisplay();
    requestApply();
  }
  function updateEvDisplay() {
    var v = currentEv;
    document.getElementById('evValue').textContent = (v > 0 ? '+' : '') + v.toFixed(1) + ' EV';
  }

  // --- Apply ---
  function requestApply() {
    if (state === 'LOADING' || state === 'SAVING' || state === 'ERROR') return;
    state = 'ADJUSTING';
    if (!applyPending) {
      applyPending = true;
      NativeBridge.applyPreview(selectedLut, selectedMetering, currentEv);
    }
  }

  // --- Kotlin→JS callbacks ---
  window.onPreviewReady = function() {
    state = 'READY';
    document.getElementById('loadingOverlay').classList.add('hidden');
    document.getElementById('errorOverlay').classList.add('hidden');
    document.getElementById('controls').classList.remove('disabled');
    // Apply default preview
    NativeBridge.applyPreview(selectedLut, selectedMetering, currentEv);
  };
  window.onPreviewError = function(message) {
    state = 'ERROR';
    document.getElementById('loadingOverlay').classList.add('hidden');
    document.getElementById('errorText').textContent = message || '处理失败';
    document.getElementById('errorOverlay').classList.remove('hidden');
  };
  window.refreshPreview = function() {
    applyPending = false;
    var img = document.getElementById('previewImg');
    img.style.display = 'block';
    img.src = 'preview://latest?t=' + Date.now();
    if (state === 'ADJUSTING') state = 'READY';
    // If there was a pending request while we were applying, apply again
    // (debounce handled by Kotlin side — no need here)
  };
  window.notifyPreviewError = function(message) {
    applyPending = false;
    // Keep current preview image, just log
    console.warn('Preview apply error: ' + message);
  };

  // --- Actions ---
  function onBack() {
    if (state === 'SAVING') return;
    NativeBridge.cancelPreview();
  }
  function onReset() {
    selectedLut = 'provia';
    selectedMetering = 'highlight-safe';
    currentEv = 0;
    document.getElementById('evSlider').value = 0;
    updateEvDisplay();
    // Update LUT label
    var found = presets.find(function(p) { return p[0] === 'provia'; });
    document.getElementById('lutLabel').textContent = found ? found[1] : 'Provia';
    var lutOpts = document.getElementById('lutPanel').querySelectorAll('.dropdown-opt');
    for (var i = 0; i < lutOpts.length; i++) {
      lutOpts[i].classList.toggle('selected', lutOpts[i].getAttribute('data-value') === 'provia');
    }
    // Update metering label
    document.getElementById('meteringLabel').textContent = '高光保护';
    var meteringOpts = document.getElementById('meteringPanel').querySelectorAll('.dropdown-opt');
    for (var j = 0; j < meteringOpts.length; j++) {
      meteringOpts[j].classList.toggle('selected', meteringOpts[j].getAttribute('data-value') === 'highlight-safe');
    }
    requestApply();
  }
  function onSave() {
    if (state === 'SAVING') return;
    state = 'SAVING';
    document.getElementById('saveBtn').classList.add('saving');
    document.getElementById('controls').classList.add('disabled');
    document.getElementById('backBtn').classList.add('disabled');
    NativeBridge.save(selectedLut, selectedMetering, currentEv);
  }
  function retryBegin() {
    state = 'LOADING';
    document.getElementById('errorOverlay').classList.add('hidden');
    document.getElementById('loadingOverlay').classList.remove('hidden');
    NativeBridge.beginPreview(filePath);
  }

  // --- Auto-init ---
  window.addEventListener('load', function() { setTimeout(init, 100); });
</script>
</body>
</html>
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/assets/color_grading_preview.html
git commit -m "feat: add full-screen color grading preview HTML"
```

---

### Task 5: Create ColorGradingActivity.kt

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt`

- [ ] **Step 1: Create the Activity with WebView, NativeBridge, and shouldInterceptRequest**

Create the file with the following complete content. This Activity manages the preview lifecycle, forwards IPC to MainActivity's WebView, and streams JPEG files via `shouldInterceptRequest`:

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.util.concurrent.atomic.AtomicLong

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    private var webView: WebView? = null
    private var previewFilePath: String? = null
    private val applyRequestId = AtomicLong(0)
    private var isSessionActive = false

    // --- Lifecycle ---

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val filePath = intent.getStringExtra("filePath")
        if (filePath == null) {
            Log.e(TAG, "No filePath provided")
            finish()
            return
        }

        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            settings.allowFileAccess = false

            webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(
                    view: WebView, request: WebResourceRequest
                ): WebResourceResponse? {
                    if (request.url.scheme == "preview" && request.url.host == "latest") {
                        val path = previewFilePath
                        if (path != null) {
                            val file = File(path)
                            if (file.exists()) {
                                return WebResourceResponse(
                                    "image/jpeg", null, 200, "OK",
                                    mapOf("Content-Length" to file.length().toString()),
                                    FileInputStream(file)
                                )
                            }
                        }
                        return WebResourceResponse(
                            "image/jpeg", null, 404, "Not Found",
                            emptyMap(), null
                        )
                    }
                    return super.shouldInterceptRequest(view, request)
                }
            }

            addJavascriptInterface(
                NativeColorGradingPreviewBridge(this@ColorGradingActivity, filePath),
                "NativeBridge"
            )
            loadUrl("file:///android_asset/color_grading_preview.html")
        }

        setContentView(webView)
    }

    override fun onDestroy() {
        // Ensure cleanup if user backs out during active session
        if (isSessionActive) {
            endPreviewSession()
        }
        webView?.let {
            (it.parent as? android.view.ViewGroup)?.removeView(it)
            it.destroy()
        }
        webView = null
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        // End session and close
        if (isSessionActive) {
            endPreviewSession()
        }
        super.onBackPressed()
    }

    // --- IPC Helpers ---

    private fun callMainWebView(js: String, callback: ((String?) -> Unit)? = null) {
        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            Log.w(TAG, "MainActivity not available")
            runOnUiThread {
                Toast.makeText(this, "无法连接后端", Toast.LENGTH_SHORT).show()
                finish()
            }
            return
        }
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(js) { result ->
                callback?.invoke(result)
            }
        }
    }

    private fun parseJsString(result: String?): String? {
        if (result == null) return null
        val trimmed = result.trim()
        return if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
            try {
                JSONArray("[$trimmed]").getString(0)
            } catch (e: Exception) {
                trimmed.removeSurrounding("\"")
            }
        } else {
            trimmed
        }
    }

    private fun endPreviewSession() {
        isSessionActive = false
        previewFilePath = null
        callMainWebView(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} })();"
        )
    }

    internal fun onPreviewReady() {
        isSessionActive = true
        runOnUiThread {
            webView?.evaluateJavascript("window.onPreviewReady?.();", null)
        }
    }

    internal fun onPreviewError(message: String?) {
        isSessionActive = false
        runOnUiThread {
            webView?.evaluateJavascript(
                "window.onPreviewError?.(${JSONObject.quote(message ?: "未知错误")});", null
            )
        }
    }

    internal fun refreshWebViewPreview() {
        runOnUiThread {
            webView?.evaluateJavascript("window.refreshPreview?.();", null)
        }
    }

    internal fun notifyPreviewError(message: String?) {
        runOnUiThread {
            webView?.evaluateJavascript(
                "window.notifyPreviewError?.(${JSONObject.quote(message ?: "应用失败")});", null
            )
        }
    }
}

private class NativeColorGradingPreviewBridge(
    activity: ColorGradingActivity,
    private val filePath: String,
) {
    private val activityRef: WeakReference<ColorGradingActivity> = WeakReference(activity)
    private val applyRequestId = AtomicLong(0)

    @JavascriptInterface
    fun beginPreview(filePath: String) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "beginPreview: $filePath")
        activity.callMainWebView(
            "(async function(){ try { await window.__tauriBeginColorGradingPreview?.('${
                filePath.replace("'", "\\'")
            }'); return 'ok'; } catch(e) { return 'error:' + e.message; } })();"
        ) { result ->
            val parsed = activity.parseJsString(result)
            if (parsed?.startsWith("error:") == true) {
                val msg = parsed.substring(6)
                Log.e(TAG, "beginPreview failed: $msg")
                activity.onPreviewError(msg)
            } else {
                Log.d(TAG, "beginPreview success")
                activity.onPreviewReady()
            }
        }
    }

    @JavascriptInterface
    fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        val myId = applyRequestId.incrementAndGet()
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset id=$myId")
        activity.callMainWebView(
            "(async function(){ try { var r = await window.__tauriApplyColorGradingPreview?.('${
                lutId.replace("'", "\\'")
            }','${meteringMode.replace("'", "\\'")}',${evOffset}); return r || ''; } catch(e) { return 'error:' + e.message; } })();"
        ) { result ->
            // Discard stale responses
            if (myId != applyRequestId.get()) {
                Log.d(TAG, "Discarding stale apply result (expected $myId, current ${applyRequestId.get()})")
                return@callMainWebView
            }
            val parsed = activity.parseJsString(result)
            if (parsed?.startsWith("error:") == true) {
                val msg = parsed.substring(6)
                Log.e(TAG, "applyPreview failed: $msg")
                activity.notifyPreviewError(msg)
            } else if (parsed != null) {
                Log.d(TAG, "applyPreview success: $parsed")
                activity.previewFilePath = parsed
                activity.refreshWebViewPreview()
            }
        }
    }

    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        // Step 1: End preview session
        activity.isSessionActive = false
        activity.previewFilePath = null

        activity.callMainWebView(
            "(async function(){ try { await window.__tauriEndColorGradingPreview?.(); } catch(e) {} })();"
        ) {
            // Step 2: Trigger full export
            activity.callMainWebView(
                "(async function(){ try { await window.__tauriTriggerColorGrading?.('${
                    filePath.replace("'", "\\'")
                }','${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset},false); } catch(e) {} })();"
            ) {
                // Step 3: Save last used config
                activity.callMainWebView(
                    "window.__tauriSaveColorGradingLastUsed?.('${lutId.replace("'", "\\'")}','${meteringMode.replace("'", "\\'")}',${evOffset});"
                ) {
                    // Step 4: Close activity
                    activity.runOnUiThread { activity.finish() }
                }
            }
        }
    }

    @JavascriptInterface
    fun cancelPreview() {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "cancelPreview")
        activity.endPreviewSession()
        activity.runOnUiThread { activity.finish() }
    }

    @JavascriptInterface
    fun getConfig(): String {
        val activity = activityRef.get() ?: return "{}"

        // Synchronous: fetch presets and lastUsed from main WebView's JS globals
        // Since we can't do synchronous evaluateJavascript, we use the cached approach:
        // The HTML init() is called after load, and we pass what we know synchronously.
        // For presets/lastUsed, we need to get them from the main WebView.
        // Since @JavascriptInterface can return String, we do a blocking approach:
        // Use a Future to block on the evaluateJavascript result.

        val mainActivity = MainActivity.instance
        if (mainActivity == null) {
            return JSONObject().apply {
                put("filePath", filePath)
                put("presets", JSONArray())
            }.toString()
        }

        // Blocking call to get config from main WebView
        val resultFuture = java.util.concurrent.CompletableFuture<String>()
        mainActivity.runOnUiThread {
            mainActivity.getWebView()?.evaluateJavascript(
                "(function(){try{var l=window.__tauriGetColorGradingLastUsed?.()??'null';var p=window.__tauriGetColorGradingPresets?.()??'[]';return JSON.stringify({lastUsed:l,presets:p})}catch(e){return JSON.stringify({lastUsed:'null',presets:'[]'})}})();"
            ) { result ->
                resultFuture.complete(result ?: "{}")
            }
        }

        val raw = try {
            resultFuture.get(5, java.util.concurrent.TimeUnit.SECONDS)
        } catch (e: Exception) {
            Log.w(TAG, "getConfig timed out or failed", e)
            return JSONObject().apply {
                put("filePath", filePath)
                put("presets", JSONArray())
            }.toString()
        }

        val parsed = activity.parseJsString(raw) ?: raw
        val json = try { JSONObject(parsed) } catch (e: Exception) { JSONObject() }

        val lastUsedStr = json.optString("lastUsed", "null")
        val lastUsed = if (lastUsedStr != "null" && lastUsedStr.isNotEmpty()) {
            try { JSONObject(lastUsedStr) } catch (e: Exception) { null }
        } else null

        val presetsStr = json.optString("presets", "[]")
        val presetsArr = try { JSONArray(presetsStr) } catch (e: Exception) { JSONArray() }

        return JSONObject().apply {
            put("filePath", filePath)
            put("lastUsed", lastUsed ?: JSONObject.NULL)
            put("presets", presetsArr)
        }.toString()
    }

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt
git commit -m "feat: add ColorGradingActivity with WebView preview and IPC forwarding"
```

---

### Task 6: Modify ImageViewerActivity to launch ColorGradingActivity

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt:474-522`

- [ ] **Step 1: Replace `triggerColorGradingForCurrentImage()` method**

Replace the entire method body (lines 474-522). The new implementation simply resolves the file path and launches the new Activity:

```kotlin
    private fun triggerColorGradingForCurrentImage() {
        val uriString = uris.getOrNull(currentIndex) ?: return
        val filePath = resolveUriToFilePath(uriString)
        if (filePath == null) {
            Log.w(TAG, "Cannot resolve file path for URI: $uriString")
            return
        }
        val intent = android.content.Intent(this, ColorGradingActivity::class.java)
        intent.putExtra("filePath", filePath)
        intent.putExtra("displayName", currentDisplayName)
        startActivity(intent)
    }
```

This removes the entire config-fetching-via-evaluateJavascript flow and the overlay dialog launch. The `WebViewOverlayController.showColorGrading` and `NativeColorGradingBridge` classes are **kept** because the gallery page still uses them.

- [ ] **Step 2: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt
git commit -m "feat: ImageViewerActivity launches ColorGradingActivity for real-time preview"
```

---

### Task 7: Build and verify

**Files:** None (verification only)

- [ ] **Step 1: Build both platforms**

Run: `./build.sh windows android`
Expected: Both Windows and Android builds succeed without compilation errors.

- [ ] **Step 2: Verify no regressions**

Check that the build output does not contain errors related to:
- Missing `ColorGradingActivity` class
- Missing `color_grading_preview.html` asset
- TypeScript compilation errors in `App.tsx` or `global.ts`

- [ ] **Step 3: Commit build verification (if any fixes were needed)**

If any build errors were found and fixed, commit the fixes:

```bash
git add -A
git commit -m "fix: resolve build issues from color grading preview implementation"
```
