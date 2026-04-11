// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Unit tests for the Android MediaStore storage backend.
//!
//! These tests verify the core functionality of the backend components
//! without requiring an actual Android device.

use super::backend::{AndroidMediaStoreBackend, MediaStoreMetadata};
use super::limiter::UploadLimiter;
use super::retry::{retry_with_backoff, RetryConfig};
use super::types::{
    default_relative_path, display_name_from_path, mime_type_from_filename,
    relative_path_from_full_path, MediaStoreCollection, MediaStoreError, QueryResult,
    MIME_TYPE_DEFAULT, MIME_TYPE_HEIF, MIME_TYPE_JPEG, MIME_TYPE_MP4,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use unftp_core::auth::DefaultUser;
// Import the traits to call trait methods
use unftp_core::storage::{ErrorKind, Metadata, StorageBackend};

#[cfg(not(target_os = "android"))]
use super::bridge::MockMediaStoreBridge;
#[cfg(not(target_os = "android"))]
use super::types::MediaStoreBridgeClient;
#[cfg(not(target_os = "android"))]
use tempfile::TempDir;

#[test]
fn android_mediastore_mod_source_does_not_reexport_create_bridge() {
    let source = include_str!("mod.rs");

    assert!(!source.contains("pub use bridge::create_bridge;"));
}

// ============================================================================
// Tests for display_name_from_path
// ============================================================================

#[test]
fn test_display_name_from_path_simple() {
    assert_eq!(display_name_from_path("photo.jpg"), "photo.jpg");
}

#[test]
fn test_display_name_from_path_with_directory() {
    assert_eq!(display_name_from_path("/DCIM/Camera/photo.jpg"), "photo.jpg");
}

#[test]
fn test_display_name_from_path_multiple_slashes() {
    assert_eq!(display_name_from_path("/a/b/c/d/photo.jpg"), "photo.jpg");
}

#[test]
fn test_display_name_from_path_trailing_slash() {
    assert_eq!(display_name_from_path("/DCIM/Camera/"), "");
}

#[test]
fn test_display_name_from_path_empty() {
    assert_eq!(display_name_from_path(""), "");
}

// ============================================================================
// Tests for default_relative_path
// ============================================================================

#[test]
fn test_default_relative_path() {
    let path = default_relative_path();
    assert!(path.starts_with("DCIM/"));
    assert!(path.ends_with('/'));
}

// ============================================================================
// Tests for relative_path_from_full_path
// ============================================================================

#[test]
fn test_relative_path_from_full_path_nested() {
    assert_eq!(relative_path_from_full_path("/DCIM/Camera/photo.jpg"), "DCIM/Camera/");
}

#[test]
fn test_relative_path_from_full_path_single_level() {
    assert_eq!(relative_path_from_full_path("/DCIM/photo.jpg"), "DCIM/");
}

#[test]
fn test_relative_path_from_full_path_root() {
    assert_eq!(relative_path_from_full_path("photo.jpg"), "");
}

#[test]
fn test_relative_path_from_full_path_no_leading_slash() {
    assert_eq!(relative_path_from_full_path("DCIM/photo.jpg"), "DCIM/");
}

// ============================================================================
// Tests for mime_type_from_filename
// ============================================================================

#[test]
fn test_mime_type_jpeg() {
    assert_eq!(mime_type_from_filename("photo.jpg"), MIME_TYPE_JPEG);
    assert_eq!(mime_type_from_filename("photo.jpeg"), MIME_TYPE_JPEG);
    assert_eq!(mime_type_from_filename("PHOTO.JPG"), MIME_TYPE_JPEG);
    assert_eq!(mime_type_from_filename("Photo.JPEG"), MIME_TYPE_JPEG);
}

#[test]
fn test_mime_type_video_and_heif() {
    assert_eq!(mime_type_from_filename("video.mp4"), MIME_TYPE_MP4);
    assert_eq!(mime_type_from_filename("video.MP4"), MIME_TYPE_MP4);
    assert_eq!(mime_type_from_filename("image.heif"), MIME_TYPE_HEIF);
}

#[test]
fn test_mime_type_unknown() {
    assert_eq!(mime_type_from_filename("file.txt"), MIME_TYPE_DEFAULT);
    assert_eq!(mime_type_from_filename("file.bin"), MIME_TYPE_DEFAULT);
    assert_eq!(mime_type_from_filename("noextension"), MIME_TYPE_DEFAULT);
}

// ============================================================================
// Tests for retry_with_backoff
// ============================================================================

#[tokio::test]
async fn test_retry_succeeds_immediately() {
    use std::sync::atomic::{AtomicU32, Ordering};
    
    let config = RetryConfig::fast();
    let count = Arc::new(AtomicU32::new(0));
    let count_clone = count.clone();
    
    let result: Result<i32, &str> = retry_with_backoff(&config, "test", || {
        let c = count_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(42)
        }
    }).await;
    
    assert_eq!(result.unwrap(), 42);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_retry_succeeds_after_one_failure() {
    use std::sync::atomic::{AtomicU32, Ordering};
    
    let config = RetryConfig::fast();
    let count = Arc::new(AtomicU32::new(0));
    let count_clone = count.clone();
    
    let result: Result<i32, &str> = retry_with_backoff(&config, "test", || {
        let c = count_clone.clone();
        async move {
            let n = c.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Err("temporary failure")
            } else {
                Ok(42)
            }
        }
    }).await;
    
    assert_eq!(result.unwrap(), 42);
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_retry_exhausted_all_attempts() {
    use std::sync::atomic::{AtomicU32, Ordering};
    
    let config = RetryConfig {
        max_retries: 2,
        initial_delay: std::time::Duration::from_millis(10),
        max_delay: std::time::Duration::from_millis(100),
        backoff_multiplier: 2.0,
    };
    let count = Arc::new(AtomicU32::new(0));
    let count_clone = count.clone();
    
    let result: Result<i32, &str> = retry_with_backoff(&config, "test", || {
        let c = count_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err("permanent failure")
        }
    }).await;
    
    assert!(result.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
}

// ============================================================================
// Tests for UploadLimiter
// ============================================================================

#[test]
fn test_upload_limiter_creation() {
    let limiter = UploadLimiter::new(4);
    assert_eq!(limiter.max_concurrent(), 4);
    assert_eq!(limiter.available_permits(), 4);
}

#[test]
fn test_upload_limiter_default() {
    let limiter = UploadLimiter::default_limiter();
    assert_eq!(limiter.max_concurrent(), 4);
}

#[tokio::test]
async fn test_upload_limiter_acquire_release() {
    let limiter = UploadLimiter::new(2);
    
    assert_eq!(limiter.available_permits(), 2);
    
    let permit1 = limiter.acquire().await;
    assert_eq!(limiter.available_permits(), 1);
    
    let permit2 = limiter.acquire().await;
    assert_eq!(limiter.available_permits(), 0);
    
    drop(permit1);
    assert_eq!(limiter.available_permits(), 1);
    
    drop(permit2);
    assert_eq!(limiter.available_permits(), 2);
}

// ============================================================================
// Tests for MockMediaStoreBridge (on non-Android platforms)
// ============================================================================

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_mock_bridge_query_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
    
    let result = bridge.query_file("nonexistent.jpg").await;
    assert!(matches!(result, Err(MediaStoreError::NotFound(_))));
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_mock_bridge_create_and_query() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
    
    // Create a file
    let fd = bridge
        .open_fd_for_write("test.jpg", "image/jpeg", "DCIM/", MediaStoreCollection::Images)
        .await;
    #[cfg(unix)]
    assert!(fd.is_ok());
    #[cfg(not(unix))]
    assert!(fd.is_err()); // Expected to fail on non-Unix
    
    // Query should succeed
    let result = bridge.query_file("DCIM/test.jpg").await;
    assert!(result.is_ok());
    let query = result.unwrap();
    assert_eq!(query.display_name, "test.jpg");
    assert_eq!(query.mime_type, "image/jpeg");
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_mock_bridge_list_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
    
    let results = bridge.query_files("DCIM/").await;
    // Directory doesn't exist, so results should be empty
    assert_eq!(results.unwrap().len(), 0);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_mock_bridge_delete_file() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());
    
    // Create a file
    let _ = bridge
        .open_fd_for_write("test.jpg", "image/jpeg", "DCIM/", MediaStoreCollection::Images)
        .await;
    
    // Delete it
    let result = bridge.delete_file("DCIM/test.jpg").await;
    assert!(result.is_ok());
    
    // Query should fail
    let result = bridge.query_file("DCIM/test.jpg").await;
    assert!(matches!(result, Err(MediaStoreError::NotFound(_))));
}

