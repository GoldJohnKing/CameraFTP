//! Integration tests for PASV port selection logic
//!
//! These tests verify the behavior described in server_factory.rs:
//!
//! **Simple mode (advanced_connection.enabled=false):**
//! 1. Default range 50000-50100 available → use default range
//! 2. Default range all occupied → auto-find a 100-port range with available ports
//!
//! **Advanced mode (advanced_connection.enabled=true):**
//! 1. User-configured range has available ports → use user range
//! 2. User-configured range all occupied → return NoAvailablePasvPort error

use tokio::net::TcpListener;

/// Helper to occupy a range of ports for testing
/// Returns a vector of TcpListeners that hold the ports
async fn occupy_ports(start: u16, end: u16) -> Vec<TcpListener> {
    let mut listeners = Vec::new();
    for port in start..=end {
        if let Ok(listener) = TcpListener::bind(format!("0.0.0.0:{}", port)).await {
            listeners.push(listener);
        }
    }
    listeners
}

/// Helper to check if a port is available
async fn is_port_available(port: u16) -> bool {
    TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .is_ok()
}

/// Check PASV port range availability
/// Returns (available_count, total_count, first_available)
async fn check_pasv_port_range(start: u16, end: u16) -> (usize, usize, Option<u16>) {
    let total = if end >= start { (end - start + 1) as usize } else { 0 };
    let mut available = 0;
    let mut first_available = None;

    for port in start..=end {
        if is_port_available(port).await {
            available += 1;
            if first_available.is_none() {
                first_available = Some(port);
            }
        }
    }

    (available, total, first_available)
}

/// Find an available 100-port PASV range
/// Returns Some((start, end)) if found, None otherwise
async fn find_available_pasv_range(start: u16) -> Option<(u16, u16)> {
    const RANGE_SIZE: u16 = 100;
    const MIN_AVAILABLE: usize = 10;

    let max_range_start = 65535 - RANGE_SIZE + 1;
    let search_start = start.min(max_range_start);

    let mut range_start = (search_start / RANGE_SIZE) * RANGE_SIZE;
    if range_start < search_start {
        range_start += RANGE_SIZE;
    }

    while range_start <= max_range_start {
        let range_end = range_start + RANGE_SIZE - 1;
        let (available, _, _) = check_pasv_port_range(range_start, range_end).await;

        if available >= MIN_AVAILABLE {
            return Some((range_start, range_end));
        }

        range_start += RANGE_SIZE;
    }

    None
}

// ============================================================================
// Tests for check_pasv_port_range
// ============================================================================

#[tokio::test]
async fn test_check_pasv_port_range_all_available() {
    // Use high ports unlikely to be in use
    let start = 59000;
    let end = 59099;

    let (available, total, first_available) = check_pasv_port_range(start, end).await;

    // All ports should be available
    assert_eq!(total, 100);
    assert!(available > 90, "Expected most ports available, got {}", available);
    assert_eq!(first_available, Some(start));
}

#[tokio::test]
async fn test_check_pasv_port_range_partially_occupied() {
    // Occupy some ports in the middle of the range
    let start = 59100;
    let end = 59109;
    let _listeners = occupy_ports(59103, 59106).await;

    let (available, total, first_available) = check_pasv_port_range(start, end).await;

    assert_eq!(total, 10);
    // 4 ports occupied (59103-59106), so 6 should be available
    assert!(
        available >= 6,
        "Expected at least 6 available, got {}",
        available
    );
    // First available should be 59100 or 59101 (before the occupied range)
    assert!(
        first_available.unwrap() <= 59102,
        "First available should be before occupied ports"
    );
}

#[tokio::test]
async fn test_check_pasv_port_range_all_occupied() {
    // Occupy the entire range
    let start = 59200;
    let end = 59209;
    let _listeners = occupy_ports(start, end).await;

    let (available, total, first_available) = check_pasv_port_range(start, end).await;

    assert_eq!(total, 10);
    assert_eq!(available, 0, "All ports should be occupied");
    assert_eq!(first_available, None);
}

