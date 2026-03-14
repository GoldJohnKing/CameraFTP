// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android MediaStore storage backend for libunftp.
//!
//! This module implements the `StorageBackend` trait from libunftp, allowing
//! FTP uploads to be stored directly in Android's MediaStore gallery.

use super::limiter::UploadLimiter;
use super::retry::{retry_with_backoff, RetryConfig};
use super::types::{
    default_relative_path, display_name_from_path, mime_type_from_filename,
    relative_path_from_full_path, MediaStoreBridgeClient,
    MediaStoreError, QueryResult,
};
use async_trait::async_trait;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::AsyncRead;
#[cfg(unix)]
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};
use unftp_core::auth::DefaultUser;
use unftp_core::storage::{Fileinfo, Metadata, StorageBackend, Error as StorageError};

/// Metadata implementation for MediaStore files.
#[derive(Debug, Clone)]
pub struct MediaStoreMetadata {
    /// File size in bytes.
    pub size: u64,
    /// Last modified time.
    pub modified: SystemTime,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// MIME type.
    pub mime_type: String,
}

impl Metadata for MediaStoreMetadata {
    fn len(&self) -> u64 {
        self.size
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn is_file(&self) -> bool {
        !self.is_dir
    }

    fn is_symlink(&self) -> bool {
        false
    }

    fn modified(&self) -> Result<SystemTime, StorageError> {
        Ok(self.modified)
    }

    fn gid(&self) -> u32 {
        0
    }

    fn uid(&self) -> u32 {
        0
    }
}

impl From<QueryResult> for MediaStoreMetadata {
    fn from(result: QueryResult) -> Self {
        Self {
            size: result.size,
            modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(result.date_modified),
            is_dir: result.is_directory(),
            mime_type: result.mime_type,
        }
    }
}

/// Android MediaStore storage backend for libunftp.
///
/// This backend stores uploaded files directly into the Android MediaStore,
/// making them immediately visible in the device's gallery.
#[derive(Debug)]
pub struct AndroidMediaStoreBackend {
    /// Bridge client for MediaStore operations.
    bridge: Arc<dyn MediaStoreBridgeClient>,
    /// Upload concurrency limiter.
    limiter: UploadLimiter,
    /// Retry configuration.
    retry_config: RetryConfig,
    /// Base relative path for uploads (e.g., "DCIM/CameraFTP/").
    base_relative_path: String,
}

impl AndroidMediaStoreBackend {
    /// Creates a new MediaStore backend with default settings.
    pub fn new() -> Self {
        Self {
            bridge: super::bridge::create_bridge(),
            limiter: UploadLimiter::default_limiter(),
            retry_config: RetryConfig::default(),
            base_relative_path: default_relative_path().to_string(),
        }
    }

    /// Creates a new MediaStore backend with a custom bridge (for testing).
    pub fn with_bridge(bridge: Arc<dyn MediaStoreBridgeClient>) -> Self {
        Self {
            bridge,
            limiter: UploadLimiter::default_limiter(),
            retry_config: RetryConfig::default(),
            base_relative_path: default_relative_path().to_string(),
        }
    }

    /// Creates a new MediaStore backend with custom settings.
    pub fn with_config(
        bridge: Arc<dyn MediaStoreBridgeClient>,
        limiter: UploadLimiter,
        retry_config: RetryConfig,
        base_relative_path: String,
    ) -> Self {
        Self {
            bridge,
            limiter,
            retry_config,
            base_relative_path,
        }
    }

    /// Normalizes a path by removing leading slashes and resolving "." and "..".
    #[cfg(test)]
    pub fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        let normalized = path_str.trim_start_matches('/');
        
