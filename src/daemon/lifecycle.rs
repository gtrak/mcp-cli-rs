use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

/// Manages daemon lifecycle with idle timeout
#[derive(Clone)]
pub struct DaemonLifecycle {
    /// Last timestamp when activity was detected
    last_activity: Arc<Mutex<Instant>>,
    /// Time after which daemon should shutdown
    idle_timeout: Duration,
    /// Flag indicating daemon should run
    running: Arc<AtomicBool>,
}

impl DaemonLifecycle {
    /// Create new lifecycle manager with custom idle timeout
    pub fn new(idle_timeout_secs: u64) -> Self {
        let last_activity = Arc::new(Mutex::new(Instant::now()));
        let running = Arc::new(AtomicBool::new(true));

        let idle_timeout = Duration::from_secs(idle_timeout_secs);

        Self {
            last_activity,
            idle_timeout,
            running,
        }
    }

    /// Update the last activity timestamp to now
    /// Call this whenever a request is received
    pub fn update_activity(&self) {
        let mut last_activity = self.last_activity.lock().unwrap();
        *last_activity = Instant::now();
    }

    /// Check if the daemon should shutdown due to idle timeout
    /// Call this periodically (e.g., every 1 second) in a separate task
    pub fn should_shutdown(&self) -> bool {
        let last_activity = self.last_activity.lock().unwrap();
        let elapsed = last_activity.elapsed();
        elapsed > self.idle_timeout
    }

    /// Signal that daemon should shut down
    pub fn shutdown(&self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if daemon is running
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Get time until idle timeout (if not yet timed out)
    pub fn time_until_idle(&self) -> Option<Duration> {
        let last_activity = self.last_activity.lock().unwrap();
        let elapsed = last_activity.elapsed();
        if elapsed < self.idle_timeout {
            Some(self.idle_timeout - elapsed)
        } else {
            None
        }
    }

    /// Get elapsed time since last activity
    pub fn elapsed_since_last_activity(&self) -> Duration {
        let last_activity = self.last_activity.lock().unwrap();
        last_activity.elapsed()
    }
}

impl Default for DaemonLifecycle {
    fn default() -> Self {
        Self::new(60) // Default 60 second idle timeout
    }
}

/// Background task that monitors idle timeout and shuts down daemon if needed
pub async fn run_idle_timer(lifecycle: &DaemonLifecycle) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        // Check if we should shut down
        if lifecycle.should_shutdown() {
            tracing::info!("Idle timeout exceeded, shutting down daemon");
            lifecycle.shutdown();
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_new() {
        let lifecycle = DaemonLifecycle::new(30);
        assert!(lifecycle.is_running());
    }

    #[test]
    fn test_update_activity() {
        let lifecycle = DaemonLifecycle::new(30);
        lifecycle.update_activity();
        let elapsed = lifecycle.elapsed_since_last_activity();
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn test_should_shutdown_no_activity() {
        let lifecycle = DaemonLifecycle::new(1);
        std::thread::sleep(Duration::from_secs(2));
        assert!(lifecycle.should_shutdown());
    }

    #[test]
    fn test_should_shutdown_with_activity() {
        let lifecycle = DaemonLifecycle::new(10);
        lifecycle.update_activity();
        std::thread::sleep(Duration::from_millis(500));
        assert!(!lifecycle.should_shutdown());
    }

    #[test]
    fn test_time_until_idle() {
        let lifecycle = DaemonLifecycle::new(30);
        lifecycle.update_activity();
        let time_until = lifecycle.time_until_idle().unwrap();
        assert!(time_until > Duration::from_secs(29));
        assert!(time_until < Duration::from_secs(31));
    }

    #[test]
    fn test_shutdown() {
        let lifecycle = DaemonLifecycle::new(30);
        lifecycle.shutdown();
        assert!(!lifecycle.is_running());
    }

    #[test]
    fn test_default_timeout() {
        let lifecycle = DaemonLifecycle::default();
        // Elapsed time should be very small (just created), not exactly ZERO
        let elapsed = lifecycle.elapsed_since_last_activity();
        assert!(elapsed < Duration::from_millis(100), "Elapsed time should be near zero immediately after creation");
    }

    #[test]
    fn test_custom_ttl_value() {
        let lifecycle = DaemonLifecycle::new(120);
        // Time until idle should be close to 120 seconds (minus tiny elapsed time)
        let time_until = lifecycle.time_until_idle().unwrap();
        assert!(time_until > Duration::from_secs(119), "Time until idle should be close to 120s");
        assert!(time_until <= Duration::from_secs(120), "Time until idle should not exceed 120s");
    }

    #[test]
    fn test_ttl_not_exceeded_immediately() {
        let lifecycle = DaemonLifecycle::new(60);
        // Should not shutdown immediately
        assert!(!lifecycle.should_shutdown());
    }

    #[test]
    fn test_ttl_after_delay() {
        let lifecycle = DaemonLifecycle::new(10);
        lifecycle.update_activity();
        std::thread::sleep(Duration::from_millis(200));
        // Should not shutdown
        assert!(!lifecycle.should_shutdown());
    }

    #[test]
    fn test_elapsed_since_last_activity() {
        let lifecycle = DaemonLifecycle::new(30);
        lifecycle.update_activity();
        // Immediately after update, elapsed should be very small
        let elapsed = lifecycle.elapsed_since_last_activity();
        assert!(elapsed < Duration::from_millis(100), "Elapsed should be small immediately after activity update");
        
        // After sleeping, elapsed should be measurable
        std::thread::sleep(Duration::from_millis(50));
        let elapsed = lifecycle.elapsed_since_last_activity();
        assert!(elapsed >= Duration::from_millis(50), "Elapsed should account for sleep time");
    }

    #[test]
    fn test_time_until_idle_with_activity() {
        let lifecycle = DaemonLifecycle::new(60);
        lifecycle.update_activity();
        let time_until = lifecycle.time_until_idle().unwrap();
        // Should be close to 60 seconds minus tiny elapsed time
        assert!(time_until > Duration::from_secs(59), "Time until idle should be close to 60s");
        assert!(time_until <= Duration::from_secs(60), "Time until idle should not exceed 60s");
    }

    #[test]
    fn test_time_until_idle_without_activity() {
        let lifecycle = DaemonLifecycle::new(2); // Use short 2 second timeout
        std::thread::sleep(Duration::from_secs(3)); // Sleep longer than timeout
        // After sleeping past the timeout, should return None
        assert!(lifecycle.time_until_idle().is_none(), "Should return None when past idle timeout");
    }
}
