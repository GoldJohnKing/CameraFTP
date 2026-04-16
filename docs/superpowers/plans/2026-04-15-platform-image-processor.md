# Platform-Abstracted Image Processor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single Rust `image` crate image processor with a platform abstraction: Android uses native `ImageDecoder` + `Bitmap.compress` via JNI; Windows keeps the existing Rust implementation.

**Architecture:** Define a trait `ImagePreprocessor` with `prepare()`. On Android, `AndroidImagePreprocessor` calls a new Kotlin static method `ImageProcessorBridge.prepareForUpload()` via JNI — this uses `ImageDecoder.setTargetSize()` for **single-pass decode+downsample** (not the old `inSampleSize` + `createScaledBitmap` two-step method), then `Bitmap.compress` for JPEG encoding. On Windows, `RustImagePreprocessor` wraps the existing `image` crate code. The `AiEditService` receives the preprocessor at construction time.

**Tech Stack:** Rust (trait + cfg-gated impls), JNI (`jni` + `ndk-context`), Kotlin (`ImageDecoder` API 28+ single-pass downsample, `Bitmap.compress` JPEG encode)

---

### Task 1: Define the `ImagePreprocessor` trait and refactor `image_processor.rs` into a module

**Files:**
- Create: `src-tauri/src/ai_edit/image_processor/mod.rs`
- Create: `src-tauri/src/ai_edit/image_processor/rust_processor.rs`
- Modify: `src-tauri/src/ai_edit/mod.rs` (update module visibility)
- Modify: `src-tauri/src/ai_edit/service.rs` (accept preprocessor, remove free-function call)

- [ ] **Step 1: Create `image_processor/mod.rs` with the trait and shared types**

```rust
// src-tauri/src/ai_edit/image_processor/mod.rs
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::AppError;
use std::path::Path;

pub const MAX_LONG_SIDE: u32 = 4096;
pub const JPEG_QUALITY: u8 = 85;

#[derive(Debug)]
pub struct PreparedImage {
    pub base64_data: String,
    pub mime_type: &'static str,
}

pub trait ImagePreprocessor: Send + Sync {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError>;
}

#[cfg(not(target_os = "android"))]
mod rust_processor;

#[cfg(not(target_os = "android"))]
pub fn create_preprocessor() -> Box<dyn ImagePreprocessor> {
    Box::new(rust_processor::RustImagePreprocessor)
}

#[cfg(target_os = "android")]
mod android_processor;

#[cfg(target_os = "android")]
pub fn create_preprocessor() -> Box<dyn ImagePreprocessor> {
    Box::new(android_processor::AndroidImagePreprocessor)
}
```

- [ ] **Step 2: Move existing Rust implementation to `rust_processor.rs`**

Move the entire current `image_processor.rs` content into `rust_processor.rs`, replacing the public `prepare_for_upload` free function with a `RustImagePreprocessor` struct implementing the trait. Keep `resize_if_needed` and `encode_as_jpeg` as private helpers. Keep all tests.

```rust
// src-tauri/src/ai_edit/image_processor/rust_processor.rs
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::path::Path;

use crate::error::AppError;
use super::{ImagePreprocessor, PreparedImage, MAX_LONG_SIDE, JPEG_QUALITY};

pub struct RustImagePreprocessor;

impl ImagePreprocessor for RustImagePreprocessor {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError> {
        let ext = file_path.extension() ... // same logic as current prepare_for_upload
        // wrap in catch_unwind, etc.
    }
}

fn prepare_heic(...) { ... }
fn encode_as_jpeg(...) { ... }
fn resize_if_needed(...) { ... }

#[cfg(test)]
mod tests { ... } // move all tests here, update to use RustImagePreprocessor.prepare()
```

- [ ] **Step 3: Update `mod.rs` visibility**

In `src-tauri/src/ai_edit/mod.rs`, change `pub(crate) mod image_processor` to `pub(crate) mod image_processor` (keep same). No change needed if already correct.

- [ ] **Step 4: Update `service.rs` to use the trait**

Add `preprocessor: Box<dyn ImagePreprocessor>` field to `AiEditService`. Construct via `image_processor::create_preprocessor()` in `AiEditService::new()`. Replace the `image_processor::prepare_for_upload()` call in `process_task()` with `self.preprocessor.prepare()`.

- [ ] **Step 5: Build and test**

Run: `./build.sh windows android`
Expected: Both platforms compile. Windows uses `RustImagePreprocessor`, Android gets a placeholder `AndroidImagePreprocessor`.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "refactor(ai-edit): abstract ImagePreprocessor trait with platform dispatch"
```

---

### Task 2: Create Android `ImageProcessorBridge` in Kotlin

**Files:**
- Create: `src-tauri/gen/android/.../bridges/ImageProcessorBridge.kt`

- [ ] **Step 1: Write the Kotlin bridge class**

```kotlin
// bridges/ImageProcessorBridge.kt
// SPDX-License-Identifier: AGPL-3.0-or-later

package com.gjk.cameraftpcompanion.bridges

import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.util.Base64
import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.File
import kotlin.math.roundToInt
import kotlin.math.max