        // Handle relative path components
        let mut components = Vec::new();
        for part in normalized.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    components.pop();
                }
                _ => components.push(part),
            }
        }
        
        PathBuf::from(components.join("/"))
    }

    #[cfg(not(test))]
    fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        let normalized = path_str.trim_start_matches('/');
        
        // Handle relative path components
        let mut components = Vec::new();
        for part in normalized.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    components.pop();
                }
                _ => components.push(part),
            }
        }
        
        PathBuf::from(components.join("/"))
    }

    /// Resolves a user-provided path to the full relative path in MediaStore.
    #[cfg(test)]
    pub fn resolve_path(&self, path: &Path) -> String {
        let normalized = self.normalize_path(path);
        
        // If path is absolute (starts with /), use it relative to base
        // If path is relative, use it directly
        let full_path = if normalized.starts_with("DCIM/") || normalized.starts_with("Pictures/") {
            normalized.to_string_lossy().to_string()
        } else {
            format!("{}{}", self.base_relative_path, normalized.to_string_lossy())
        };
        
        full_path
    }

    #[cfg(not(test))]
    fn resolve_path(&self, path: &Path) -> String {
        let normalized = self.normalize_path(path);
        
        // If path is absolute (starts with /), use it relative to base
        // If path is relative, use it directly
        let full_path = if normalized.starts_with("DCIM/") || normalized.starts_with("Pictures/") {
            normalized.to_string_lossy().to_string()
        } else {
            format!("{}{}", self.base_relative_path, normalized.to_string_lossy())
        };
        
        full_path
    }

    /// Validates a path for security (prevents directory traversal attacks).
    #[cfg(test)]
    pub fn validate_path(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy();
        
        // Check for null bytes
        if path_str.contains('\0') {
            return Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains null bytes",
            )));
        }
        
        // Check for absolute path escape attempts
        if path_str.contains("..") {
            // After normalization, ".." should be resolved.
            // If it still exists, it means someone tried to escape the root.
            let normalized = self.normalize_path(path);
            if normalized.to_string_lossy().contains("..") {
                return Err(StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Path traversal attempt detected",
                )));
            }
        }
        
        Ok(())
    }

    #[cfg(not(test))]
    fn validate_path(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy();
        
        // Check for null bytes
        if path_str.contains('\0') {
            return Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains null bytes",
            )));
        }
        
        // Check for absolute path escape attempts
        if path_str.contains("..") {
            // After normalization, ".." should be resolved.
            // If it still exists, it means someone tried to escape the root.
            let normalized = self.normalize_path(path);
            if normalized.to_string_lossy().contains("..") {
                return Err(StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Path traversal attempt detected",
                )));
            }
        }
        
        Ok(())
    }

    /// Writes data from a reader to a file descriptor.
    #[cfg(unix)]
    async fn write_to_fd<R>(&self, fd: i32, mut reader: R, start_pos: u64) -> Result<u64, MediaStoreError>
    where
        R: AsyncRead + Send + Sync + Unpin,
    {
        use std::fs::File;
        use std::io::{Seek, SeekFrom, Write};
        use std::os::fd::{FromRawFd, OwnedFd};

        // SAFETY: The fd should be valid and opened for writing
        let owned_fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = File::from(owned_fd);
        
        // Seek to start position if needed
        if start_pos > 0 {
            file.seek(SeekFrom::Start(start_pos))
                .map_err(MediaStoreError::IoError)?;
        }

        // Copy data from reader to file
        let mut buffer = vec![0u8; 8192];
        let mut total_written: u64 = start_pos;

        loop {
            let bytes_read = reader.read(&mut buffer).await.map_err(|e| {
                MediaStoreError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;
            
            if bytes_read == 0 {
                break;
            }
            
            file.write_all(&buffer[..bytes_read])?;
            total_written += bytes_read as u64;
        }

        file.sync_all()?;
        
        Ok(total_written - start_pos)
    }

    #[cfg(not(unix))]
    async fn write_to_fd<R>(&self, _fd: i32, _reader: R, _start_pos: u64) -> Result<u64, MediaStoreError>
    where
        R: AsyncRead + Send + Sync + Unpin,
    {
        Err(MediaStoreError::BridgeError(
            "File descriptors not supported on this platform".to_string(),
        ))
    }
}

impl Default for AndroidMediaStoreBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend<DefaultUser> for AndroidMediaStoreBackend {
    type Metadata = MediaStoreMetadata;

    async fn metadata<P>(&self, _user: &DefaultUser, path: P) -> Result<Self::Metadata, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Getting metadata");

        let bridge = self.bridge.clone();
        let result = retry_with_backoff(&self.retry_config, "metadata", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.query_file(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to get metadata");
            match e {
                MediaStoreError::NotFound(_) => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    e.to_string(),
                )),
                _ => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )),
            }
        })?;

        Ok(MediaStoreMetadata::from(result))
    }

    async fn list<P>(&self, _user: &DefaultUser, path: P) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Listing directory");

        let bridge = self.bridge.clone();
        let results = retry_with_backoff(&self.retry_config, "list", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.query_files(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to list directory");
            StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        let files: Vec<Fileinfo<PathBuf, Self::Metadata>> = results
            .into_iter()
            .map(|r| {
                let path = PathBuf::from(&r.display_name);
                Fileinfo {
                    path,
                    metadata: MediaStoreMetadata::from(r),
                }
            })
            .collect();

        Ok(files)
    }

    async fn get<P>(&self, _user: &DefaultUser, path: P, start_pos: u64) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, start_pos, "Getting file");

        let bridge = self.bridge.clone();
        let fd_info = retry_with_backoff(&self.retry_config, "open_fd_for_read", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.open_fd_for_read(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to open file descriptor for read");
            match e {
                MediaStoreError::NotFound(_) => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    e.to_string(),
                )),
                _ => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )),
            }
        })?;

        #[cfg(unix)]
        {
            use std::os::fd::{FromRawFd, OwnedFd};
            use tokio::io::AsyncSeekExt;

            // SAFETY: fd is detached from Java side and owned by Rust now.
            let owned_fd = unsafe { OwnedFd::from_raw_fd(fd_info.fd) };
            let std_file = std::fs::File::from(owned_fd);
            let mut tokio_file = tokio::fs::File::from_std(std_file);

            if start_pos > 0 {
                tokio_file
                    .seek(std::io::SeekFrom::Start(start_pos))
                    .await
                    .map_err(StorageError::from)?;
            }

            Ok(Box::new(tokio_file))
        }

        #[cfg(not(unix))]
        {
            let _ = fd_info;
            Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "File descriptor operations not supported on this platform",
            )))
        }
    }

    async fn put<P, R>(&self, _user: &DefaultUser, reader: R, path: P, start_pos: u64) -> Result<u64, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
        R: AsyncRead + Send + Sync + Unpin + 'static,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let display_name = display_name_from_path(&path.to_string_lossy());
        let relative_path = self.resolve_path(path);
        let parent_path = relative_path_from_full_path(&relative_path);
        let mime_type = mime_type_from_filename(&display_name);

        debug!(
            path = %path.display(),
            display_name = %display_name,
            relative_path = %parent_path,
            mime_type = %mime_type,
            start_pos,
            "Uploading file"
        );

        if start_pos > 0 {
            return Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Resume upload is not supported for Android MediaStore backend",
            )));
        }

        // Acquire upload slot (limit concurrency)
        let _permit = self.limiter.acquire().await;
        
        // Open file descriptor for writing with retry
        let bridge = self.bridge.clone();
        let fd_info = retry_with_backoff(&self.retry_config, "open_fd_for_write", || {
            let bridge = bridge.clone();
            let display_name = display_name.clone();
            let parent_path = parent_path.clone();
            let mime_type = mime_type.to_string();
            async move {
                bridge.open_fd_for_write(&display_name, &mime_type, &parent_path).await
            }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to open file descriptor for write");
            StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        // Write data to file descriptor
        #[cfg(unix)]
        {
            let bytes_written = self.write_to_fd(fd_info.fd, reader, start_pos).await.map_err(|e| {
                error!(error = ?e, "Failed to write to file descriptor");
                let bridge = self.bridge.clone();
                let content_uri = fd_info.content_uri.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(abort_err) = bridge.abort_entry(&content_uri).await {
                        error!(error = ?abort_err, uri = %content_uri, "Failed to abort MediaStore entry after write error");
                    }
                });
                StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

            let bridge = self.bridge.clone();
            let content_uri = fd_info.content_uri.clone();
            retry_with_backoff(&self.retry_config, "finalize_entry", || {
                let bridge = bridge.clone();
                let content_uri = content_uri.clone();
                async move { bridge.finalize_entry(&content_uri, Some(bytes_written)).await }
            })
            .await
            .map_err(|e| {
                error!(error = ?e, uri = %fd_info.content_uri, "Failed to finalize MediaStore entry");
                let bridge = self.bridge.clone();
                let content_uri = fd_info.content_uri.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(abort_err) = bridge.abort_entry(&content_uri).await {
                        error!(error = ?abort_err, uri = %content_uri, "Failed to abort MediaStore entry after finalize error");
                    }
                });
                StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

            info!(
                path = %path.display(),
                bytes = bytes_written,
                uri = %fd_info.content_uri,
                "File uploaded successfully"
            );

            Ok(bytes_written)
        }

        #[cfg(not(unix))]
        {
            let _ = (fd_info, reader, start_pos);
            Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "File descriptor operations not supported on this platform",
            )))
        }
    }

    async fn del<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Deleting file");

        let bridge = self.bridge.clone();
        retry_with_backoff(&self.retry_config, "delete", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.delete_file(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to delete file");
            match e {
                MediaStoreError::NotFound(_) => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    e.to_string(),
                )),
                _ => StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )),
            }
        })?;

        info!(path = %full_path, "File deleted successfully");
        Ok(())
    }

    async fn mkd<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Creating directory");

        // MediaStore doesn't have explicit directories, but we can create
        // a placeholder to track the directory structure
        let bridge = self.bridge.clone();
        retry_with_backoff(&self.retry_config, "mkd", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.create_directory(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to create directory");
            StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        info!(path = %full_path, "Directory created successfully");
        Ok(())
    }

    async fn rename<P>(&self, _user: &DefaultUser, from: P, to: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let from = from.as_ref();
        let to = to.as_ref();
        self.validate_path(from)?;
        self.validate_path(to)?;
        
        // MediaStore doesn't support direct rename.
        // We would need to copy and delete.
        warn!(
            from = %from.display(),
            to = %to.display(),
            "Rename operation not fully supported in MediaStore"
        );

        Err(StorageError::from(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Rename not supported in MediaStore backend",
        )))
    }

    async fn rmd<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Removing directory");

        let bridge = self.bridge.clone();
        retry_with_backoff(&self.retry_config, "rmd", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.delete_file(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to remove directory");
            StorageError::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        info!(path = %full_path, "Directory removed successfully");
        Ok(())
    }

    async fn cwd<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Changing directory");

        // Verify the directory exists
        let bridge = self.bridge.clone();
        let _ = retry_with_backoff(&self.retry_config, "cwd", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.query_file(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to change directory");
            StorageError::from(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {}", e),
            ))
        })?;

        Ok(())
    }

    fn name(&self) -> &str {
        "AndroidMediaStore"
    }

    fn supported_features(&self) -> u32 {
        // No special features supported
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        let backend = AndroidMediaStoreBackend::new();
        
        assert_eq!(backend.normalize_path(Path::new("/foo/bar")), PathBuf::from("foo/bar"));
        assert_eq!(backend.normalize_path(Path::new("foo/bar")), PathBuf::from("foo/bar"));
        assert_eq!(backend.normalize_path(Path::new("/foo/../bar")), PathBuf::from("bar"));
        assert_eq!(backend.normalize_path(Path::new("./foo")), PathBuf::from("foo"));
    }

    #[test]
    fn test_validate_path() {
        let backend = AndroidMediaStoreBackend::new();
        
        assert!(backend.validate_path(Path::new("test.jpg")).is_ok());
        assert!(backend.validate_path(Path::new("DCIM/test.jpg")).is_ok());
        
        // Null bytes should fail
        assert!(backend.validate_path(Path::new("test\0.jpg")).is_err());
    }

    #[test]
    fn test_resolve_path() {
        let backend = AndroidMediaStoreBackend::new();
        
        // Paths starting with DCIM/ or Pictures/ are used as-is
        assert!(backend.resolve_path(Path::new("DCIM/test.jpg")).starts_with("DCIM/"));
        
        // Other paths are prefixed with base path
        let resolved = backend.resolve_path(Path::new("test.jpg"));
        assert!(resolved.starts_with("DCIM/CameraFTP/"));
    }

    #[test]
    fn test_metadata_from_query_result() {
        let result = QueryResult {
            content_uri: "content://media/external/images/media/1".to_string(),
            display_name: "test.jpg".to_string(),
            size: 1024,
            date_modified: 1609459200000, // 2021-01-01 00:00:00 UTC
            mime_type: "image/jpeg".to_string(),
            relative_path: "DCIM/".to_string(),
        };
        
        let metadata = MediaStoreMetadata::from(result);
        assert_eq!(metadata.size, 1024);
        assert!(!metadata.is_dir);
        assert_eq!(metadata.mime_type, "image/jpeg");
    }
}
