// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JNI bridge implementation for Android MediaStore operations.

use super::types::{
    FileDescriptorInfo, MediaStoreBridgeClient, MediaStoreCollection, MediaStoreError, QueryResult,
};
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(target_os = "android")]
use super::types::{display_name_from_path, mime_type_from_filename, relative_path_from_full_path};
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JString, JValue};
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use serde::Deserialize;
#[cfg(target_os = "android")]
use tracing::debug;

// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
use super::types::{display_name_from_path, mime_type_from_filename, relative_path_from_full_path};

#[cfg(target_os = "android")]
const MEDIASTORE_BRIDGE_CLASS: &str = "com.gjk.cameraftpcompanion.bridges.MediaStoreBridge";

#[cfg(target_os = "android")]
const FINALIZE_ENTRY_METHOD_NAME: &str = "finalizeEntryAndEmitGalleryItemsAddedNative";

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn normalize_relative_path_for_match(relative_path: &str) -> String {
    let trimmed = relative_path.trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}/")
    }
}

#[cfg(target_os = "android")]
const FINALIZE_ENTRY_METHOD_SIGNATURE: &str =
    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/Long;)Z";

/// JNI-based MediaStore bridge for Android.
#[cfg(target_os = "android")]
#[derive(Debug, Default)]
pub struct JniMediaStoreBridge;

#[cfg(target_os = "android")]
impl JniMediaStoreBridge {
    pub fn new() -> Self {
        Self
    }

    /// 把 JNI 引导阶段（JavaVM / attach / Context / loadClass）的 AppError
    /// 映射为 MediaStoreError（文案语义不变）。
    fn bridge_err(e: crate::error::AppError) -> MediaStoreError {
        MediaStoreError::BridgeError(e.user_message())
    }

    fn with_env<T>(f: impl FnOnce(&mut JNIEnv<'_>) -> Result<T, MediaStoreError>) -> Result<T, MediaStoreError> {
        // JVM attach 走共享助手（crate::utils::jni，含统一异常清理）；
        // f 的 MediaStoreError 变体（如 NotFound）经 Ok 通道透传，
        // 不经过 AppError 往返以避免丢失变体。
        crate::utils::jni::with_env(move |env| Ok(f(env)))
            .map_err(Self::bridge_err)?
    }

    /// 在 `spawn_blocking` 线程中执行同步 JNI 调用，避免阻塞 tokio 异步运行时线程
    /// （模式同 crypto::verify_password 在 CustomAuthenticator 中的处理）。
    async fn with_env_blocking<T, F>(f: F) -> Result<T, MediaStoreError>
    where
        T: Send + 'static,
        F: FnOnce(&mut JNIEnv<'_>) -> Result<T, MediaStoreError> + Send + 'static,
    {
        tokio::task::spawn_blocking(move || Self::with_env(f))
            .await
            .map_err(|e| {
                MediaStoreError::BridgeError(format!("JNI blocking task join failed: {e}"))
            })?
    }

    fn get_bridge_class<'a>(env: &mut JNIEnv<'a>) -> Result<JClass<'a>, MediaStoreError> {
        let context = Self::get_context(env)?;
        crate::utils::jni::load_app_class(env, &context, MEDIASTORE_BRIDGE_CLASS)
            .map_err(Self::bridge_err)
    }