// ============================================================================
// Tests for AndroidMediaStoreBackend
// ============================================================================

#[test]
fn test_backend_normalize_path() {
    let backend = AndroidMediaStoreBackend::new();
    
    // Simple path
    assert_eq!(
        backend.normalize_path(Path::new("photo.jpg")),
        PathBuf::from("photo.jpg")
    );
    
    // Path with leading slash
    assert_eq!(
        backend.normalize_path(Path::new("/DCIM/photo.jpg")),
        PathBuf::from("DCIM/photo.jpg")
    );
    
    // Path with .. (should be resolved)
    assert_eq!(
        backend.normalize_path(Path::new("/DCIM/../photo.jpg")),
        PathBuf::from("photo.jpg")
    );
    
    // Path with . (should be removed)
    assert_eq!(
        backend.normalize_path(Path::new("./DCIM/./photo.jpg")),
        PathBuf::from("DCIM/photo.jpg")
    );
}

#[test]
fn test_backend_validate_path_valid() {
    let backend = AndroidMediaStoreBackend::new();
    
    assert!(backend.validate_path(Path::new("photo.jpg")).is_ok());
    assert!(backend.validate_path(Path::new("/DCIM/photo.jpg")).is_ok());
    assert!(backend.validate_path(Path::new("DCIM/Camera/photo.jpg")).is_ok());
}

#[test]
fn test_backend_validate_path_null_bytes() {
    let backend = AndroidMediaStoreBackend::new();
    
    // Path with null byte should fail
    let path = Path::new("test\0.jpg");
    assert!(backend.validate_path(path).is_err());
}

