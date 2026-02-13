//! Integration tests for retry logic with exponential backoff (TEST-06, TEST-07, TEST-08)
//!
//! These tests verify:
//! - TEST-06: Exponential backoff timing with measurable delays
//! - TEST-07: Max retry limit enforcement
//! - TEST-08: Delay increases exponentially between retries

mod fixtures;

use mcp_cli_rs::error::McpError;
use mcp_cli_rs::retry::{RetryConfig, retry_with_backoff};
use fixtures::spawn_failing_server;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use futures_util::FutureExt;

/// TEST-06: Verify exponential backoff produces measurable delays
///
/// This test creates a server that fails the first 2 requests, then succeeds.
/// We measure the total time to verify exponential backoff introduces delays.
#[tokio::test]
async fn test_exponential_backoff() {
    // Configure retry with small delays for faster tests
    let config = RetryConfig {
        max_attempts: 3,
        base_delay_ms: 50,
        max_delay_ms: 500,
    };

    // Create server that fails first 2 requests
    let (server, url) = spawn_failing_server(2).await;

    // Track attempt count
    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = attempt_count.clone();
    let url_clone = url.clone();

    // Operation that will be retried
    let operation = move || {
        let attempt_count = attempt_count_clone.clone();
        let url = url_clone.clone();
        Box::new(async move {
            attempt_count.fetch_add(1, Ordering::SeqCst);
            
            // Simulate HTTP request that might fail
            let client = reqwest::Client::new();
            let response = client
                .post(&url)
                .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
                .send()
                .await
                .map_err(|e| McpError::ConnectionError {
                    server: url.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                })?;

            // Check response status
            let status = response.status();
            if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                return Err(McpError::ConnectionError {
                    server: url.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "Service temporarily unavailable",
                    ),
                });
            }

            // Parse response
            let body: serde_json::Value = response.json().await.map_err(|e| McpError::ConnectionError {
                server: url.clone(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            })?;

            if body.get("error").is_some() {
                return Err(McpError::ConnectionError {
                    server: url.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Server returned error",
                    ),
                });
            }

            Ok::<String, McpError>("success".to_string())
        }).boxed()
    };

    // Execute with retry and measure timing
    let start_time = Instant::now();
    let result = retry_with_backoff(operation, &config).await;
    let total_duration = start_time.elapsed();

    // Cleanup
    server.shutdown().await;

    // Assertions
    assert!(result.is_ok(), "Operation should succeed after retries");
    
    // We should have made exactly 3 attempts (2 failures + 1 success)
    let attempts = attempt_count.load(Ordering::SeqCst);
    assert_eq!(attempts, 3, "Should make exactly 3 attempts (2 failures + 1 success)");

    // Verify exponential backoff delays
    // First retry delay: ~50ms (base_delay_ms)
    // Second retry delay: ~100ms (base_delay_ms * 2)
    // Total should be at least ~150ms of delay time
    println!("Total duration: {:?}", total_duration);
    assert!(
        total_duration.as_millis() >= 100,
        "Total duration should include backoff delays (~150ms minimum), got {:?}",
        total_duration
    );

    // Verify operation succeeded
    assert_eq!(result.unwrap(), "success", "Operation should return success");
}

/// TEST-07: Verify max retry limit is enforced
///
/// This test creates a server that always fails, and verifies we stop
/// after the configured max_attempts.
#[tokio::test]
async fn test_max_retry_limit() {
    // Configure retry with low attempts
    let config = RetryConfig {
        max_attempts: 2,
        base_delay_ms: 10,
        max_delay_ms: 100,
    };

    // Create server that always fails (fails first 5 requests)
    let (server, url) = spawn_failing_server(5).await;

    // Track attempts
    let attempt_count = Arc::new(AtomicUsize::new(0));
    let attempt_count_clone = attempt_count.clone();
    let url_clone = url.clone();

    // Operation that will fail
    let operation = move || {
        let attempt_count = attempt_count_clone.clone();
        let url = url_clone.clone();
        Box::new(async move {
            attempt_count.fetch_add(1, Ordering::SeqCst);
            
            // Simulate HTTP request that will fail
            let client = reqwest::Client::new();
            let _response = client
                .post(&url)
                .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
                .send()
                .await;

            // Always fail with connection error
            Err::<String, McpError>(McpError::ConnectionError {
                server: url.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "Service temporarily unavailable",
                ),
            })
        }).boxed()
    };

    // Execute with retry - should fail after max_attempts
    let result = retry_with_backoff(operation, &config).await;

    // Cleanup
    server.shutdown().await;

    // Assertions
    assert!(result.is_err(), "Operation should fail after max retries");

    // Verify error type is MaxRetriesExceeded
    match result {
        Err(McpError::MaxRetriesExceeded { attempts }) => {
            assert_eq!(attempts, 2, "Should report 2 attempts (max_attempts)");
        }
        Err(other) => {
            panic!("Expected MaxRetriesExceeded error, got: {:?}", other);
        }
        Ok(_) => {
            panic!("Expected error, but operation succeeded");
        }
    }

    // Verify exactly 2 attempts were made
    let attempts = attempt_count.load(Ordering::SeqCst);
    assert_eq!(attempts, 2, "Should make exactly 2 attempts before giving up");
}

