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
    classify_file, collection_from_class, default_relative_path,
    display_name_from_path, MediaFileClass, mime_type_from_filename, relative_path_from_full_path,
    MediaStoreCollection, MediaStoreError, QueryResult, MIME_TYPE_DEFAULT, MIME_TYPE_HEIF,
    MIME_TYPE_JPEG, MIME_TYPE_MP4,
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

#[test]
fn test_mime_type_raw_formats() {
    assert_eq!(mime_type_from_filename("photo.dng"), "image/x-adobe-dng");
    assert_eq!(mime_type_from_filename("photo.nef"), "image/x-nikon-nef");
    assert_eq!(mime_type_from_filename("photo.nrw"), "image/x-nikon-nrw");
    assert_eq!(mime_type_from_filename("photo.cr2"), "image/x-canon-cr2");
    assert_eq!(mime_type_from_filename("photo.cr3"), "image/x-canon-cr3");
    assert_eq!(mime_type_from_filename("photo.arw"), "image/x-sony-arw");
    assert_eq!(mime_type_from_filename("photo.sr2"), "image/x-sony-sr2");
    assert_eq!(mime_type_from_filename("photo.raf"), "image/x-fuji-raf");
    assert_eq!(mime_type_from_filename("photo.orf"), "image/x-olympus-orf");
    assert_eq!(mime_type_from_filename("photo.rw2"), "image/x-panasonic-rw2");
    assert_eq!(mime_type_from_filename("photo.pef"), "image/x-pentax-pef");
    assert_eq!(mime_type_from_filename("photo.x3f"), "image/x-sigma-x3f");
}

#[test]
fn test_mime_type_raw_formats_case_insensitive() {
    assert_eq!(mime_type_from_filename("PHOTO.DNG"), "image/x-adobe-dng");
    assert_eq!(mime_type_from_filename("Photo.NEF"), "image/x-nikon-nef");
    assert_eq!(mime_type_from_filename("Photo.CR3"), "image/x-canon-cr3");
}

#[test]
fn test_classify_file_routes_raw_to_images_via_collection_from_class() {
    for ext in &["dng", "nef", "nrw", "cr2", "cr3", "arw", "sr2", "raf", "orf", "rw2", "pef", "x3f"] {
        // On non-Android, classify_file returns NonMedia for RAW extensions.
        // Verify collection_from_class works correctly for both paths.
        let (_, class) = classify_file(&format!("photo.{ext}"));
        let collection = collection_from_class(class);
        // On Android, RAW files would be Image → Images. On non-Android, NonMedia → Downloads.
        // The mapping itself is what we test here.
        match class {
            MediaFileClass::Image => assert_eq!(collection, MediaStoreCollection::Images, ".{ext} Image → Images"),
            MediaFileClass::Video => assert_eq!(collection, MediaStoreCollection::Videos, ".{ext} Video → Videos"),
            MediaFileClass::NonMedia => assert_eq!(collection, MediaStoreCollection::Downloads, ".{ext} NonMedia → Downloads"),
        }
    }
}

#[test]
fn test_classify_file_unknown_keeps_files_in_downloads() {
    let (_, class) = classify_file("file.bin");
    assert_eq!(collection_from_class(class), MediaStoreCollection::Downloads);
    let (_, class) = classify_file("file.txt");
    assert_eq!(collection_from_class(class), MediaStoreCollection::Downloads);
}

#[test]
fn test_classify_file_unknown_extension_returns_non_media() {
    let (mime, class) = classify_file("unknown.xyz");
    assert_eq!(class, MediaFileClass::NonMedia);
    assert_eq!(mime, MIME_TYPE_DEFAULT);
}

#[test]
fn test_classify_file_empty_extension_returns_non_media() {
    let (mime, class) = classify_file("noextension");
    assert_eq!(class, MediaFileClass::NonMedia);
    assert_eq!(mime, MIME_TYPE_DEFAULT);
}