    fn get_context<'a>(env: &mut JNIEnv<'a>) -> Result<JObject<'a>, MediaStoreError> {
        crate::utils::jni::android_context(env).map_err(Self::bridge_err)
    }

    fn new_jstring<'a>(env: &mut JNIEnv<'a>, value: &str) -> Result<JObject<'a>, MediaStoreError> {
        let s = crate::utils::jni::jni_ok(env, "Failed to create Java string", |env| {
            env.new_string(value)
        })
        .map_err(Self::bridge_err)?;
        Ok(JObject::from(s))
    }

    fn optional_long<'a>(
        env: &mut JNIEnv<'a>,
        value: Option<u64>,
    ) -> Result<JObject<'a>, MediaStoreError> {
        match value {
            None => Ok(JObject::null()),
            Some(v) => {
                let boxed = crate::utils::jni::jni_ok(env, "Failed to box Long value", |env| {
                    env.call_static_method(
                        "java/lang/Long",
                        "valueOf",
                        "(J)Ljava/lang/Long;",
                        &[JValue::Long(v as i64)],
                    )
                    .and_then(|o| o.l())
                })
                .map_err(Self::bridge_err)?;
                Ok(boxed)
            }
        }
    }

    fn parse_list_entries(json: &str) -> Result<Vec<QueryResult>, MediaStoreError> {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ListEntry {
            uri: String,
            display_name: String,
            size: u64,
            date_modified: u64,
            mime_type: Option<String>,
            relative_path: Option<String>,
        }

        #[derive(Debug, Deserialize)]
        struct ListResponse {
            entries: Vec<ListEntry>,
        }

        let response: ListResponse = serde_json::from_str(json).map_err(|e| {
            MediaStoreError::QueryFailed(format!("Failed to parse listEntriesNative response: {e}"))
        })?;

        let entries = response
            .entries
            .into_iter()
            .map(|e| QueryResult {
                content_uri: e.uri,
                display_name: e.display_name.clone(),
                size: e.size,
                date_modified: e.date_modified,
                mime_type: e
                    .mime_type
                    .unwrap_or_else(|| mime_type_from_filename(&e.display_name).to_string()),
                relative_path: e.relative_path.unwrap_or_default(),
            })
            .collect();

        Ok(entries)
    }

    fn parse_create_result(json: &str, display_name: &str, relative_path: &str) -> Result<FileDescriptorInfo, MediaStoreError> {
        #[derive(Debug, Deserialize)]
        struct CreateResult {
            fd: i32,
            uri: Option<String>,
            error: Option<String>,
        }

        let response: CreateResult = serde_json::from_str(json).map_err(|e| {
            MediaStoreError::InsertFailed(format!("Failed to parse createEntryNative response: {e}"))
        })?;

        if response.fd < 0 {
            let message = response
                .error
                .unwrap_or_else(|| "createEntryNative returned invalid fd".to_string());
            return Err(MediaStoreError::OpenFdFailed(message));
        }

        let content_uri = response
            .uri
            .ok_or_else(|| MediaStoreError::InsertFailed("Missing URI in createEntryNative response".to_string()))?;

        Ok(FileDescriptorInfo {
            #[cfg(unix)]
            fd: response.fd,
            content_uri,
            path: PathBuf::from(format!("{relative_path}{display_name}")),
        })
    }

    fn call_static_string(
        env: &mut JNIEnv<'_>,
        method: &str,
        signature: &str,
        args: &[JValue<'_, '_>],
    ) -> Result<String, MediaStoreError> {
        let class = Self::get_bridge_class(env)?;

        let obj = env
            .call_static_method(&class, method, signature, args)
            .and_then(|v| v.l())
            .map_err(|e| {
                crate::utils::jni::clear_pending_exception(env);
                MediaStoreError::BridgeError(format!("{method} call failed: {e}"))
            })?;

        if obj.is_null() {
            return Ok(String::new());
        }

        let s = JString::from(obj);
        match env.get_string(&s).map(|v| v.into()) {
            Ok(value) => Ok(value),
            Err(e) => {
                // 解码结果字符串可能残留 pending Java 异常（如 OOM），
                // 必须清除，否则当前线程后续 JNI 调用都会失败
                crate::utils::jni::clear_pending_exception(env);
                Err(MediaStoreError::BridgeError(format!(
                    "{method} result decode failed: {e}"
                )))
            }
        }
    }

    fn call_static_bool(
        env: &mut JNIEnv<'_>,
        method: &str,
        signature: &str,
        args: &[JValue<'_, '_>],
    ) -> Result<bool, MediaStoreError> {
        let class = Self::get_bridge_class(env)?;
        env.call_static_method(&class, method, signature, args)
            .and_then(|v| v.z())
            .map_err(|e| {
                crate::utils::jni::clear_pending_exception(env);
                MediaStoreError::BridgeError(format!("{method} call failed: {e}"))
            })
    }

    fn call_static_i32(
        env: &mut JNIEnv<'_>,
        method: &str,
        signature: &str,
        args: &[JValue<'_, '_>],
    ) -> Result<i32, MediaStoreError> {
        let class = Self::get_bridge_class(env)?;
        env.call_static_method(&class, method, signature, args)
            .and_then(|v| v.i())
            .map_err(|e| {
                crate::utils::jni::clear_pending_exception(env);
                MediaStoreError::BridgeError(format!("{method} call failed: {e}"))
            })
    }
}