#[test]
fn test_backend_resolve_path() {
    let backend = AndroidMediaStoreBackend::new();
    
    // Paths already starting with DCIM/ are preserved
    let resolved = backend.resolve_path(Path::new("DCIM/Camera/photo.jpg"));
    assert!(resolved.starts_with("DCIM/"));
    
    // Other paths get the base prefix
    let resolved = backend.resolve_path(Path::new("photo.jpg"));
    assert!(resolved.contains("CameraFTP"));
}

#[test]
fn test_backend_resolve_path_preserves_dcim() {
    let backend = AndroidMediaStoreBackend::new();
    
    // DCIM paths should be preserved
    let resolved = backend.resolve_path(Path::new("DCIM/test.jpg"));
    assert!(resolved.starts_with("DCIM/"));
    
    // Pictures paths should be preserved
    let resolved = backend.resolve_path(Path::new("Pictures/test.jpg"));
    assert!(resolved.starts_with("Pictures/"));
}

#[test]
fn test_metadata_from_query_result_file() {
    let query = QueryResult {
        content_uri: "content://media/external/images/media/1".to_string(),
        display_name: "test.jpg".to_string(),
        size: 12345,
        date_modified: 1609459200000,
        mime_type: "image/jpeg".to_string(),
        relative_path: "DCIM/".to_string(),
    };
    
    let metadata = MediaStoreMetadata::from(query);
    assert_eq!(metadata.len(), 12345);
    assert!(metadata.is_file());
    assert!(!metadata.is_dir());
    assert!(!metadata.is_symlink());
    assert_eq!(metadata.mime_type, "image/jpeg");
}

#[test]
fn test_metadata_from_query_result_directory() {
    let query = QueryResult {
        content_uri: "".to_string(),
        display_name: "DCIM/".to_string(),
        size: 0,
        date_modified: 0,
        mime_type: "".to_string(),
        relative_path: "".to_string(),
    };
    
    let metadata = MediaStoreMetadata::from(query);
    assert!(metadata.is_dir());
    assert!(!metadata.is_file());
}

// ============================================================================
// Integration-style tests for backend operations (using mock bridge)
// ============================================================================

#[cfg(not(target_os = "android"))]
fn create_test_backend() -> (AndroidMediaStoreBackend, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let bridge = Arc::new(MockMediaStoreBridge::new(temp_dir.path().to_path_buf()));
    let backend = AndroidMediaStoreBackend::with_bridge(bridge);
    (backend, temp_dir)
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_empty_directory() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    
    let result = backend.list(&user, Path::new("/")).await;
    // Empty directory should return empty list
    assert!(result.is_ok());
    let files = result.unwrap();
    assert!(files.is_empty());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_nonexistent() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    
    let result = backend.metadata(&user, Path::new("nonexistent.jpg")).await;
    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_mkd_and_list() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    
    // MKD is intentionally unsupported in current single-mount mode.
    let result = backend.mkd(&user, Path::new("testdir")).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::CommandNotImplemented);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_put_rejects_non_media_files_with_explicit_kind() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;

    let result = backend
        .put(&user, tokio::io::empty(), Path::new("notes.txt"), 0)
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::FileNameNotAllowedError);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_cwd_existing_virtual_directory_succeeds() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let nested_dir = temp_dir.path().join("DCIM/CameraFTP/album");
    std::fs::create_dir_all(&nested_dir).expect("create nested dir");
    std::fs::write(nested_dir.join("photo.jpg"), b"jpeg").expect("write nested file");

    let result = backend.cwd(&user, Path::new("/album")).await;

    assert!(result.is_ok());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_cwd_missing_virtual_directory_returns_directory_not_available() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;

    let result = backend.cwd(&user, Path::new("/missing")).await;

    assert!(result.is_ok());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_nested_virtual_directory_returns_files() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let nested_dir = temp_dir.path().join("DCIM/CameraFTP/album");
    std::fs::create_dir_all(&nested_dir).expect("create nested dir");
    std::fs::write(nested_dir.join("photo.jpg"), b"jpeg").expect("write nested file");

    let result = backend.list(&user, Path::new("/album")).await;

    assert!(result.is_ok());
    let files = result.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, PathBuf::from("photo.jpg"));
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_missing_virtual_directory_returns_empty_listing() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;

    let result = backend.list(&user, Path::new("/missing")).await;

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_del_nonexistent() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    
    // Deleting nonexistent file should fail
    let result = backend.del(&user, Path::new("nonexistent.jpg")).await;
    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_rename_not_supported() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    
    // Rename should return unsupported error
    let result = backend.rename(&user, Path::new("old.jpg"), Path::new("new.jpg")).await;
    assert!(result.is_err());
}

#[test]
fn test_backend_name() {
    let backend = AndroidMediaStoreBackend::new();
    assert_eq!(backend.name(), "AndroidMediaStore");
}

#[test]
fn test_backend_supported_features() {
    let backend = AndroidMediaStoreBackend::new();
    assert_eq!(backend.supported_features(), 0);
}
