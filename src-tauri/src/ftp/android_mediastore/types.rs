// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Types and DTOs for Android MediaStore storage backend.
//!
//! This module defines the data structures used for communication between
//! the Rust storage backend and the Android JNI bridge.

use std::path::PathBuf;
use thiserror::Error;

/// MIME types supported by MediaStore for image files.
pub const MIME_TYPE_JPEG: &str = "image/jpeg";
pub const MIME_TYPE_HEIF: &str = "image/heif";
pub const MIME_TYPE_MP4: &str = "video/mp4";
pub const MIME_TYPE_MOV: &str = "video/quicktime";

/// Default MIME type for unknown files.
pub const MIME_TYPE_DEFAULT: &str = "application/octet-stream";

/// Target MediaStore collection for an upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStoreCollection {
    Images,
    Videos,
    Downloads,
}

impl MediaStoreCollection {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaStoreCollection::Images => "images",
            MediaStoreCollection::Videos => "videos",
            MediaStoreCollection::Downloads => "downloads",
        }
    }
}

/// Result of a MediaStore insert operation.
#[derive(Debug, Clone)]
pub struct InsertResult {
    /// Content URI of the inserted file (e.g., content://media/external/images/media/123)
    pub content_uri: String,
    /// The display name of the file
    pub display_name: String,
}

/// Result of a MediaStore query operation.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Content URI of the file
    pub content_uri: String,
    /// Display name (filename)
    pub display_name: String,
    /// File size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch millis)
    pub date_modified: u64,
    /// MIME type
    pub mime_type: String,
    /// Relative path within the collection (e.g., "DCIM/Camera/")
    pub relative_path: String,
}

impl QueryResult {
    /// Returns true if this entry is a directory (based on MIME type).
    pub fn is_directory(&self) -> bool {
        self.mime_type == "inode/directory" || self.mime_type.is_empty() && self.display_name.ends_with('/')
    }
}

/// File descriptor wrapper for Android ParcelFileDescriptor.
#[derive(Debug)]
pub struct FileDescriptorInfo {
    /// The raw file descriptor (only valid on Unix/Android).
    #[cfg(unix)]
    pub fd: i32,
    /// MediaStore content URI for this entry.
    pub content_uri: String,
    /// The file path for reference.
    pub path: PathBuf,
}

/// Error type for MediaStore operations.
#[derive(Debug, Error)]
pub enum MediaStoreError {
    #[error("Failed to open file descriptor: {0}")]
    OpenFdFailed(String),

    #[error("Failed to insert into MediaStore: {0}")]
    InsertFailed(String),

    #[error("Failed to query MediaStore: {0}")]
    QueryFailed(String),

    #[error("Failed to delete from MediaStore: {0}")]
    DeleteFailed(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Bridge error: {0}")]
    BridgeError(String),

    #[error("Operation cancelled")]
    Cancelled,
}

/// Trait for MediaStore bridge client.
///
/// This trait abstracts the JNI bridge to allow for testing with mock implementations.
#[async_trait::async_trait]
pub trait MediaStoreBridgeClient: Send + Sync + std::fmt::Debug {
    /// Opens a file descriptor for reading the file at the given path.
    ///
    /// Returns the file descriptor info on success.
    async fn open_fd_for_read(&self, path: &str) -> Result<FileDescriptorInfo, MediaStoreError>;

    /// Opens a file descriptor for writing a new file.
    ///
    /// Returns the file descriptor info on success.
    async fn open_fd_for_write(
        &self,
        display_name: &str,
        mime_type: &str,
        relative_path: &str,
        collection: MediaStoreCollection,
    ) -> Result<FileDescriptorInfo, MediaStoreError>;

    /// Finalizes a MediaStore entry after write completion.
    async fn finalize_entry(
        &self,
        content_uri: &str,
        expected_size: Option<u64>,
    ) -> Result<(), MediaStoreError>;

    /// Aborts a MediaStore entry and removes its pending row.
    async fn abort_entry(&self, content_uri: &str) -> Result<(), MediaStoreError>;

    /// Queries files in the given directory path.
    ///
    /// Returns a list of query results.
    async fn query_files(&self, path: &str) -> Result<Vec<QueryResult>, MediaStoreError>;

    /// Queries a single file's metadata.
    ///
    /// Returns the query result or NotFound error.
    async fn query_file(&self, path: &str) -> Result<QueryResult, MediaStoreError>;

    /// Deletes a file from MediaStore.
    async fn delete_file(&self, path: &str) -> Result<(), MediaStoreError>;