#[cfg(target_os = "android")]
#[async_trait::async_trait]
impl MediaStoreBridgeClient for JniMediaStoreBridge {
    async fn open_fd_for_read(&self, path: &str) -> Result<FileDescriptorInfo, MediaStoreError> {
        debug!(path, "Opening file descriptor for read");

        let display_name = display_name_from_path(path);
        let relative_path = relative_path_from_full_path(path);
        let not_found_path = path.to_string();

        let content_uri = Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_relative_path = Self::new_jstring(env, &relative_path)?;
            let j_display_name = Self::new_jstring(env, &display_name)?;

            let uri = Self::call_static_string(
                env,
                "findEntryUriNative",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[
                    JValue::Object(&context),
                    JValue::Object(&j_relative_path),
                    JValue::Object(&j_display_name),
                ],
            )?;

            if uri.is_empty() {
                return Err(MediaStoreError::NotFound(not_found_path));
            }

            Ok(uri)
        })
        .await?;

        let fd = {
            let content_uri = content_uri.clone();
            Self::with_env_blocking(move |env| {
                let context = Self::get_context(env)?;
                let j_uri = Self::new_jstring(env, &content_uri)?;
                Self::call_static_i32(
                    env,
                    "openEntryForReadNative",
                    "(Landroid/content/Context;Ljava/lang/String;)I",
                    &[JValue::Object(&context), JValue::Object(&j_uri)],
                )
            })
            .await?
        };

        Ok(FileDescriptorInfo {
            #[cfg(unix)]
            fd,
            content_uri,
            path: PathBuf::from(path.trim_start_matches('/')),
        })
    }

    async fn open_fd_for_write(
        &self,
        display_name: &str,
        mime_type: &str,
        relative_path: &str,
        collection: MediaStoreCollection,
    ) -> Result<FileDescriptorInfo, MediaStoreError> {
        debug!(display_name, mime_type, relative_path, collection = %collection.as_str(), "Opening file descriptor for write");

        let display_name = display_name.to_string();
        let mime_type = mime_type.to_string();
        let relative_path = relative_path.to_string();
        Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_display_name = Self::new_jstring(env, &display_name)?;
            let j_mime = Self::new_jstring(env, &mime_type)?;
            let j_relative_path = Self::new_jstring(env, &relative_path)?;
            let j_collection = Self::new_jstring(env, collection.as_str())?;
            let j_size = JObject::null();

            let json = Self::call_static_string(
                env,
                "createEntryNative",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/Long;)Ljava/lang/String;",
                &[
                    JValue::Object(&context),
                    JValue::Object(&j_display_name),
                    JValue::Object(&j_mime),
                    JValue::Object(&j_relative_path),
                    JValue::Object(&j_collection),
                    JValue::Object(&j_size),
                ],
            )?;

            Self::parse_create_result(&json, &display_name, &relative_path)
        })
        .await
    }

    async fn finalize_entry(
        &self,
        content_uri: &str,
        expected_size: Option<u64>,
    ) -> Result<(), MediaStoreError> {
        debug!(content_uri, expected_size, "Finalizing MediaStore entry");

        let content_uri = content_uri.to_string();
        Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_uri = Self::new_jstring(env, &content_uri)?;
            let j_size = Self::optional_long(env, expected_size)?;

            let ok = Self::call_static_bool(
                env,
                FINALIZE_ENTRY_METHOD_NAME,
                FINALIZE_ENTRY_METHOD_SIGNATURE,
                &[
                    JValue::Object(&context),
                    JValue::Object(&j_uri),
                    JValue::Object(&j_size),
                ],
            )?;

            if ok {
                Ok(())
            } else {
                Err(MediaStoreError::InsertFailed(format!(
                    "finalizeEntryNative returned false for {content_uri}"
                )))
            }
        })
        .await
    }

    async fn abort_entry(&self, content_uri: &str) -> Result<(), MediaStoreError> {
        debug!(content_uri, "Aborting MediaStore entry");

        let content_uri = content_uri.to_string();
        Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_uri = Self::new_jstring(env, &content_uri)?;

            let ok = Self::call_static_bool(
                env,
                "abortEntryNative",
                "(Landroid/content/Context;Ljava/lang/String;)Z",
                &[JValue::Object(&context), JValue::Object(&j_uri)],
            )?;

            if ok {
                Ok(())
            } else {
                Err(MediaStoreError::DeleteFailed(format!(
                    "abortEntryNative returned false for {content_uri}"
                )))
            }
        })
        .await
    }

    async fn query_files(&self, path: &str) -> Result<Vec<QueryResult>, MediaStoreError> {
        debug!(path, "Querying files");

        let trimmed = path.trim_start_matches('/');
        let relative_path = if trimmed.is_empty() {
            String::new()
        } else {
            format!("{}/", trimmed.trim_end_matches('/'))
        };

        let json = Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_relative_path = Self::new_jstring(env, &relative_path)?;

            Self::call_static_string(
                env,
                "listEntriesNative",
                "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&context), JValue::Object(&j_relative_path)],
            )
        })
        .await?;

        Self::parse_list_entries(&json)
    }

    async fn query_file(&self, path: &str) -> Result<QueryResult, MediaStoreError> {
        debug!(path, "Querying single file");

        let display_name = display_name_from_path(path);
        let relative_path = normalize_relative_path_for_match(&relative_path_from_full_path(path));

        let files = self.query_files(&relative_path).await?;
        files
            .into_iter()
            .find(|f| {
                f.display_name == display_name
                    && normalize_relative_path_for_match(&f.relative_path) == relative_path
            })
            .ok_or_else(|| MediaStoreError::NotFound(path.to_string()))
    }

    async fn delete_file(&self, path: &str) -> Result<(), MediaStoreError> {
        debug!(path, "Deleting file");

        let display_name = display_name_from_path(path);
        let relative_path = relative_path_from_full_path(path);

        let content_uri = {
            let not_found_path = path.to_string();
            Self::with_env_blocking(move |env| {
                let context = Self::get_context(env)?;
                let j_relative_path = Self::new_jstring(env, &relative_path)?;
                let j_display_name = Self::new_jstring(env, &display_name)?;

                let uri = Self::call_static_string(
                    env,
                    "findEntryUriNative",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(&context),
                        JValue::Object(&j_relative_path),
                        JValue::Object(&j_display_name),
                    ],
                )?;

                if uri.is_empty() {
                    Err(MediaStoreError::NotFound(not_found_path))
                } else {
                    Ok(uri)
                }
            })
            .await?
        };

        let deleted = Self::with_env_blocking(move |env| {
            let context = Self::get_context(env)?;
            let j_uri = Self::new_jstring(env, &content_uri)?;
            Self::call_static_bool(
                env,
                "deleteEntryNative",
                "(Landroid/content/Context;Ljava/lang/String;)Z",
                &[JValue::Object(&context), JValue::Object(&j_uri)],
            )
        })
        .await?;

        if deleted {
            Ok(())
        } else {
            Err(MediaStoreError::DeleteFailed(format!(
                "deleteEntryNative returned false for {path}"
            )))
        }
    }
}