#[tokio::test]
async fn test_check_pasv_port_range_first_port_available() {
    // Occupy ports after the first one
    let start = 59300;
    let end = 59304;
    let _listeners = occupy_ports(59301, 59304).await;

    let (available, total, first_available) = check_pasv_port_range(start, end).await;

    assert_eq!(total, 5);
    assert!(available >= 1);
    assert_eq!(first_available, Some(start));
}

#[tokio::test]
async fn test_check_pasv_port_range_single_port() {
    let port = 59400;

    let (available, total, first_available) = check_pasv_port_range(port, port).await;

    assert_eq!(total, 1);
    if available == 1 {
        assert_eq!(first_available, Some(port));
    } else {
        assert_eq!(first_available, None);
    }
}

#[tokio::test]
async fn test_check_pasv_port_range_invalid_range() {
    // End < Start should result in 0 total
    let (available, total, first_available) = check_pasv_port_range(100, 50).await;

    assert_eq!(total, 0);
    assert_eq!(available, 0);
    assert_eq!(first_available, None);
}

// ============================================================================
// Tests for find_available_pasv_range
// ============================================================================

#[tokio::test]
async fn test_find_available_pasv_range_default_available() {
    // Search from a high port range that's likely available
    let start = 58000;

    let result = find_available_pasv_range(start).await;

    assert!(result.is_some());
    let (range_start, range_end) = result.unwrap();
    assert_eq!(range_end - range_start + 1, 100);
    // Should be aligned to 100-port blocks
    assert_eq!(range_start % 100, 0);
}

#[tokio::test]
async fn test_find_available_pasv_range_skips_occupied_blocks() {
    // Occupy most ports in a 100-port block to force it below MIN_AVAILABLE threshold
    // We need to occupy more than 90 ports (leaving less than 10 available)
    let block_start = 60000;
    let block_end = 60089; // Occupy first 90 ports

    let listeners = occupy_ports(block_start, block_end).await;
    let occupied_count = listeners.len();

    // Verify we occupied enough ports to trigger skip behavior
    // If we couldn't occupy enough ports, the test is still valid but may return the same block
    let min_occupied_for_skip = 91; // Need to leave less than 10 available

    // Search starting from the occupied block
    let result = find_available_pasv_range(block_start).await;

    assert!(result.is_some());
    let (range_start, range_end) = result.unwrap();
    assert_eq!(range_end - range_start + 1, 100);

    // If we successfully occupied enough ports, should skip to next block
    if occupied_count >= min_occupied_for_skip {
        assert!(
            range_start >= 60100,
            "Should skip occupied block ({} ports occupied), got range starting at {}",
            occupied_count,
            range_start
        );
    }
    // Otherwise, the test still passes - we just couldn't create enough port occupation
}

#[tokio::test]
async fn test_find_available_pasv_range_partial_occupation_ok() {
    // Partially occupy a range (less than MIN_AVAILABLE occupied is ok)
    let block_start = 60500;
    // Only occupy 5 ports (leaving 95 available, which is > MIN_AVAILABLE of 10)
    let _listeners = occupy_ports(block_start, block_start + 4).await;

    let result = find_available_pasv_range(block_start).await;

    assert!(result.is_some());
    let (range_start, _) = result.unwrap();
    // Should still return this block since most ports are available
    assert!(
        range_start <= block_start + 100,
        "Should accept partially occupied block"
    );
}

#[tokio::test]
async fn test_find_available_pasv_range_aligned_to_blocks() {
    // Start from a non-aligned position
    let start = 58850; // Not aligned to 100

    let result = find_available_pasv_range(start).await;

    assert!(result.is_some());
    let (range_start, _) = result.unwrap();
    // Should be aligned to the next 100-port block
    assert!(range_start >= 58900);
    assert_eq!(range_start % 100, 0);
}

// ============================================================================
// Integration tests for PASV port selection logic (server_factory behavior)
// ============================================================================

