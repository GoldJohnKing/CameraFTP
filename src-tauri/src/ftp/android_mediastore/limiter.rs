// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Upload concurrency limiter for MediaStore operations.
//!
//! Android MediaStore has limited capacity for concurrent file operations.
//! This module provides a semaphore-based limiter to control concurrency.

use std::sync::Arc;
use tokio::sync::{Semaphore, SemaphorePermit};

/// Default maximum concurrent uploads.
pub const DEFAULT_MAX_CONCURRENT_UPLOADS: usize = 4;

/// Upload limiter using a semaphore to control concurrency.
#[derive(Debug, Clone)]
pub struct UploadLimiter {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl UploadLimiter {
    /// Creates a new upload limiter with the specified maximum concurrent uploads.
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    /// Creates a new upload limiter with the default maximum concurrent uploads.
    pub fn default_limiter() -> Self {
        Self::new(DEFAULT_MAX_CONCURRENT_UPLOADS)
    }

    /// Acquires a permit to perform an upload operation.
    ///
    /// The permit is automatically released when dropped.
    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        self.semaphore.acquire().await.expect("semaphore is closed")
    }

    /// Returns the maximum number of concurrent uploads allowed.
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Returns the number of currently available permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// RAII guard for an acquired upload permit.
///
/// When this guard is dropped, the permit is automatically released.
#[derive(Debug)]
pub struct UploadGuard<'a> {
    _permit: SemaphorePermit<'a>,
}

impl<'a> UploadGuard<'a> {
    /// Creates a new upload guard from an acquired permit.
    pub fn new(permit: SemaphorePermit<'a>) -> Self {
        Self { _permit: permit }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_upload_limiter_creation() {
        let limiter = UploadLimiter::new(4);
        assert_eq!(limiter.max_concurrent(), 4);
        assert_eq!(limiter.available_permits(), 4);
    }

    #[test]
    fn test_default_limiter() {
        let limiter = UploadLimiter::default_limiter();
        assert_eq!(limiter.max_concurrent(), DEFAULT_MAX_CONCURRENT_UPLOADS);
    }

    #[tokio::test]
    async fn test_acquire_releases_permit() {
        let limiter = UploadLimiter::new(2);
        
        // Initially 2 permits available
        assert_eq!(limiter.available_permits(), 2);
        
        // Acquire one
        let permit1 = limiter.acquire().await;
        assert_eq!(limiter.available_permits(), 1);
        
        // Acquire another
        let permit2 = limiter.acquire().await;
        assert_eq!(limiter.available_permits(), 0);
        
        // Release one
        drop(permit1);
        assert_eq!(limiter.available_permits(), 1);
        
        // Release the other
        drop(permit2);
        assert_eq!(limiter.available_permits(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_uploads_limited() {
        let limiter = Arc::new(UploadLimiter::new(2));
        let limiter_clone = limiter.clone();
        
        // Spawn tasks that hold permits
        let handle1 = tokio::spawn(async move {
            let _permit = limiter_clone.acquire().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            // Permit released here
        });
        
        let limiter_clone = limiter.clone();
        let handle2 = tokio::spawn(async move {
            let _permit = limiter_clone.acquire().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
        
        // Wait a bit for tasks to acquire permits
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Both permits should be in use
        assert_eq!(limiter.available_permits(), 0);
        
        // Wait for tasks to complete
        handle1.await.unwrap();
        handle2.await.unwrap();
        
        // Permits should be available again
        assert_eq!(limiter.available_permits(), 2);
    }
}
