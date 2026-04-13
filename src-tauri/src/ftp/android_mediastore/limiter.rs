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
}

impl UploadLimiter {
    /// Creates a new upload limiter with the specified maximum concurrent uploads.
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
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

    /// Returns the number of currently available permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
        
        // Wait for spawned tasks to acquire permits
        for _ in 0..100 {
            if limiter.available_permits() == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        
        // Both permits should be in use
        assert_eq!(limiter.available_permits(), 0);
        
        // Wait for tasks to complete
        handle1.await.unwrap();
        handle2.await.unwrap();
        
        // Permits should be available again
        assert_eq!(limiter.available_permits(), 2);
    }
}