class ImageProcessorBridge {
    companion object {
        private const val TAG = "ImageProcessorBridge"

        @JvmStatic
        fun prepareForUpload(filePath: String, maxLongSide: Int, jpegQuality: Int): String? {
            return try {
                val file = File(filePath)
                val source = ImageDecoder.createSource(file)
                val bitmap = ImageDecoder.decodeBitmap(source) { info, _ ->
                    val w = info.size.width
                    val h = info.size.height
                    val longSide = maxOf(w, h)
                    if (longSide > maxLongSide) {
                        val scale = maxLongSide.toFloat() / longSide.toFloat()
                        setTargetSize((w * scale).roundToInt(), (h * scale).roundToInt())
                    }
                }

                val stream = ByteArrayOutputStream()
                bitmap.compress(Bitmap.CompressFormat.JPEG, jpegQuality, stream)
                val bytes = stream.toByteArray()

                Base64.encodeToString(bytes, Base64.NO_WRAP)
            } catch (e: OutOfMemoryError) {
                Log.e(TAG, "OOM preparing image: $filePath", e)
                null
            } catch (e: Exception) {
                Log.e(TAG, "Failed to prepare image: $filePath", e)
                null
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "feat(android): add ImageProcessorBridge with native ImageDecoder"
```

---

### Task 3: Implement `AndroidImagePreprocessor` in Rust (JNI caller)

**Files:**
- Create: `src-tauri/src/ai_edit/image_processor/android_processor.rs`

- [ ] **Step 1: Write the JNI-calling implementation**

Follow the exact same JNI pattern as `platform/android.rs` (`get_java_vm` → `attach_current_thread` → `get_android_context` → `getClassLoader` → `loadClass` → `call_static_method`).

```rust
// src-tauri/src/ai_edit/image_processor/android_processor.rs
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;
use jni::objects::{JClass, JObject, JValue};
use jni::JavaVM;

use crate::error::AppError;
use super::{ImagePreprocessor, PreparedImage, MAX_LONG_SIDE, JPEG_QUALITY};

const BRIDGE_CLASS: &str = "com.gjk.cameraftpcompanion.bridges.ImageProcessorBridge";
const METHOD_NAME: &str = "prepareForUpload";
const METHOD_SIG: &str = "(Ljava/lang/String;II)Ljava/lang/String;";

pub struct AndroidImagePreprocessor;

impl ImagePreprocessor for AndroidImagePreprocessor {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError> {
        let path_str = file_path.to_string_lossy().to_string();

        let jvm = get_java_vm()?;
        let mut env = jvm.attach_current_thread()
            .map_err(|e| AppError::AiEditError(format!("JNI attach failed: {e}")))?;
        let context = get_android_context(&mut env)?;
        let bridge_class = load_class(&mut env, &context)?;

        let j_path = env.new_string(&path_str)
            .map_err(|e| AppError::AiEditError(format!("JNI new_string failed: {e}")))?;

        let result = env.call_static_method(
            bridge_class,
            METHOD_NAME,
            METHOD_SIG,
            &[
                JValue::Object(&JObject::from(j_path)),
                JValue::Int(MAX_LONG_SIDE as i32),
                JValue::Int(JPEG_QUALITY as i32),
            ],
        ).map_err(|e| AppError::AiEditError(format!("JNI call failed: {e}")))?;

        let j_result = result.l()
            .map_err(|e| AppError::AiEditError(format!("JNI result extraction failed: {e}")))?;

        if j_result.is_null() {
            return Err(AppError::AiEditError(
                "Android native image processing failed — likely OOM or unsupported format".to_string(),
            ));
        }

        let base64: String = env.get_string(&j_result.into())
            .map_err(|e| AppError::AiEditError(format!("JNI get_string failed: {e}")))?
            .into();

        Ok(PreparedImage {
            base64_data: base64,
            mime_type: "image/jpeg",
        })
    }
}

fn get_java_vm() -> Result<JavaVM, AppError> {
    let context = ndk_context::android_context();
    unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|e| AppError::AiEditError(format!("Failed to get JavaVM: {e}")))
}

fn get_android_context<'a>(env: &mut jni::JNIEnv<'a>) -> Result<JObject<'a>, AppError> {
    let context = ndk_context::android_context();
    let raw = unsafe { JObject::from_raw(context.context().cast()) };
    let local = env.new_local_ref(&raw)
        .map_err(|e| AppError::AiEditError(format!("Failed to get Android context: {e}")))?;
    let _ = raw.into_raw();
    Ok(local)
}

fn load_class<'a>(env: &mut jni::JNIEnv<'a>, context: &JObject<'a>) -> Result<JClass<'a>, AppError> {
    let loader = env.call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
        .map_err(|e| AppError::AiEditError(format!("getClassLoader failed: {e}")))?;
    let name = env.new_string(BRIDGE_CLASS)
        .map_err(|e| AppError::AiEditError(format!("new_string failed: {e}")))?;
    let class_obj = env.call_method(
        loader,
        "loadClass",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        &[JValue::Object(&JObject::from(name))],
    ).and_then(|v| v.l())
        .map_err(|e| AppError::AiEditError(format!("loadClass failed: {e}")))?;
    Ok(JClass::from(class_obj))
}
```

- [ ] **Step 2: Build Android**

Run: `./build.sh android`
Expected: Compiles successfully with the new JNI bridge.

- [ ] **Step 3: Build Windows**

Run: `./build.sh windows`
Expected: Compiles with `RustImagePreprocessor` (no JNI code compiled on Windows).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(android): implement AndroidImagePreprocessor via JNI"
```

---

### Task 4: Clean up and verify

**Files:**
- Delete: nothing (old `image_processor.rs` was already renamed)
- Verify: all tests pass on both platforms

- [ ] **Step 1: Run Rust tests**

Run: `cargo.exe test --package cameraftp --lib ai_edit::image_processor`
Expected: All tests pass (Windows `RustImagePreprocessor` tests).

- [ ] **Step 2: Run full build**

Run: `./build.sh windows android`
Expected: Both platforms build successfully.

- [ ] **Step 3: Commit any final cleanup**

```bash
git add -A && git commit -m "chore: cleanup after platform image processor refactor"
```
