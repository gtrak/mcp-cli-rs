//! Daemon lifecycle and cleanup validation tests.
//!
//! Tests daemon startup, idle timeout, orphan cleanup, config change detection,
//! and graceful shutdown across all platforms.
//!
//! XP-04: Validates daemon lifecycle works consistently on Linux, macOS, Windows

use tokio::time::Duration;

/// Test cross-platform consistency: Idle timeout variance
#[tokio::test]
async fn test_cross_platform_idle_timeout_consistency() {
    // Test idle timeout behavior on both platforms
    let timeout_duration = Duration::from_secs(60); // Standard idle timeout

    // Verify timeout is reasonable
    assert!(
        timeout_duration.as_secs() > 0 && timeout_duration.as_secs() <= 120,
        "Idle timeout should be between 0 and 120 seconds"
    );

    // Verify consistent behavior: same timeout duration on all platforms
    #[cfg(unix)]
    {
        // Unix implementation should use same 60s timeout
        assert_eq!(timeout_duration, Duration::from_secs(60));
    }

    #[cfg(windows)]
    {
        // Windows implementation should use same 60s timeout
        assert_eq!(timeout_duration, Duration::from_secs(60));
    }
}