/// TEST-08: Verify retry delays increase exponentially
///
/// This test measures actual delays between attempts and verifies they
/// follow exponential growth pattern and respect max_delay_ms cap.
#[tokio::test]
async fn test_retry_delay_increases() {
    // Configure retry with larger base delay for clearer measurement
    let config = RetryConfig {
        max_attempts: 4,
        base_delay_ms: 100,
        max_delay_ms: 1000,
    };

    // Create server that fails first 3 requests
    let (server, url) = spawn_failing_server(3).await;

    // Track timestamps for each attempt using thread-safe container
    let timestamps: Arc<tokio::sync::Mutex<Vec<Instant>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let timestamps_clone = timestamps.clone();
    let url_clone = url.clone();

    // Operation that will be retried
    let operation = move || {
        let timestamps = timestamps_clone.clone();
        let url = url_clone.clone();
        Box::new(async move {
            timestamps.lock().await.push(Instant::now());
            
            // Simulate HTTP request
            let client = reqwest::Client::new();
            let response = client
                .post(&url)
                .body(r#"{"jsonrpc":"2.0","method":"ping","id":1}"#)
                .send()
                .await
                .map_err(|e| McpError::ConnectionError {
                    server: url.clone(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                })?;

            let status = response.status();
            if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
                return Err(McpError::ConnectionError {
                    server: url.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "Service temporarily unavailable",
                    ),
                });
            }

            Ok::<String, McpError>("success".to_string())
        }).boxed()
    };

    // Execute with retry
    let result = retry_with_backoff(operation, &config).await;

    // Cleanup
    server.shutdown().await;

    // Assertions
    assert!(result.is_ok(), "Operation should succeed after retries");

    // Get timestamps
    let timestamps_guard = timestamps.lock().await;
    
    // We should have 4 timestamps (initial + 3 retries before success)
    assert_eq!(timestamps_guard.len(), 4, "Should have 4 timestamps");

    // Calculate delays between attempts
    let delays: Vec<_> = (1..timestamps_guard.len())
        .map(|i| timestamps_guard[i].duration_since(timestamps_guard[i - 1]))
        .collect();

    println!("Base delay: {}ms", config.base_delay_ms);
    println!("Max delay: {}ms", config.max_delay_ms);
    for (i, delay) in delays.iter().enumerate() {
        println!("Delay {}: {:?}", i + 1, delay);
    }

    // Verify we have at least 3 delays
    assert_eq!(delays.len(), 3, "Should have 3 delays between 4 attempts");

    // Verify each subsequent delay is longer than the previous (exponential growth)
    for i in 1..delays.len() {
        assert!(
            delays[i] > delays[i - 1],
            "Delay {} should be longer than delay {} (exponential backoff)",
            i + 1,
            i
        );
    }

    // Verify delays don't exceed max_delay_ms cap
    for (i, delay) in delays.iter().enumerate() {
        let delay_ms = delay.as_millis() as u64;
        // Allow for some variance (delays include operation time)
        assert!(
            delay_ms <= config.max_delay_ms + 500,
            "Delay {} should not exceed max_delay_ms ({}ms + margin)",
            i + 1,
            config.max_delay_ms
        );
    }

    // Verify the pattern: first delay ~base_delay_ms, second ~2*base_delay_ms, etc.
    // Due to jitter and operation time, we use ranges
    let first_delay_ms = delays[0].as_millis() as u64;
    let second_delay_ms = delays[1].as_millis() as u64;
    let third_delay_ms = delays[2].as_millis() as u64;

    // First delay should be around base_delay_ms (100ms) plus operation time
    assert!(
        first_delay_ms >= 80 && first_delay_ms <= 300,
        "First delay should be in range [80ms, 300ms] (base: {}ms)",
        config.base_delay_ms
    );

    // Second delay should be around 2*base_delay_ms (200ms) plus operation time
    assert!(
        second_delay_ms >= first_delay_ms + 50 && second_delay_ms <= 500,
        "Second delay should be in range [first+50ms, 500ms]"
    );

    // Third delay should be around 4*base_delay_ms (400ms) plus operation time
    assert!(
        third_delay_ms >= second_delay_ms + 50 && third_delay_ms <= 1000,
        "Third delay should be in range [second+50ms, 1000ms]"
    );

    println!("All delays verified for exponential backoff pattern!");
}
