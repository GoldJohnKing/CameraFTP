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
    classify_file, collection_from_class, default_relative_path, display_name_from_path,
    relative_path_from_full_path, MediaStoreBridgeClient,
    MediaStoreError, MediaStoreCollection, QueryResult,
};
use async_trait::async_trait;
use futures::future::join_all;
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

/// Default relative path for non-media file uploads.
const DOWNLOADS_RELATIVE_PATH: &str = "Download/CameraFTP/";

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

enum FileLookupResolution {
    File {
        candidate_path: String,
        entry: QueryResult,
    },
    Directory {
        modified: u64,
    },
    NotFound,
}

impl AndroidMediaStoreBackend {
    fn is_non_directory_query_error(error: &std::io::Error) -> bool {
        matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory)
            || error.raw_os_error() == Some(267)
    }

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
    pub(crate) fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        let normalized = path_str.trim_start_matches('/');
        
        let mut components = Vec::new();
        for part in normalized.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    // Defense-in-depth: validate_path rejects ".." components before
                    // normalize_path is reached, so this branch is unreachable on public
                    // API paths. Kept for safety in case normalize_path is called directly.
                    components.pop();
                }
                _ => components.push(part),
            }
        }
        
        PathBuf::from(components.join("/"))
    }

    /// Resolves a user-provided path to the full relative path in MediaStore.
    pub(crate) fn resolve_path(&self, path: &Path) -> String {
        let normalized = self.normalize_path(path);
        let normalized_str = normalized.to_string_lossy();
        let roots = self.listing_virtual_roots();
        let preserves_explicit_root = roots.iter().any(|root| {
            let normalized_root = root.trim_end_matches('/');
            Self::is_within_virtual_root(&normalized_str, normalized_root)
        });
        
        let full_path = if preserves_explicit_root {
            normalized_str.to_string()
        } else {
            format!("{}{}", self.base_relative_path, normalized_str)
        };
        
        full_path
    }

    fn effective_parent_path_for_upload(
        collection: MediaStoreCollection,
        relative_path: &str,
        parent_path: &str,
        base_relative_path: &str,
    ) -> String {
        if collection == MediaStoreCollection::Downloads {
            if relative_path.starts_with(DOWNLOADS_RELATIVE_PATH) {
                parent_path.to_string()
            } else {
                // Preserve virtual subdirectories: DCIM/CameraFTP/subdir/file.txt → Download/CameraFTP/subdir/
                let suffix = relative_path
                    .strip_prefix(base_relative_path)
                    .and_then(|remainder| remainder.rfind('/').map(|pos| &remainder[..=pos]))
                    .unwrap_or("");
                format!("{DOWNLOADS_RELATIVE_PATH}{suffix}")
            }
        } else {
            parent_path.to_string()
        }
    }

    /// Validates a path for security (prevents directory traversal attacks).
    pub(crate) fn validate_path(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy();
        
        if path_str.contains('\0') {
            return Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains null bytes",
            )));
        }

        // Reject if any path component is exactly ".."
        for component in path_str.split('/') {
            if component == ".." {
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

    fn normalize_virtual_root(root: &str) -> String {
        root.trim_start_matches('/').trim_end_matches('/').to_string() + "/"
    }

    fn listing_virtual_roots(&self) -> Vec<String> {
        let primary_root = Self::normalize_virtual_root(&self.base_relative_path);
        let secondary_root = Self::normalize_virtual_root(DOWNLOADS_RELATIVE_PATH);

        let mut roots = Vec::new();
        for root in [primary_root, secondary_root] {
            if !roots.contains(&root) {
                roots.push(root);
            }
        }

        roots
    }

    fn is_within_virtual_root(path: &str, root: &str) -> bool {
        if path == root {
            return true;
        }
        let root_bytes = root.as_bytes();
        let path_bytes = path.as_bytes();
        path_bytes.len() > root_bytes.len()
            && path_bytes[root_bytes.len()] == b'/'
            && path_bytes[..root_bytes.len()] == *root_bytes
    }

    fn virtual_path_candidates(&self, path: &Path, for_directory: bool) -> Vec<String> {
        let normalized = self.normalize_path(path);
        let normalized_str = normalized.to_string_lossy().trim_start_matches('/').to_string();
        let roots = self.listing_virtual_roots();

        let preserves_explicit_root = roots.iter().any(|root| {
            let normalized_root = root.trim_end_matches('/');
            Self::is_within_virtual_root(&normalized_str, normalized_root)
        });

        if preserves_explicit_root {
            if for_directory {
                let directory_path = if normalized_str.is_empty() {
                    String::new()
                } else {
                    format!("{}/", normalized_str.trim_end_matches('/'))
                };
                return vec![directory_path];
            }

            return vec![normalized_str];
        }

        let suffix = if for_directory {
            if normalized.as_os_str().is_empty() {
                String::new()
            } else {
                format!("{}/", normalized.to_string_lossy().trim_end_matches('/'))
            }
        } else {
            normalized.to_string_lossy().trim_start_matches('/').to_string()
        };

        roots
            .into_iter()
            .map(|root| format!("{root}{suffix}"))
            .collect()
    }

    pub(crate) fn virtual_directory_candidates(&self, path: &Path) -> Vec<String> {
        self.virtual_path_candidates(path, true)
    }

    pub(crate) fn virtual_file_candidates(&self, path: &Path) -> Vec<String> {
        self.virtual_path_candidates(path, false)
    }

    async fn resolve_file_lookup(&self, path: &Path) -> Result<FileLookupResolution, StorageError> {
        let file_candidates = self.virtual_file_candidates(path);
        let mut first_file_match: Option<(String, QueryResult)> = None;
        let mut secondary_file_collision: Option<String> = None;
        let mut found_directory_in_non_primary = false;

        // Query all file candidates in parallel to reduce latency
        let results: Vec<(usize, String, Result<QueryResult, MediaStoreError>)> =
            futures::future::join_all(file_candidates.into_iter().enumerate().map(|(index, candidate_path)| {
                let bridge = self.bridge.clone();
                let retry_config = self.retry_config.clone();
                async move {
                    let operation = if index == 0 { "query_file_primary" } else { "query_file_secondary" };
                    let result = retry_with_backoff(&retry_config, operation, || {
                        let bridge = bridge.clone();
                        let query_path = candidate_path.clone();
                        async move { bridge.query_file(&query_path).await }
                    })
                    .await;
                    (index, candidate_path, result)
                }
            }))
            .await;

        for (index, candidate_path, result) in results {
            match result {
                Ok(entry) => {
                    if entry.is_directory() {
                        if index > 0 {
                            found_directory_in_non_primary = true;
                        }
                        continue;
                    }

                    if first_file_match.is_none() {
                        first_file_match = Some((candidate_path, entry));
                    } else if secondary_file_collision.is_none() {
                        secondary_file_collision = Some(candidate_path);
                    }
                }
                Err(MediaStoreError::NotFound(_)) => {
                    continue;
                }
                Err(e) => {
                    error!(error = ?e, path = %candidate_path, "File lookup failed");
                    return Err(Self::file_not_available(e.to_string()));
                }
            }
        }

        // Only check for directory collision if there might be one:
        // - secondary file collision exists, or
        // - no file was found (might be a directory-only virtual path), or
        // - a non-primary candidate resolved as a directory (could shadow the primary file)
        if secondary_file_collision.is_some() || first_file_match.is_none() || found_directory_in_non_primary {
            if let Some(modified) = self.directory_modified_millis(path).await? {
                if let Some((primary_file_path, _)) = &first_file_match {
                    warn!(
                        path = %path.display(),
                        primary = %primary_file_path,
                        "Virtual path resolves to directory over file during read lookup"
                    );
                }
                return Ok(FileLookupResolution::Directory { modified });
            }
        }

        if let Some((candidate_path, entry)) = first_file_match {
            if let Some(secondary_path) = secondary_file_collision {
                warn!(
                    path = %path.display(),
                    primary = %candidate_path,
                    secondary = %secondary_path,
                    "Virtual path file/file collision resolved in favor of primary root"
                );
            }
            return Ok(FileLookupResolution::File {
                candidate_path,
                entry,
            });
        }

        Ok(FileLookupResolution::NotFound)
    }

    async fn directory_modified_millis(&self, path: &Path) -> Result<Option<u64>, StorageError> {
        let directory_candidates = self.virtual_directory_candidates(path);
        let bridge = self.bridge.clone();
        let retry_config = self.retry_config.clone();

        let all_results: Vec<Vec<QueryResult>> = join_all(
            directory_candidates.into_iter().map(|directory_path| {
                let bridge = bridge.clone();
                let retry_config = retry_config.clone();
                async move {
                    Self::query_directory_entries_with_bridge(
                        &bridge, &retry_config, "query_directory", &directory_path,
                    )
                    .await
                    .unwrap_or_default()
                }
            }),
        )
        .await;

        let mut found_directory = false;
        let mut max_modified = 0_u64;
        for results in all_results {
            if !results.is_empty() {
                found_directory = true;
                if let Some(candidate_modified) = results.iter().map(|entry| entry.date_modified).max() {
                    max_modified = max_modified.max(candidate_modified);
                }
            }
        }

        if found_directory {
            Ok(Some(max_modified))
        } else if self.is_root_path(path) {
            Ok(Some(0))
        } else {
            Ok(None)
        }
    }

    fn storage_error(kind: StorageErrorKind, message: impl Into<String>) -> StorageError {
        StorageError::new(kind, std::io::Error::other(message.into()))
    }

    fn unsupported_command(message: impl Into<String>) -> StorageError {
        Self::storage_error(StorageErrorKind::CommandNotImplemented, message)
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

    /// Queries directory entries with explicit bridge/retry_config for use in parallel closures.
    async fn query_directory_entries_with_bridge(
        bridge: &Arc<dyn MediaStoreBridgeClient>,
        retry_config: &RetryConfig,
        operation: &'static str,
        directory_path: &str,
    ) -> Result<Vec<QueryResult>, StorageError> {
        let bridge = bridge.clone();
        let query_path = directory_path.to_string();
        let retry_config = retry_config.clone();

        retry_with_backoff(&retry_config, operation, || {
            let bridge = bridge.clone();
            let query_path = query_path.clone();
            async move { bridge.query_files(&query_path).await }
        })
        .await
        .or_else(|e| match e {
            MediaStoreError::NotFound(_) => Ok(Vec::new()),
            MediaStoreError::IoError(io_error) if Self::is_non_directory_query_error(&io_error) => {
                Ok(Vec::new())
            }
            other => {
                error!(error = ?other, path = %directory_path, "Failed to query directory entries");
                Err(Self::file_not_available(other.to_string()))
            }
        })
    }

    fn merge_directory_listings(
        primary: Vec<Fileinfo<PathBuf, MediaStoreMetadata>>,
        secondary: Vec<Fileinfo<PathBuf, MediaStoreMetadata>>,
    ) -> Vec<Fileinfo<PathBuf, MediaStoreMetadata>> {
        let mut merged = BTreeMap::<String, Fileinfo<PathBuf, MediaStoreMetadata>>::new();

        for entry in primary {
            merged.insert(entry.path.to_string_lossy().to_string(), entry);
        }

        for entry in secondary {
            let name = entry.path.to_string_lossy().to_string();
            match merged.get_mut(&name) {
                None => {
                    merged.insert(name, entry);
                }
                Some(existing) => {
                    let existing_is_dir = existing.metadata.is_dir();
                    let incoming_is_dir = entry.metadata.is_dir();
                    let existing_path = existing.path.to_string_lossy().to_string();
                    let incoming_path = entry.path.to_string_lossy().to_string();

                    if existing_is_dir && incoming_is_dir {
                        if entry.metadata.modified > existing.metadata.modified {
                            existing.metadata.modified = entry.metadata.modified;
                        }
                    } else if !existing_is_dir && !incoming_is_dir {
                        warn!(
                            name = %name,
                            existing = %existing_path,
                            incoming = %incoming_path,
                            "Virtual root file/file collision resolved in favor of primary root"
                        );
                    } else if !existing_is_dir && incoming_is_dir {
                        warn!(
                            name = %name,
                            existing = %existing_path,
                            incoming = %incoming_path,
                            "Virtual root file/dir collision resolved in favor of directory"
                        );
                        *existing = entry;
                    } else {
                        warn!(
                            name = %name,
                            existing = %existing_path,
                            incoming = %incoming_path,
                            "Virtual root dir/file collision resolved in favor of directory"
                        );
                    }
                }
            }
        }

        merged.into_values().collect()
    }

    async fn directory_exists(&self, path: &Path) -> Result<bool, StorageError> {
        Ok(self.directory_modified_millis(path).await?.is_some())
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
            let modified = self.directory_modified_millis(path).await?.unwrap_or(0);
            return Ok(MediaStoreMetadata {
                size: 0,
                modified: SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(modified),
                is_dir: true,
                mime_type: "inode/directory".to_string(),
            });
        }
        
        match self.resolve_file_lookup(path).await? {
            FileLookupResolution::File { entry, .. } => Ok(MediaStoreMetadata::from(entry)),
            FileLookupResolution::Directory { modified } => Ok(Self::synthesize_directory_info(
                display_name_from_path(&self.normalize_path(path).to_string_lossy()),
                modified,
            )
            .metadata),
            FileLookupResolution::NotFound => {
                Err(Self::file_not_available(format!("Metadata not found: {}", path.display())))
            }
        }
    }

    async fn list<P>(&self, _user: &DefaultUser, path: P) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;

        let is_root = self.is_root_path(path);
        let directory_candidates = self.virtual_directory_candidates(path);
        debug!(path = %path.display(), candidates = ?directory_candidates, "Listing directory");

        // Query all directory candidates in parallel
        let candidate_results: Vec<(String, Vec<QueryResult>)> = join_all(
            directory_candidates.into_iter().enumerate().map(|(index, directory_candidate)| {
                let bridge = self.bridge.clone();
                let retry_config = self.retry_config.clone();
                async move {
                    let operation = if index == 0 { "list_primary" } else { "list_secondary" };
                    let results = Self::query_directory_entries_with_bridge(
                        &bridge, &retry_config, operation, &directory_candidate,
                    )
                    .await
                    .unwrap_or_default();
                    (directory_candidate, results)
                }
            }),
        )
        .await;

        let mut merged_listing = Vec::new();
        let mut any_candidate_had_results = false;
        for (directory_candidate, results) in candidate_results {
            if !results.is_empty() {
                any_candidate_had_results = true;
            }
            let listing = self.build_directory_listing(&directory_candidate, results);
            merged_listing = Self::merge_directory_listings(merged_listing, listing);
        }

        if !is_root && !any_candidate_had_results {
            return Err(Self::storage_error(
                StorageErrorKind::PermanentDirectoryNotAvailable,
                format!("Directory not found: {}", path.display()),
            ));
        }

        Ok(merged_listing)
    }

    async fn get<P>(&self, _user: &DefaultUser, path: P, start_pos: u64) -> Result<Box<dyn AsyncRead + Send + Sync + Unpin>, StorageError>
    where
        P: AsRef<Path> + Send + Debug,
    {
        let path = path.as_ref();
        self.validate_path(path)?;
        
        let resolved_file_path = match self.resolve_file_lookup(path).await? {
            FileLookupResolution::File { candidate_path, .. } => candidate_path,
            FileLookupResolution::Directory { .. } => {
                return Err(Self::file_not_available(format!(
                    "Path is a directory and cannot be opened as a file: {}",
                    path.display()
                )));
            }
            FileLookupResolution::NotFound => {
                return Err(Self::file_not_available(format!(
                    "File not found: {}",
                    path.display()
                )));
            }
        };
        debug!(path = %resolved_file_path, start_pos, "Getting file");

        let bridge = self.bridge.clone();
        let fd_info = retry_with_backoff(&self.retry_config, "open_fd_for_read", || {
            let bridge = bridge.clone();
            let path = resolved_file_path.clone();
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

        // Classify file using Android system MimeTypeMap
        let (mime_type, file_class) = classify_file(&display_name);
        let collection = collection_from_class(file_class);

        // For non-media files, override the relative path to Download/CameraFTP/
        // since MediaStore.Files only allows Download/ and Documents/ primary directories.
        let effective_parent_path = Self::effective_parent_path_for_upload(
            collection,
            &relative_path,
            &parent_path,
            &self.base_relative_path,
        );

        debug!(
            path = %path.display(),
            display_name = %display_name,
            relative_path = %effective_parent_path,
            mime_type = %mime_type,
            collection = %collection.as_str(),
            file_class = ?file_class,
            start_pos,
            "Uploading file"
        );

        if start_pos > 0 {
            return Err(Self::unsupported_command(
                "Resume upload is not supported for Android MediaStore backend",
            ));
        }

        // Acquire upload slot (limit concurrency)
        let _permit = self.limiter.acquire().await;
        
        // Open file descriptor for writing with retry
        let bridge = self.bridge.clone();
        let fd_info = retry_with_backoff(&self.retry_config, "open_fd_for_write", || {
            let bridge = bridge.clone();
            let display_name = display_name.clone();
            let parent_path = effective_parent_path.clone();
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
        
        let resolved_file_path = match self.resolve_file_lookup(path).await? {
            FileLookupResolution::File { candidate_path, .. } => candidate_path,
            FileLookupResolution::Directory { .. } => {
                return Err(Self::file_not_available(format!(
                    "Path is a directory and cannot be deleted as a file: {}",
                    path.display()
                )));
            }
            FileLookupResolution::NotFound => {
                return Err(Self::file_not_available(format!(
                    "File not found: {}",
                    path.display()
                )));
            }
        };
        debug!(path = %resolved_file_path, "Deleting file");

        let bridge = self.bridge.clone();
        retry_with_backoff(&self.retry_config, "delete", || {
            let bridge = bridge.clone();
            let path = resolved_file_path.clone();
            async move { bridge.delete_file(&path).await }
        })
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to delete file");
            Self::file_not_available(e.to_string())
        })?;

        info!(path = %resolved_file_path, "File deleted successfully");
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

        if !self.directory_exists(path).await? {
            return Err(Self::storage_error(
                StorageErrorKind::PermanentDirectoryNotAvailable,
                format!("Directory not found: {}", path.display()),
            ));
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
        // Benign names containing ".." as a substring should be allowed
        assert!(backend.validate_path(Path::new("photo..jpg")).is_ok());
        assert!(backend.validate_path(Path::new("my..backup.tar")).is_ok());
        
        // Null bytes should fail
        assert!(backend.validate_path(Path::new("test\0.jpg")).is_err());
        // ".." as a path component should fail
        assert!(backend.validate_path(Path::new("../secret")).is_err());
        assert!(backend.validate_path(Path::new("foo/../bar")).is_err());
        assert!(backend.validate_path(Path::new("/../../etc/passwd")).is_err());
    }

    #[test]
    fn test_resolve_path() {
        let backend = AndroidMediaStoreBackend::new();

        // Only virtual-root paths are preserved as-is
        assert_eq!(backend.resolve_path(Path::new("DCIM/CameraFTP/test.jpg")), "DCIM/CameraFTP/test.jpg");
        assert_eq!(backend.resolve_path(Path::new("Download/CameraFTP/test.jpg")), "Download/CameraFTP/test.jpg");

        // Non-virtual-root DCIM paths get the base prefix
        assert_eq!(backend.resolve_path(Path::new("DCIM/test.jpg")), "DCIM/CameraFTP/DCIM/test.jpg");

        // Bare filenames get the base prefix
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

    #[test]
    fn test_virtual_directory_candidates_use_configured_primary_root() {
        let backend = AndroidMediaStoreBackend::with_config(
            Arc::new(MockMediaStoreBridge::temp()),
            UploadLimiter::default_limiter(),
            RetryConfig::default(),
            "Pictures/CameraFTP/".to_string(),
        );

        let candidates = backend.virtual_directory_candidates(Path::new("/album"));
        assert_eq!(
            candidates,
            vec![
                "Pictures/CameraFTP/album/".to_string(),
                "Download/CameraFTP/album/".to_string(),
            ]
        );
    }

    #[test]
    fn test_virtual_file_candidates_dedup_when_primary_equals_download_root() {
        let backend = AndroidMediaStoreBackend::with_config(
            Arc::new(MockMediaStoreBridge::temp()),
            UploadLimiter::default_limiter(),
            RetryConfig::default(),
            "Download/CameraFTP/".to_string(),
        );

        let candidates = backend.virtual_file_candidates(Path::new("note.txt"));
        assert_eq!(candidates, vec!["Download/CameraFTP/note.txt".to_string()]);
    }

    #[test]
    fn test_virtual_file_candidates_preserve_explicit_rooted_path() {
        let backend = AndroidMediaStoreBackend::with_config(
            Arc::new(MockMediaStoreBridge::temp()),
            UploadLimiter::default_limiter(),
            RetryConfig::default(),
            "DCIM/CameraFTP/".to_string(),
        );

        let candidates = backend.virtual_file_candidates(Path::new("/DCIM/CameraFTP/file.jpg"));
        assert_eq!(candidates, vec!["DCIM/CameraFTP/file.jpg".to_string()]);
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn test_cwd_root_should_succeed_unconditionally() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let result = backend.cwd(&DefaultUser {}, Path::new("/")).await;
        assert!(result.is_ok(), "cwd / should always succeed");
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn test_cwd_missing_subdirectory_should_fail() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let result = backend.cwd(&DefaultUser {}, Path::new("/subdir")).await;
        assert!(result.is_err(), "cwd subdirectory should fail when virtual directory is missing");
        assert_eq!(
            result.unwrap_err().kind(),
            StorageErrorKind::PermanentDirectoryNotAvailable
        );
    }

    #[test]
    fn test_virtual_file_candidates_do_not_preserve_explicit_non_virtual_dcim_rooted_path() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let candidates = backend.virtual_file_candidates(Path::new("/DCIM/album/file.jpg"));
        assert_eq!(
            candidates,
            vec![
                "DCIM/CameraFTP/DCIM/album/file.jpg".to_string(),
                "Download/CameraFTP/DCIM/album/file.jpg".to_string()
            ]
        );
    }

    #[test]
    fn test_virtual_file_candidates_preserve_explicit_dcim_cameraftp_rooted_path() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let candidates = backend.virtual_file_candidates(Path::new("/DCIM/CameraFTP/file.jpg"));
        assert_eq!(candidates, vec!["DCIM/CameraFTP/file.jpg".to_string()]);
    }

    #[test]
    fn test_virtual_file_candidates_preserve_explicit_download_cameraftp_rooted_path() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let candidates = backend.virtual_file_candidates(Path::new("/Download/CameraFTP/file.jpg"));
        assert_eq!(candidates, vec!["Download/CameraFTP/file.jpg".to_string()]);
    }

    #[test]
    fn test_virtual_file_candidates_do_not_preserve_arbitrary_download_rooted_path() {
        let backend = AndroidMediaStoreBackend::with_bridge(Arc::new(MockMediaStoreBridge::temp()));

        let candidates = backend.virtual_file_candidates(Path::new("/Download/Other/file.jpg"));
        assert_eq!(
            candidates,
            vec![
                "DCIM/CameraFTP/Download/Other/file.jpg".to_string(),
                "Download/CameraFTP/Download/Other/file.jpg".to_string()
            ]
        );
    }

    #[test]
    fn test_effective_parent_path_for_downloads_preserves_explicit_subdirectory() {
        let base = default_relative_path();

        let explicit_relative_path = "Download/CameraFTP/subdir/notes.txt";
        let explicit_parent_path = "Download/CameraFTP/subdir/";
        let effective = AndroidMediaStoreBackend::effective_parent_path_for_upload(
            MediaStoreCollection::Downloads,
            explicit_relative_path,
            explicit_parent_path,
            base,
        );
        assert_eq!(effective, explicit_parent_path);

        let non_explicit_relative_path = "DCIM/CameraFTP/notes.txt";
        let non_explicit_parent_path = "DCIM/CameraFTP/";
        let effective = AndroidMediaStoreBackend::effective_parent_path_for_upload(
            MediaStoreCollection::Downloads,
            non_explicit_relative_path,
            non_explicit_parent_path,
            base,
        );
        assert_eq!(effective, DOWNLOADS_RELATIVE_PATH);

        // Virtual subdir from base-relative path should be preserved
        let subdir_relative_path = "DCIM/CameraFTP/subdir/notes.txt";
        let subdir_parent_path = "DCIM/CameraFTP/subdir/";
        let effective = AndroidMediaStoreBackend::effective_parent_path_for_upload(
            MediaStoreCollection::Downloads,
            subdir_relative_path,
            subdir_parent_path,
            base,
        );
        assert_eq!(effective, "Download/CameraFTP/subdir/");
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
