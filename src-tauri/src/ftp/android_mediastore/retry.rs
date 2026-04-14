// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Retry logic with exponential backoff for MediaStore operations.
//!
//! Android MediaStore operations can occasionally fail due to transient
//! conditions (e.g., storage temporarily unavailable). This module provides
//! retry logic with exponential backoff to handle such cases.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Default maximum number of retry attempts.
pub const DEFAULT_MAX_RETRIES: usize = 3;

/// Default initial delay between retries (100ms).
pub const DEFAULT_INITIAL_DELAY_MS: u64 = 100;

/// Default maximum delay between retries (5 seconds).
pub const DEFAULT_MAX_DELAY_MS: u64 = 5000;

/// Default backoff multiplier (2.0).
pub const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: usize,
    /// Initial delay between retries.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Backoff multiplier for exponential backoff.
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_delay: Duration::from_millis(DEFAULT_INITIAL_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
        }
    }
}

impl RetryConfig {
    /// Creates a new retry configuration with custom values.
    pub fn new(max_retries: usize, initial_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay: Duration::from_millis(initial_delay_ms),
            max_delay: Duration::from_millis(max_delay_ms),
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
        }
    }

    /// Creates a fast retry configuration for testing.
    pub fn fast() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        }
    }
}

/// Executes an async operation with exponential backoff retry.
///
/// # Arguments
/// * `config` - Retry configuration
/// * `operation` - Name of the operation (for logging)
/// * `f` - Async function to execute
///
/// # Returns
/// The result of the operation on success, or the last error after all retries exhausted.
///
/// # Example
/// ```ignore
/// use crate::ftp::android_mediastore::retry::{retry_with_backoff, RetryConfig};
///
/// let config = RetryConfig::default();
/// let result = retry_with_backoff(&config, "open_fd", || async {
///     bridge.open_fd_for_read(path).await
/// }).await?;
/// ```
pub async fn retry_with_backoff<T, E, F, Fut>(
    config: &RetryConfig,
    operation: &str,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut delay = config.initial_delay;
    let mut attempt = 0;

    loop {
        attempt += 1;
        
        match f().await {
            Ok(result) => {
                if attempt > 1 {
                    debug!(operation, attempt, "Retry succeeded");
                }
                return Ok(result);
            }
            Err(e) => {
                if attempt > config.max_retries {
                    warn!(
                        operation,
                        attempt,
                        max_retries = config.max_retries,
                        error = ?e,
                        "All retry attempts exhausted"
                    );
                    return Err(e);
                }

                warn!(
                    operation,
                    attempt,
                    delay_ms = delay.as_millis(),
                    error = ?e,
                    "Operation failed, retrying"
                );

                sleep(delay).await;

                // Calculate next delay with exponential backoff
                delay = std::cmp::min(
                    Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_multiplier),
                    config.max_delay,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_succeeds_on_first_attempt() {
        let config = RetryConfig::fast();
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = retry_with_backoff(&config, "test", || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok::<_, String>(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig::fast();
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result = retry_with_backoff(&config, "test", || {
            let count = count_clone.clone();
            async move {
                let calls = count.fetch_add(1, Ordering::SeqCst);
                if calls < 2 {
                    Err("temporary failure".to_string())
                } else {
                    Ok::<_, String>(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        };
        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let result: Result<(), String> = retry_with_backoff(&config, "test", || {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>("permanent failure".to_string())
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
    }


}