#[test]
fn test_collection_from_class_mapping() {
    assert_eq!(collection_from_class(MediaFileClass::Image), MediaStoreCollection::Images);
    assert_eq!(collection_from_class(MediaFileClass::Video), MediaStoreCollection::Videos);
    assert_eq!(collection_from_class(MediaFileClass::NonMedia), MediaStoreCollection::Downloads);
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
async fn test_mock_bridge_query_file_requires_exact_relative_path() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = MockMediaStoreBridge::new(temp_dir.path().to_path_buf());

    std::fs::create_dir_all(temp_dir.path().join("DCIM/subdir")).expect("create subdir");
    std::fs::write(temp_dir.path().join("DCIM/subdir/foo.jpg"), b"nested").expect("write nested file");

    let nested = bridge
        .query_file("DCIM/subdir/foo.jpg")
        .await
        .expect("exact nested path should resolve");
    assert_eq!(nested.relative_path, "DCIM/subdir/");

    let root_miss = bridge.query_file("DCIM/foo.jpg").await;
    assert!(matches!(root_miss, Err(MediaStoreError::NotFound(_))));
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
    
    // Only paths within virtual roots are preserved
    let resolved = backend.resolve_path(Path::new("DCIM/CameraFTP/photo.jpg"));
    assert_eq!(resolved, "DCIM/CameraFTP/photo.jpg");

    // Non-virtual-root path gets the base prefix
    let resolved = backend.resolve_path(Path::new("DCIM/Camera/photo.jpg"));
    assert_eq!(resolved, "DCIM/CameraFTP/DCIM/Camera/photo.jpg");
    
    // Other paths get the base prefix
    let resolved = backend.resolve_path(Path::new("photo.jpg"));
    assert!(resolved.contains("CameraFTP"));
}

