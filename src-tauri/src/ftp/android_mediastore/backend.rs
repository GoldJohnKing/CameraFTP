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
    collection_from_filename, default_relative_path, display_name_from_path, mime_type_from_filename,
    relative_path_from_full_path, MediaStoreBridgeClient,
    MediaStoreError, QueryResult,
};
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::AsyncRead;
#[cfg(unix)]
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};
use unftp_core::auth::DefaultUser;
use unftp_core::storage::{
    Error as StorageError, ErrorKind as StorageErrorKind, Fileinfo, Metadata, StorageBackend,
};

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

    fn is_root_path(&self, path: &Path) -> bool {
        self.normalize_path(path).as_os_str().is_empty()
    }

    fn resolve_directory_path(&self, path: &Path) -> String {
        let resolved = self.resolve_path(path);
        if resolved.is_empty() || resolved.ends_with('/') {
            resolved
        } else {
            format!("{resolved}/")
        }
    }

    fn storage_error(kind: StorageErrorKind, message: impl Into<String>) -> StorageError {
        StorageError::new(kind, std::io::Error::other(message.into()))
    }

    fn unsupported_command(message: impl Into<String>) -> StorageError {
        Self::storage_error(StorageErrorKind::CommandNotImplemented, message)
    }

    fn file_name_not_allowed(message: impl Into<String>) -> StorageError {
        Self::storage_error(StorageErrorKind::FileNameNotAllowedError, message)
    }

    fn file_not_available(message: impl Into<String>) -> StorageError {
        Self::storage_error(StorageErrorKind::PermanentFileNotAvailable, message)
    }

    fn direct_child_name(directory_prefix: &str, entry: &QueryResult) -> Option<(String, bool)> {
        let entry_relative_path = entry.relative_path.trim_start_matches('/');
        let directory_prefix = directory_prefix.trim_start_matches('/');
        let remainder = entry_relative_path.strip_prefix(directory_prefix)?;

        if remainder.is_empty() {
            return Some((entry.display_name.clone(), false));
        }

        let child_name = remainder.split('/').next()?.trim();
        if child_name.is_empty() {
            None
        } else {
            Some((child_name.to_string(), true))
        }
    }

    fn synthesize_directory_info(name: String, modified: u64) -> Fileinfo<PathBuf, MediaStoreMetadata> {
        Fileinfo {
            path: PathBuf::from(name),
            metadata: MediaStoreMetadata {
                size: 0,
                modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(modified),
                is_dir: true,
                mime_type: "inode/directory".to_string(),
            },
        }
    }

    fn build_directory_listing(
        &self,
        directory_prefix: &str,
        results: Vec<QueryResult>,
    ) -> Vec<Fileinfo<PathBuf, MediaStoreMetadata>> {
        let mut directories = BTreeMap::<String, u64>::new();
        let mut files = Vec::new();

        for entry in results {
            if let Some((child_name, is_directory)) = Self::direct_child_name(directory_prefix, &entry) {
                if is_directory {
                    directories
                        .entry(child_name)
                        .and_modify(|modified| *modified = (*modified).max(entry.date_modified))
                        .or_insert(entry.date_modified);
                } else {
                    files.push(Fileinfo {
                        path: PathBuf::from(child_name),
                        metadata: MediaStoreMetadata::from(entry),
                    });
                }
            }
        }

        let mut listing = directories
            .into_iter()
            .map(|(name, modified)| Self::synthesize_directory_info(name, modified))
            .collect::<Vec<_>>();
        listing.extend(files);
        listing
    }

    async fn directory_exists(&self, path: &Path) -> Result<bool, StorageError> {
        if self.is_root_path(path) {
            return Ok(true);
        }

        let directory_prefix = self.resolve_directory_path(path);
        let bridge = self.bridge.clone();
        let results = retry_with_backoff(&self.retry_config, "query_directory", || {
            let bridge = bridge.clone();
            let directory_prefix = directory_prefix.clone();
            async move { bridge.query_files(&directory_prefix).await }
        })
        .await
        .map_err(|e| Self::file_not_available(e.to_string()))?;

        Ok(!results.is_empty())
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
    #[allow(dead_code)]
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

        if self.is_root_path(path) {
            return Ok(MediaStoreMetadata {
                size: 0,
                modified: SystemTime::now(),
                is_dir: true,
                mime_type: "inode/directory".to_string(),
            });
        }
        
        let full_path = self.resolve_path(path);
        debug!(path = %full_path, "Getting metadata");

        let bridge = self.bridge.clone();
        let result = retry_with_backoff(&self.retry_config, "metadata", || {
            let bridge = bridge.clone();
            let path = full_path.clone();
            async move { bridge.query_file(&path).await }
        })
        .await;

        match result {
            Ok(result) => Ok(MediaStoreMetadata::from(result)),
            Err(MediaStoreError::NotFound(_)) => {
                if self.directory_exists(path).await? {
                    Ok(Self::synthesize_directory_info(
                        display_name_from_path(&self.normalize_path(path).to_string_lossy()),
                        0,
                    )
                    .metadata)
                } else {
                    Err(Self::file_not_available(format!("Metadata not found: {}", path.display())))
                }
            }
            Err(e) => {
                error!(error = ?e, "Failed to get metadata");
                Err(Self::file_not_available(e.to_string()))
            }
        }
    }

    async fn list<P>(&self, _user: &DefaultUser, path: P) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let directory_prefix = self.resolve_directory_path(path);
        debug!(path = %directory_prefix, "Listing directory");

        let bridge = self.bridge.clone();
        let results = retry_with_backoff(&self.retry_config, "list", || {
            let bridge = bridge.clone();
            let path = directory_prefix.clone();
            async move { bridge.query_files(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to list directory");
            Self::file_not_available(e.to_string())
        })?;

        let files = self.build_directory_listing(&directory_prefix, results);

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
            Self::file_not_available(e.to_string())
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
            Err(Self::unsupported_command(
                "File descriptor operations not supported on this platform",
            ))
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
        let collection = collection_from_filename(&display_name);

        debug!(
            path = %path.display(),
            display_name = %display_name,
            relative_path = %parent_path,
            mime_type = %mime_type,
            collection = %collection.as_str(),
            start_pos,
            "Uploading file"
        );

        if start_pos > 0 {
            return Err(Self::unsupported_command(
                "Resume upload is not supported for Android MediaStore backend",
            ));
        }

        if !matches!(collection, super::types::MediaStoreCollection::Images | super::types::MediaStoreCollection::Videos) {
            return Err(Self::file_name_not_allowed(format!(
                "Only image and video uploads are supported by the Android MediaStore backend: {}",
                display_name
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
            let collection = collection;
            async move {
                bridge
                    .open_fd_for_write(&display_name, &mime_type, &parent_path, collection)
                    .await
            }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to open file descriptor for write");
            Self::file_not_available(e.to_string())
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
                Self::file_not_available(e.to_string())
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
                Self::file_not_available(e.to_string())
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
            Err(Self::unsupported_command(
                "File descriptor operations not supported on this platform",
            ))
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
            Self::file_not_available(e.to_string())
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

        Err(Self::unsupported_command(format!(
            "MKD is currently unsupported: {}",
            path.display()
        )))
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

        Err(Self::unsupported_command("Rename not supported in MediaStore backend"))
    }

    async fn rmd<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;

        Err(Self::unsupported_command(format!(
            "RMD is currently unsupported: {}",
            path.display()
        )))
    }

    async fn cwd<P>(&self, _user: &DefaultUser, path: P) -> Result<(), StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;

        if self.is_root_path(path) {
            debug!("Changing directory to FTP root");
            return Ok(());
        }

        debug!(path = %path.display(), "Changing directory to virtual subdirectory");
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
    use super::super::bridge::MockMediaStoreBridge;
    use tempfile::TempDir;
    use unftp_core::storage::ErrorKind as StorageErrorKind;

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

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn test_cwd_root_should_succeed_when_base_directory_exists() {
        let temp_dir = TempDir::new().expect("temp dir");
        let base_path = temp_dir.path().to_path_buf();
        let base_directory = base_path.join("DCIM/CameraFTP");
        std::fs::create_dir_all(&base_directory).expect("create base directory");

        let bridge = Arc::new(MockMediaStoreBridge::new(base_path));
        let backend = AndroidMediaStoreBackend::with_bridge(bridge);

        let result = backend.cwd(&DefaultUser {}, Path::new("/")).await;
        assert!(result.is_ok(), "cwd / should succeed when base directory exists");
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn test_cwd_missing_subdirectory_should_succeed() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let result = backend.cwd(&DefaultUser {}, Path::new("/subdir")).await;
        assert!(result.is_ok(), "cwd subdirectory should succeed for virtual directories");
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn test_mkd_and_rmd_should_be_command_not_implemented() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let mkd_result = backend.mkd(&DefaultUser {}, Path::new("/newdir")).await;
        assert!(mkd_result.is_err(), "mkd should fail as unsupported");
        assert_eq!(mkd_result.unwrap_err().kind(), StorageErrorKind::CommandNotImplemented);

        let rmd_result = backend.rmd(&DefaultUser {}, Path::new("/newdir")).await;
        assert!(rmd_result.is_err(), "rmd should fail as unsupported");
        assert_eq!(rmd_result.unwrap_err().kind(), StorageErrorKind::CommandNotImplemented);
    }
}
