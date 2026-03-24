// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android MediaStore storage backend for libunftp.
//!
//! This module provides a storage backend that writes FTP uploads directly
//! to Android's MediaStore, making them immediately visible in the device's
//! gallery application.
//!
//! # Features
//!
//! - Direct integration with Android MediaStore API via JNI
//! - Retry logic with exponential backoff for transient failures
//! - Concurrency limiting (max 4 concurrent uploads by default)
//! - Automatic MIME type detection based on file extension
//! - Support for common image formats (JPEG, PNG, HEIF, RAW)
//!
//! # Architecture
//!
//! The backend consists of several components:
//!
//! - [`backend`]: The main `StorageBackend` implementation for libunftp
//! - [`bridge`]: JNI bridge to Android MediaStore API
//! - [`types`]: Data types and DTOs for MediaStore operations
//! - [`retry`]: Retry logic with exponential backoff
//! - [`limiter`]: Upload concurrency limiter
//!
//! # Example
//!
//! ```ignore
//! use libunftp::ServerBuilder;
//! use camera_ftp_companion_lib::ftp::android_mediastore::AndroidMediaStoreBackend;
//!
//! let backend = AndroidMediaStoreBackend::new();
//! let server = ServerBuilder::new(Box::new(move || backend.clone()))
//!     .greeting("Welcome to CameraFTP")
//!     .build()
//!     .unwrap();
//! ```

pub mod backend;
pub mod bridge;
pub mod limiter;
pub mod retry;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export main types for convenience
pub use backend::{AndroidMediaStoreBackend, MediaStoreMetadata};
pub use bridge::{create_bridge, PlatformBridge};
pub use limiter::UploadLimiter;
pub use retry::{retry_with_backoff, RetryConfig};
pub use types::{
    default_relative_path, display_name_from_path, mime_type_from_filename,
    relative_path_from_full_path, FileDescriptorInfo, InsertResult, MediaStoreBridgeClient,
    MediaStoreError, QueryResult,
};