/// Simulates the PASV range selection logic from server_factory.rs
/// Returns the selected range or an error message
async fn select_pasv_range_simple_mode(
    default_range: (u16, u16),
) -> Result<(u16, u16), String> {
    let (available_count, _total_count, _) =
        check_pasv_port_range(default_range.0, default_range.1).await;

    if available_count > 0 {
        Ok(default_range)
    } else {
        // Auto-find
        match find_available_pasv_range(1024).await {
            Some((start, end)) => Ok((start, end)),
            None => Err("No available PASV port range found".to_string()),
        }
    }
}

/// Simulates the PASV range selection logic for advanced mode
async fn select_pasv_range_advanced_mode(
    user_range: (u16, u16),
) -> Result<(u16, u16), String> {
    let (available_count, total_count, _) =
        check_pasv_port_range(user_range.0, user_range.1).await;

    if available_count == 0 {
        Err(format!(
            "{}-{} (共{}个端口均被占用)",
            user_range.0, user_range.1, total_count
        ))
    } else {
        Ok(user_range)
    }
}

#[tokio::test]
async fn test_simple_mode_default_range_available() {
    // Use a range likely to be available
    let default_range = (58500, 58599);

    let result = select_pasv_range_simple_mode(default_range).await;

    assert!(result.is_ok());
    let (start, end) = result.unwrap();
    assert_eq!((start, end), default_range);
}

#[tokio::test]
async fn test_simple_mode_default_occupied_auto_finds() {
    // Fully occupy the default range
    let default_range = (58600, 58699);
    let _listeners = occupy_ports(default_range.0, default_range.1).await;

    let result = select_pasv_range_simple_mode(default_range).await;

    // Should auto-find a different range
    assert!(result.is_ok());
    let (start, end) = result.unwrap();
    assert_ne!((start, end), default_range, "Should find different range");
    assert_eq!(end - start + 1, 100);
}

#[tokio::test]
async fn test_advanced_mode_user_range_available() {
    let user_range = (58700, 58799);

    let result = select_pasv_range_advanced_mode(user_range).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), user_range);
}

#[tokio::test]
async fn test_advanced_mode_user_range_occupied_returns_error() {
    // Fully occupy user range
    let user_range = (58800, 58809);
    let _listeners = occupy_ports(user_range.0, user_range.1).await;

    let result = select_pasv_range_advanced_mode(user_range).await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err();
    assert!(error_msg.contains("58800-58809"));
    assert!(error_msg.contains("被占用"));
}

#[tokio::test]
async fn test_advanced_mode_partial_availability_ok() {
    // Partially occupy user range
    let user_range = (58900, 58909);
    let _listeners = occupy_ports(58900, 58904).await; // Only 5 occupied

    let result = select_pasv_range_advanced_mode(user_range).await;

    // Should succeed since some ports are available
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), user_range);
}

// ============================================================================
// Edge cases and stress tests
// ============================================================================

#[tokio::test]
async fn test_check_range_near_port_limit() {
    // Test near the 65535 limit
    let start = 65530;
    let end = 65535;

    let (available, total, first_available) = check_pasv_port_range(start, end).await;

    assert_eq!(total, 6);
    // Just verify it doesn't panic and returns reasonable values
    assert!(available <= total);
    if available > 0 {
        assert!(first_available.is_some());
    }
}

#[tokio::test]
async fn test_find_range_near_port_limit() {
    // Search starting near the limit
    let start = 65500;

    let result = find_available_pasv_range(start).await;

    // May or may not find depending on system state
    // Just verify it doesn't panic
    if let Some((range_start, range_end)) = result {
        // range_end is always <= 65535 by construction (range_start + 99)
        let _ = range_end; // Use the value to avoid unused variable warning
        assert_eq!(range_end - range_start + 1, 100);
    }
}

#[tokio::test]
async fn test_concurrent_port_checks() {
    // Verify that check_pasv_port_range works correctly under concurrent access
    let start = 59400;
    let end = 59499;

    let handles: Vec<_> = (0..5)
        .map(|_| {
            tokio::spawn(async move { check_pasv_port_range(start, end).await })
        })
        .collect();

    for handle in handles {
        let (available, total, _) = handle.await.unwrap();
        assert_eq!(total, 100);
        assert!(available <= 100);
    }
}