#[test]
fn test_backend_resolve_path_only_preserves_explicit_virtual_root_paths() {
    let backend = AndroidMediaStoreBackend::new();
    
    // DCIM virtual-root paths should be preserved
    let resolved = backend.resolve_path(Path::new("DCIM/test.jpg"));
    assert_eq!(resolved, "DCIM/CameraFTP/DCIM/test.jpg");

    let resolved = backend.resolve_path(Path::new("DCIM/CameraFTP/test.jpg"));
    assert_eq!(resolved, "DCIM/CameraFTP/test.jpg");
    
    // Pictures paths are not virtual roots and should not be preserved
    let resolved = backend.resolve_path(Path::new("Pictures/test.jpg"));
    assert_eq!(resolved, "DCIM/CameraFTP/Pictures/test.jpg");

    // Download paths should be preserved
    let resolved = backend.resolve_path(Path::new("Download/CameraFTP/test.jpg"));
    assert_eq!(resolved, "Download/CameraFTP/test.jpg");

    // Explicit rooted Download paths should be preserved after normalization
    let resolved = backend.resolve_path(Path::new("/Download/CameraFTP/rooted-test.jpg"));
    assert_eq!(resolved, "Download/CameraFTP/rooted-test.jpg");

    // Arbitrary Download paths should not bypass configured virtual root
    let resolved = backend.resolve_path(Path::new("Download/Other/test.jpg"));
    assert!(resolved.starts_with("DCIM/CameraFTP/"));
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
async fn test_backend_metadata_falls_back_to_download_root_for_file_lookup() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP")).expect("create Download root");
    std::fs::write(temp_dir.path().join("Download/CameraFTP/download-only.txt"), b"download-only")
        .expect("write Download fallback file");

    let metadata = backend
        .metadata(&user, Path::new("/download-only.txt"))
        .await
        .expect("metadata should resolve from Download root");

    assert!(metadata.is_file());
    assert_eq!(metadata.len(), b"download-only".len() as u64);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_get_falls_back_to_download_root_for_file_lookup() {
    #[cfg(unix)]
    use tokio::io::AsyncReadExt;

    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP")).expect("create Download root");
    std::fs::write(temp_dir.path().join("Download/CameraFTP/download-read.txt"), b"download-read")
        .expect("write Download fallback file");

    let result = backend.get(&user, Path::new("/download-read.txt"), 0).await;

    #[cfg(unix)]
    {
        let mut reader = result.expect("get should resolve from Download root");
        let mut content = Vec::new();
        reader
            .read_to_end(&mut content)
            .await
            .expect("read file content");
        assert_eq!(content, b"download-read");
    }

    #[cfg(not(unix))]
    {
        match result {
            Ok(_) => panic!("non-unix path should fail due unsupported FDs"),
            Err(error) => {
                assert!(
                    !error.to_string().contains("File not found"),
                    "lookup should resolve against Download root before non-unix FD limitation: {error}"
                );
            }
        }
    }
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
async fn test_backend_put_accepts_non_media_files() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;

    // Non-media files are now accepted (routed to Downloads collection).
    // On non-Android (mock bridge), the FD write may fail for platform reasons,
    // but it should NOT fail with FileNameNotAllowedError.
    let result = backend
        .put(&user, tokio::io::empty(), Path::new("notes.txt"), 0)
        .await;

    match result {
        Ok(_) => {},
        Err(e) => {
            assert_ne!(
                e.kind(),
                ErrorKind::FileNameNotAllowedError,
                "Non-media files should be accepted, not rejected: {e:?}"
            );
        }
    }
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

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::PermanentDirectoryNotAvailable);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_cwd_accepts_directory_present_only_in_download_root() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let nested_dir = temp_dir.path().join("Download/CameraFTP/download-only-dir");
    std::fs::create_dir_all(&nested_dir).expect("create Download-only directory");
    std::fs::write(nested_dir.join("inside.jpg"), b"inside").expect("write Download-only file");

    let result = backend.cwd(&user, Path::new("/download-only-dir")).await;

    assert!(result.is_ok());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_empty_download_directory_is_not_present_without_descendants() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/empty-dir"))
        .expect("create empty Download directory");

    let result = backend.metadata(&user, Path::new("/empty-dir")).await;
    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_uses_directory_modified_when_available() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let dir_path = temp_dir.path().join("Download/CameraFTP/album-with-file");
    std::fs::create_dir_all(&dir_path).expect("create directory");
    std::fs::write(dir_path.join("new.jpg"), b"new").expect("write directory child file");

    let metadata = backend
        .metadata(&user, Path::new("/album-with-file"))
        .await
        .expect("metadata should resolve directory");

    assert!(metadata.is_dir());
    assert!(
        metadata
            .modified()
            .expect("directory modified")
            .duration_since(std::time::UNIX_EPOCH)
            .expect("directory modified since epoch")
            .as_millis()
            > 0
    );
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
async fn test_backend_list_missing_virtual_directory_returns_error() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;

    let result = backend.list(&user, Path::new("/missing")).await;

    match result {
        Ok(_) => panic!("missing virtual directory should return an error"),
        Err(error) => assert_eq!(error.kind(), ErrorKind::PermanentDirectoryNotAvailable),
    }
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_cwd_rejects_existing_empty_directory_without_descendants() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/empty-cwd-dir"))
        .expect("create empty directory");

    let result = backend.cwd(&user, Path::new("/empty-cwd-dir")).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::PermanentDirectoryNotAvailable);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_cwd_rejects_empty_explicit_rooted_directory_without_descendants() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/explicit-cwd-dir"))
        .expect("create explicit rooted directory");

    let result = backend
        .cwd(&user, Path::new("/Download/CameraFTP/explicit-cwd-dir"))
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::PermanentDirectoryNotAvailable);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_rejects_existing_empty_virtual_directory_without_descendants() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/empty-list-dir"))
        .expect("create empty Download directory");

    let result = backend.list(&user, Path::new("/empty-list-dir")).await;
    assert!(result.is_err());
    match result {
        Ok(_) => panic!("empty virtual directory without descendants should be missing"),
        Err(error) => assert_eq!(error.kind(), ErrorKind::PermanentDirectoryNotAvailable),
    }
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_directory_not_confused_by_unrelated_same_display_name_file() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/target-dir"))
        .expect("create target directory");
    std::fs::write(
        temp_dir
            .path()
            .join("Download/CameraFTP/target-dir/inside.jpg"),
        b"inside",
    )
    .expect("write target directory child");

    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP/unrelated"))
        .expect("create unrelated directory");
    std::fs::write(
        temp_dir.path().join("Download/CameraFTP/unrelated/target-dir"),
        b"unrelated file with same display name",
    )
    .expect("write unrelated same-name file");

    let metadata = backend
        .metadata(&user, Path::new("/target-dir"))
        .await
        .expect("directory should resolve from descendants");

    assert!(metadata.is_dir());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_accepts_explicit_rooted_directory_path() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let rooted_dir = temp_dir.path().join("Download/CameraFTP/explicit-list-dir");
    std::fs::create_dir_all(&rooted_dir).expect("create explicit rooted directory");
    std::fs::write(rooted_dir.join("inside.jpg"), b"inside").expect("write nested file");

    let result = backend
        .list(&user, Path::new("/Download/CameraFTP/explicit-list-dir"))
        .await
        .expect("list explicit rooted directory");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].path, PathBuf::from("inside.jpg"));
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_merges_virtual_root_and_subdirectory() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_album = temp_dir.path().join("DCIM/CameraFTP/album");
    let download_album = temp_dir.path().join("Download/CameraFTP/album");
    std::fs::create_dir_all(&dcim_album).expect("create DCIM album");
    std::fs::create_dir_all(&download_album).expect("create Download album");
    std::fs::write(dcim_album.join("dcim.jpg"), b"dcim").expect("write DCIM file");
    std::fs::write(download_album.join("download.jpg"), b"download").expect("write Download file");

    let root_listing = backend
        .list(&user, Path::new("/"))
        .await
        .expect("list root");
    let album_entry = root_listing
        .into_iter()
        .find(|entry| entry.path == PathBuf::from("album"))
        .expect("merged root should contain album directory");
    assert!(album_entry.metadata.is_dir());

    let album_listing = backend
        .list(&user, Path::new("/album"))
        .await
        .expect("list merged album");
    assert!(album_listing.iter().any(|entry| entry.path == PathBuf::from("dcim.jpg")));
    assert!(album_listing
        .iter()
        .any(|entry| entry.path == PathBuf::from("download.jpg")));
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_collision_precedence_for_virtual_roots() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(&downloads_root).expect("create Download root");

    std::fs::write(dcim_root.join("same.jpg"), b"dcim-preferred").expect("write DCIM same file");
    std::fs::write(downloads_root.join("same.jpg"), b"download").expect("write Download same file");

    std::fs::create_dir_all(dcim_root.join("dir-over-file")).expect("create DCIM directory");
    std::fs::write(dcim_root.join("dir-over-file/inside.jpg"), b"inside")
        .expect("write inside DCIM directory");
    std::fs::write(downloads_root.join("dir-over-file"), b"download-file")
        .expect("write Download file for dir collision");

    std::fs::write(dcim_root.join("file-over-dir"), b"dcim-file")
        .expect("write DCIM file for dir collision");
    std::fs::create_dir_all(downloads_root.join("file-over-dir"))
        .expect("create Download directory");
    std::fs::write(downloads_root.join("file-over-dir/inside.jpg"), b"inside")
        .expect("write inside Download directory");

    let listing = backend
        .list(&user, Path::new("/"))
        .await
        .expect("list root with collisions");

    let same_file = listing
        .iter()
        .find(|entry| entry.path == PathBuf::from("same.jpg"))
        .expect("same.jpg should be listed");
    assert!(same_file.metadata.is_file());
    assert_eq!(same_file.metadata.len(), b"dcim-preferred".len() as u64);

    let dir_over_file = listing
        .iter()
        .find(|entry| entry.path == PathBuf::from("dir-over-file"))
        .expect("dir-over-file should be listed");
    assert!(dir_over_file.metadata.is_dir());

    let file_over_dir = listing
        .iter()
        .find(|entry| entry.path == PathBuf::from("file-over-dir"))
        .expect("file-over-dir should be listed");
    assert!(file_over_dir.metadata.is_dir());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_prefers_directory_shape_when_virtual_roots_conflict() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(downloads_root.join("shape-collision")).expect("create Download directory");
    std::fs::write(dcim_root.join("shape-collision"), b"dcim-file").expect("write DCIM file");
    std::fs::write(downloads_root.join("shape-collision/inside.jpg"), b"inside")
        .expect("write inside Download directory");

    let metadata = backend
        .metadata(&user, Path::new("/shape-collision"))
        .await
        .expect("metadata should match merged virtual directory shape");

    assert!(metadata.is_dir());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_prefers_primary_root_for_file_file_collisions() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(&downloads_root).expect("create Download root");
    std::fs::write(dcim_root.join("same.txt"), b"dcim-preferred").expect("write DCIM file");
    std::fs::write(downloads_root.join("same.txt"), b"download").expect("write Download file");

    let metadata = backend
        .metadata(&user, Path::new("/same.txt"))
        .await
        .expect("metadata should resolve primary root file");

    assert!(metadata.is_file());
    assert_eq!(metadata.len(), b"dcim-preferred".len() as u64);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_get_rejects_directory_when_virtual_roots_collide_file_vs_directory() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(downloads_root.join("shape-collision")).expect("create Download directory");
    std::fs::write(dcim_root.join("shape-collision"), b"dcim-file").expect("write DCIM file");
    std::fs::write(downloads_root.join("shape-collision/inside.jpg"), b"inside")
        .expect("write inside Download directory");

    let result = backend.get(&user, Path::new("/shape-collision"), 0).await;
    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_del_rejects_directory_when_virtual_roots_collide_file_vs_directory() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(downloads_root.join("shape-collision")).expect("create Download directory");
    let colliding_file = dcim_root.join("shape-collision");
    std::fs::write(&colliding_file, b"dcim-file").expect("write DCIM file");
    std::fs::write(downloads_root.join("shape-collision/inside.jpg"), b"inside")
        .expect("write inside Download directory");

    let result = backend.del(&user, Path::new("/shape-collision")).await;
    assert!(result.is_err());
    assert!(colliding_file.exists());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_explicit_rooted_dcim_path_resolves_without_double_prefix() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("DCIM/CameraFTP")).expect("create DCIM root");
    std::fs::write(temp_dir.path().join("DCIM/CameraFTP/explicit-rooted.jpg"), b"dcim-rooted")
        .expect("write explicit rooted file");

    let metadata = backend
        .metadata(&user, Path::new("/DCIM/CameraFTP/explicit-rooted.jpg"))
        .await
        .expect("metadata should resolve explicit rooted path directly");

    assert!(metadata.is_file());
    assert_eq!(metadata.len(), b"dcim-rooted".len() as u64);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_does_not_preserve_arbitrary_download_rooted_path_on_read_side() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/Other")).expect("create Download/Other");
    std::fs::write(temp_dir.path().join("Download/Other/foreign.jpg"), b"foreign")
        .expect("write foreign Download-rooted file");

    let result = backend
        .metadata(&user, Path::new("/Download/Other/foreign.jpg"))
        .await;

    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_does_not_preserve_non_virtual_dcim_rooted_path_on_read_side() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("DCIM/Other")).expect("create DCIM/Other");
    std::fs::write(temp_dir.path().join("DCIM/Other/foreign.jpg"), b"foreign")
        .expect("write foreign DCIM-rooted file");

    let result = backend
        .metadata(&user, Path::new("/DCIM/Other/foreign.jpg"))
        .await;

    assert!(result.is_err());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_get_explicit_rooted_download_cameraftp_path_uses_direct_lookup() {
    #[cfg(unix)]
    use tokio::io::AsyncReadExt;

    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let file_path = temp_dir.path().join("Download/CameraFTP/explicit-get.txt");
    std::fs::create_dir_all(file_path.parent().expect("parent dir")).expect("create parent dir");
    std::fs::write(&file_path, b"explicit-get").expect("write explicit rooted file");

    let result = backend
        .get(&user, Path::new("/Download/CameraFTP/explicit-get.txt"), 0)
        .await;

    #[cfg(unix)]
    {
        let mut reader = result.expect("explicit rooted get should resolve");
        let mut content = Vec::new();
        reader.read_to_end(&mut content).await.expect("read content");
        assert_eq!(content, b"explicit-get");
    }

    #[cfg(not(unix))]
    {
        assert!(result.is_err());
    }
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_get_prefers_primary_root_for_file_file_collisions() {
    #[cfg(unix)]
    use tokio::io::AsyncReadExt;

    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(&downloads_root).expect("create Download root");
    std::fs::write(dcim_root.join("same-get.txt"), b"dcim").expect("write DCIM file");
    std::fs::write(downloads_root.join("same-get.txt"), b"download").expect("write Download file");

    let result = backend.get(&user, Path::new("/same-get.txt"), 0).await;

    #[cfg(unix)]
    {
        let mut reader = result.expect("get should resolve primary root file");
        let mut content = Vec::new();
        reader.read_to_end(&mut content).await.expect("read content");
        assert_eq!(content, b"dcim");
    }

    #[cfg(not(unix))]
    {
        assert!(result.is_err());
        assert!(!temp_dir.path().join("DCIM/CameraFTP/same-get.txt").is_dir());
    }
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_dir_dir_collision_uses_max_modified_time() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_dir = temp_dir.path().join("DCIM/CameraFTP/same-dir");
    let download_dir = temp_dir.path().join("Download/CameraFTP/same-dir");
    std::fs::create_dir_all(&dcim_dir).expect("create DCIM directory");
    std::fs::create_dir_all(&download_dir).expect("create Download directory");

    // Write both files, then backdate the DCIM file so timestamps differ deterministically
    std::fs::write(dcim_dir.join("old.jpg"), b"old").expect("write DCIM file");
    std::fs::write(download_dir.join("new.jpg"), b"new").expect("write Download file");

    let old_time = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    filetime::set_file_mtime(
        &dcim_dir.join("old.jpg"),
        filetime::FileTime::from_system_time(old_time),
    )
    .expect("backdate DCIM file");

    let listing = backend
        .list(&user, Path::new("/"))
        .await
        .expect("list root with dir collision");

    let merged_dir = listing
        .iter()
        .find(|entry| entry.path == PathBuf::from("same-dir"))
        .expect("same-dir should be listed");
    assert!(merged_dir.metadata.is_dir());

    let expected_max_millis = std::fs::metadata(download_dir.join("new.jpg"))
        .expect("read new file metadata")
        .modified()
        .expect("new file modified time")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("new file since epoch")
        .as_millis();

    let merged_modified_millis = merged_dir
        .metadata
        .modified()
        .expect("directory modified time")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("directory modified since epoch")
        .as_millis();

    assert_eq!(merged_modified_millis, expected_max_millis);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_directory_uses_merged_max_modified_timestamp() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_dir = temp_dir.path().join("DCIM/CameraFTP/same-dir");
    let download_dir = temp_dir.path().join("Download/CameraFTP/same-dir");
    std::fs::create_dir_all(&dcim_dir).expect("create DCIM directory");
    std::fs::create_dir_all(&download_dir).expect("create Download directory");

    // Write both files, then backdate the DCIM file so timestamps differ deterministically
    std::fs::write(dcim_dir.join("old.jpg"), b"old").expect("write DCIM file");
    std::fs::write(download_dir.join("new.jpg"), b"new").expect("write Download file");

    let old_time = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    filetime::set_file_mtime(
        &dcim_dir.join("old.jpg"),
        filetime::FileTime::from_system_time(old_time),
    )
    .expect("backdate DCIM file");

    let expected_max_millis = std::fs::metadata(download_dir.join("new.jpg"))
        .expect("read new file metadata")
        .modified()
        .expect("new file modified time")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("new file since epoch")
        .as_millis();

    let metadata = backend
        .metadata(&user, Path::new("/same-dir"))
        .await
        .expect("metadata should resolve merged directory");
    assert!(metadata.is_dir());

    let modified_millis = metadata
        .modified()
        .expect("directory modified")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("directory modified since epoch")
        .as_millis();
    assert_eq!(modified_millis, expected_max_millis);
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
async fn test_backend_del_falls_back_to_download_root_for_file_lookup() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    std::fs::create_dir_all(temp_dir.path().join("Download/CameraFTP")).expect("create Download root");
    let file_path = temp_dir.path().join("Download/CameraFTP/download-delete.txt");
    std::fs::write(&file_path, b"delete-me").expect("write Download fallback file");

    backend
        .del(&user, Path::new("/download-delete.txt"))
        .await
        .expect("delete should resolve from Download root");

    assert!(!file_path.exists());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_del_prefers_primary_root_for_file_file_collisions() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_root = temp_dir.path().join("DCIM/CameraFTP");
    let downloads_root = temp_dir.path().join("Download/CameraFTP");
    std::fs::create_dir_all(&dcim_root).expect("create DCIM root");
    std::fs::create_dir_all(&downloads_root).expect("create Download root");
    let dcim_file = dcim_root.join("same-delete.txt");
    let download_file = downloads_root.join("same-delete.txt");
    std::fs::write(&dcim_file, b"dcim").expect("write DCIM file");
    std::fs::write(&download_file, b"download").expect("write Download file");

    backend
        .del(&user, Path::new("/same-delete.txt"))
        .await
        .expect("delete should resolve primary root file");

    assert!(!dcim_file.exists());
    assert!(download_file.exists());
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_del_explicit_rooted_dcim_cameraftp_path_uses_direct_lookup() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;
    let file_path = temp_dir.path().join("DCIM/CameraFTP/explicit-delete.txt");
    std::fs::create_dir_all(file_path.parent().expect("parent dir")).expect("create parent dir");
    std::fs::write(&file_path, b"delete-me").expect("write explicit rooted file");

    backend
        .del(&user, Path::new("/DCIM/CameraFTP/explicit-delete.txt"))
        .await
        .expect("explicit rooted delete should resolve");

    assert!(!file_path.exists());
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

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_put_allows_raw_files_routed_to_images() {
    let (backend, _temp_dir) = create_test_backend();
    let user = DefaultUser;
    let data = b"raw-data".to_vec();
    let reader = tokio::io::BufReader::new(std::io::Cursor::new(data.clone()));

    let result = backend.put(&user, reader, "/DCIM/CameraFTP/sample.dng", 0).await;

    // On Unix the mock bridge succeeds; on non-Unix FDs are unsupported so put()
    // fails with PermanentFileNotAvailable — but crucially NOT FileNameNotAllowedError,
    // which proves RAW files now pass the collection admission gate (Images).
    match result {
        Ok(_) => {},
        Err(e) => {
            assert_ne!(
                e.kind(),
                ErrorKind::FileNameNotAllowedError,
                "RAW files should be routed to Images collection, not rejected: {e:?}"
            );
        }
    }
}

#[cfg(all(not(target_os = "android"), unix))]
#[tokio::test]
async fn test_backend_non_media_upload_preserves_virtual_subdir_in_listing() {
    let temp_dir = TempDir::new().unwrap();
    let bridge = Arc::new(MockMediaStoreBridge::new(temp_dir.path().to_path_buf()));
    let backend = AndroidMediaStoreBackend::with_bridge(bridge);

    // Upload a non-media file to a virtual subdirectory
    let data = b"notes content".repeat(100);
    let reader = std::io::Cursor::new(data.clone());
    let bytes_written = backend
        .put(&DefaultUser {}, reader, "subdir/notes.txt", 0)
        .await
        .expect("put should succeed");
    assert_eq!(bytes_written, data.len() as u64);

    // The file should be listable under the virtual subdirectory
    let items = backend.list(&DefaultUser {}, "subdir").await.expect("list subdir should succeed");
    let names: Vec<String> = items.iter().map(|i| i.path.to_string_lossy().to_string()).collect();
    assert!(
        names.contains(&"notes.txt".to_string()),
        "Expected notes.txt in subdir listing, got: {names:?}"
    );
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_metadata_file_file_collision_at_nested_path_prefers_primary_root() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_sub = temp_dir.path().join("DCIM/CameraFTP/album");
    let download_sub = temp_dir.path().join("Download/CameraFTP/album");
    std::fs::create_dir_all(&dcim_sub).expect("create DCIM subdir");
    std::fs::create_dir_all(&download_sub).expect("create Download subdir");
    std::fs::write(dcim_sub.join("dup.jpg"), b"dcim").expect("write DCIM nested file");
    std::fs::write(download_sub.join("dup.jpg"), b"download").expect("write Download nested file");

    let metadata = backend
        .metadata(&user, Path::new("/album/dup.jpg"))
        .await
        .expect("metadata should resolve nested collision");
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), b"dcim".len() as u64);
}

#[cfg(not(target_os = "android"))]
#[tokio::test]
async fn test_backend_list_collision_at_nested_path_prefers_directory_over_file() {
    let (backend, temp_dir) = create_test_backend();
    let user = DefaultUser;

    let dcim_sub = temp_dir.path().join("DCIM/CameraFTP/album");
    let download_sub = temp_dir.path().join("Download/CameraFTP/album");
    std::fs::create_dir_all(&dcim_sub).expect("create DCIM subdir");
    std::fs::create_dir_all(&download_sub).expect("create Download subdir");

    // "nested" is a directory in DCIM, a file in Download
    std::fs::create_dir_all(dcim_sub.join("nested")).expect("create DCIM nested dir");
    std::fs::write(dcim_sub.join("nested/child.jpg"), b"child").expect("write child file");
    std::fs::write(download_sub.join("nested"), b"file").expect("write Download nested file");

    let listing = backend
        .list(&user, Path::new("/album"))
        .await
        .expect("list nested album");

    let nested = listing
        .into_iter()
        .find(|entry| entry.path == PathBuf::from("nested"))
        .expect("nested should be listed");
    assert!(nested.metadata.is_dir(), "directory should win over file at nested path");
}