    /// Creates a directory in MediaStore (via relative path convention).
    async fn create_directory(&self, path: &str) -> Result<(), MediaStoreError>;
}

/// Extracts the display name (filename) from a path.
///
/// # Examples
/// ```
/// use camera_ftp_companion_lib::ftp::android_mediastore::types::display_name_from_path;
/// assert_eq!(display_name_from_path("/DCIM/Camera/photo.jpg"), "photo.jpg");
/// assert_eq!(display_name_from_path("photo.jpg"), "photo.jpg");
/// ```
pub fn display_name_from_path(path: &str) -> String {
    let path = path.trim_start_matches('/');
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Extracts the relative directory path from a full path.
///
/// Returns the parent directory path, or empty string if the path has no parent.
///
/// # Examples
/// ```
/// use camera_ftp_companion_lib::ftp::android_mediastore::types::relative_path_from_full_path;
/// assert_eq!(relative_path_from_full_path("/DCIM/Camera/photo.jpg"), "DCIM/Camera/");
/// assert_eq!(relative_path_from_full_path("photo.jpg"), "");
/// ```
pub fn relative_path_from_full_path(path: &str) -> String {
    let path = path.trim_start_matches('/');
    if let Some(pos) = path.rfind('/') {
        path[..=pos].to_string()
    } else {
        String::new()
    }
}

/// Determines the MIME type from a file extension.
pub fn mime_type_from_filename(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        MIME_TYPE_JPEG
    } else if lower.ends_with(".heif") || lower.ends_with(".heic") {
        MIME_TYPE_HEIF
    } else if lower.ends_with(".mp4") {
        MIME_TYPE_MP4
    } else if lower.ends_with(".mov") {
        MIME_TYPE_MOV
    } else {
        MIME_TYPE_DEFAULT
    }
}

/// Routes a filename to a MediaStore collection.
///
/// Media files that need gallery refresh go to images/videos collections,
/// while all other files (including RAW formats) go to downloads.
pub fn collection_from_filename(filename: &str) -> MediaStoreCollection {
    let lower = filename.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".heif") || lower.ends_with(".heic") {
        MediaStoreCollection::Images
    } else if lower.ends_with(".mp4") || lower.ends_with(".mov") {
        MediaStoreCollection::Videos
    } else {
        MediaStoreCollection::Downloads
    }
}

/// Default relative path for camera uploads.
pub fn default_relative_path() -> &'static str {
    "DCIM/CameraFTP/"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_name_from_path() {
        assert_eq!(display_name_from_path("/DCIM/Camera/photo.jpg"), "photo.jpg");
        assert_eq!(display_name_from_path("photo.jpg"), "photo.jpg");
        assert_eq!(display_name_from_path("/photo.jpg"), "photo.jpg");
        assert_eq!(display_name_from_path("DCIM/photo.jpg"), "photo.jpg");
        assert_eq!(display_name_from_path(""), "");
    }

    #[test]
    fn test_relative_path_from_full_path() {
        assert_eq!(relative_path_from_full_path("/DCIM/Camera/photo.jpg"), "DCIM/Camera/");
        assert_eq!(relative_path_from_full_path("photo.jpg"), "");
        assert_eq!(relative_path_from_full_path("/photo.jpg"), "");
        assert_eq!(relative_path_from_full_path("DCIM/photo.jpg"), "DCIM/");
    }

    #[test]
    fn test_mime_type_from_filename() {
        assert_eq!(mime_type_from_filename("photo.jpg"), MIME_TYPE_JPEG);
        assert_eq!(mime_type_from_filename("photo.JPEG"), MIME_TYPE_JPEG);
        assert_eq!(mime_type_from_filename("photo.heif"), MIME_TYPE_HEIF);
        assert_eq!(mime_type_from_filename("video.mp4"), MIME_TYPE_MP4);
        assert_eq!(mime_type_from_filename("video.mov"), MIME_TYPE_MOV);
        assert_eq!(mime_type_from_filename("photo.dng"), MIME_TYPE_DEFAULT);
        assert_eq!(mime_type_from_filename("photo.cr2"), MIME_TYPE_DEFAULT);
        assert_eq!(mime_type_from_filename("photo.unknown"), MIME_TYPE_DEFAULT);
    }

    #[test]
    fn test_collection_from_filename() {
        assert_eq!(collection_from_filename("a.jpg"), MediaStoreCollection::Images);
        assert_eq!(collection_from_filename("a.heif"), MediaStoreCollection::Images);
        assert_eq!(collection_from_filename("a.mp4"), MediaStoreCollection::Videos);
        assert_eq!(collection_from_filename("a.mov"), MediaStoreCollection::Videos);
        assert_eq!(collection_from_filename("a.dng"), MediaStoreCollection::Downloads);
        assert_eq!(collection_from_filename("a.nef"), MediaStoreCollection::Downloads);
        assert_eq!(collection_from_filename("a.r3d"), MediaStoreCollection::Downloads);
        assert_eq!(collection_from_filename("a.bin"), MediaStoreCollection::Downloads);
    }

    #[test]
    fn test_query_result_is_directory() {
        let file = QueryResult {
            content_uri: "content://media/external/images/media/1".to_string(),
            display_name: "photo.jpg".to_string(),
            size: 1024,
            date_modified: 0,
            mime_type: "image/jpeg".to_string(),
            relative_path: "DCIM/".to_string(),
        };
        assert!(!file.is_directory());

        let dir = QueryResult {
            content_uri: "".to_string(),
            display_name: "DCIM/".to_string(),
            size: 0,
            date_modified: 0,
            mime_type: "".to_string(),
            relative_path: "".to_string(),
        };
        assert!(dir.is_directory());
    }
}