/// Mock MediaStore bridge for testing and non-Android platforms.
// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
#[derive(Debug)]
pub struct MockMediaStoreBridge {
    base_path: PathBuf,
}

// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
impl MockMediaStoreBridge {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn temp() -> Self {
        Self {
            base_path: std::env::temp_dir().join("cameraftp_mock_mediastore"),
        }
    }
}

// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl MediaStoreBridgeClient for MockMediaStoreBridge {
    async fn open_fd_for_read(&self, path: &str) -> Result<FileDescriptorInfo, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        if !full_path.exists() {
            return Err(MediaStoreError::NotFound(path.to_string()));
        }

        #[cfg(unix)]
        {
            use std::os::fd::IntoRawFd;
            let file = std::fs::File::open(&full_path)?;
            Ok(FileDescriptorInfo {
                fd: file.into_raw_fd(),
                content_uri: format!("content://mock/media/{}", path.trim_start_matches('/')),
                path: full_path,
            })
        }

        #[cfg(not(unix))]
        {
            Err(MediaStoreError::BridgeError(
                "File descriptors not supported on this platform".to_string(),
            ))
        }
    }

    async fn open_fd_for_write(
        &self,
        display_name: &str,
        _mime_type: &str,
        relative_path: &str,
        _collection: MediaStoreCollection,
    ) -> Result<FileDescriptorInfo, MediaStoreError> {
        let dir_path = self.base_path.join(relative_path.trim_start_matches('/'));
        let full_path = dir_path.join(display_name);

        tokio::fs::create_dir_all(&dir_path)
            .await
            .map_err(MediaStoreError::IoError)?;

        let file = std::fs::File::create(&full_path)?;

        #[cfg(unix)]
        {
            use std::os::fd::IntoRawFd;
            Ok(FileDescriptorInfo {
                fd: file.into_raw_fd(),
                content_uri: format!(
                    "content://mock/media/{}{}",
                    relative_path.trim_start_matches('/'),
                    display_name
                ),
                path: full_path,
            })
        }

        #[cfg(not(unix))]
        {
            drop(file);
            Err(MediaStoreError::BridgeError(
                "File descriptors not supported on this platform".to_string(),
            ))
        }
    }

    async fn finalize_entry(
        &self,
        _content_uri: &str,
        _expected_size: Option<u64>,
    ) -> Result<(), MediaStoreError> {
        Ok(())
    }

    async fn abort_entry(&self, content_uri: &str) -> Result<(), MediaStoreError> {
        let path = content_uri
            .trim_start_matches("content://mock/media/")
            .trim_start_matches('/');
        let full_path = self.base_path.join(path);
        if full_path.exists() {
            tokio::fs::remove_file(&full_path)
                .await
                .map_err(MediaStoreError::IoError)?;
        }
        Ok(())
    }

    async fn query_files(&self, path: &str) -> Result<Vec<QueryResult>, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        if !full_path.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut stack = vec![full_path.clone()];

        while let Some(current_dir) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&current_dir)
                .await
                .map_err(MediaStoreError::IoError)?;

            while let Some(entry) = entries.next_entry().await.map_err(MediaStoreError::IoError)? {
                let metadata = entry.metadata().await.map_err(MediaStoreError::IoError)?;

                if metadata.is_dir() {
                    stack.push(entry.path());
                    continue;
                }

                let name = entry.file_name().to_string_lossy().to_string();
                let relative_dir = entry
                    .path()
                    .parent()
                    .and_then(|parent| parent.strip_prefix(&self.base_path).ok())
                    .map(|parent| {
                        let value = parent.to_string_lossy().replace('\\', "/");
                        if value.is_empty() {
                            String::new()
                        } else {
                            format!("{}/", value.trim_end_matches('/'))
                        }
                    })
                    .unwrap_or_default();

                results.push(QueryResult {
                    content_uri: format!("content://mock/media/{}{}", relative_dir, name),
                    display_name: name.clone(),
                    size: metadata.len(),
                    date_modified: metadata
                        .modified()
                        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64)
                        .unwrap_or(0),
                    mime_type: mime_type_from_filename(&name).to_string(),
                    relative_path: relative_dir,
                });
            }
        }

        Ok(results)
    }

    async fn query_file(&self, path: &str) -> Result<QueryResult, MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        if !full_path.exists() {
            return Err(MediaStoreError::NotFound(path.to_string()));
        }

        let metadata = tokio::fs::metadata(&full_path)
            .await
            .map_err(MediaStoreError::IoError)?;
        let name = full_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let expected_display_name = display_name_from_path(path);

        if name != expected_display_name {
            return Err(MediaStoreError::NotFound(path.to_string()));
        }

        Ok(QueryResult {
            content_uri: format!("content://mock/media/{}", path.trim_start_matches('/')),
            display_name: name.clone(),
            size: metadata.len(),
            date_modified: metadata
                .modified()
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64)
                .unwrap_or(0),
            mime_type: if metadata.is_dir() {
                "inode/directory".to_string()
            } else {
                mime_type_from_filename(&name).to_string()
            },
            relative_path: relative_path_from_full_path(path),
        })
    }

    async fn delete_file(&self, path: &str) -> Result<(), MediaStoreError> {
        let full_path = self.base_path.join(path.trim_start_matches('/'));
        if full_path.is_dir() {
            tokio::fs::remove_dir(&full_path)
                .await
                .map_err(MediaStoreError::IoError)?;
        } else {
            tokio::fs::remove_file(&full_path)
                .await
                .map_err(MediaStoreError::IoError)?;
        }
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub fn create_bridge() -> Arc<dyn MediaStoreBridgeClient> {
    Arc::new(JniMediaStoreBridge::new())
}

// desktop-generic (not android-specific)
#[cfg(not(target_os = "android"))]
pub fn create_bridge() -> Arc<dyn MediaStoreBridgeClient> {
    Arc::new(MockMediaStoreBridge::temp())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(not(target_os = "android"), unix))]
    use tempfile::TempDir;

    #[test]
    fn test_normalize_relative_path_for_match() {
        assert_eq!(normalize_relative_path_for_match(""), "");
        assert_eq!(normalize_relative_path_for_match("DCIM/CameraFTP"), "DCIM/CameraFTP/");
        assert_eq!(normalize_relative_path_for_match("DCIM/CameraFTP/"), "DCIM/CameraFTP/");
        assert_eq!(normalize_relative_path_for_match("/DCIM/CameraFTP//"), "DCIM/CameraFTP/");
    }

    #[cfg(all(not(target_os = "android"), unix))]
    #[tokio::test]
    async fn test_mock_bridge_create_and_query_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());

        let fd_info = bridge
            .open_fd_for_write("test.jpg", "image/jpeg", "DCIM/", MediaStoreCollection::Images)
            .await
            .expect("open fd for write");
        assert!(fd_info.path.exists());

        let result = bridge.query_file("DCIM/test.jpg").await.expect("query file");
        assert_eq!(result.display_name, "test.jpg");
        assert_eq!(result.mime_type, "image/jpeg");
    }
}
