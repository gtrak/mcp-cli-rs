use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Manages daemon lifecycle with idle timeout
#[derive(Clone)]
pub struct DaemonLifecycle {
    /// Last timestamp when activity was detected
    last_activity: Arc<Mutex<Instant>>,
    /// Time after which daemon should shutdown
    idle_timeout: Duration,
    shutting_down: bool,
}

impl DaemonLifecycle {
    /// Create new lifecycle manager with custom idle timeout
    pub fn new(idle_timeout_secs: u64) -> Self {
        let last_activity = Arc::new(Mutex::new(Instant::now()));

        let idle_timeout = Duration::from_secs(idle_timeout_secs);

        Self {
            last_activity,
            idle_timeout,
            shutting_down: false,
        }
    }

    /// Update the last activity timestamp to now
    /// Call this whenever a request is received
    pub async fn update_activity(&self) {
        let mut last_activity = self.last_activity.lock().await;
        *last_activity = Instant::now();
    }

    /// Check if the daemon should shutdown due to idle timeout
    /// Call this periodically (e.g., every 1 second) in a separate task
    pub async fn should_shutdown(&self) -> bool {
        if self.shutting_down {
            return true;
        }
        let last_activity = self.last_activity.lock().await;
        let elapsed = last_activity.elapsed();
        elapsed > self.idle_timeout
    }

    /// Signal that daemon should shut down
    pub fn shutdown(&mut self) {
        self.shutting_down = true;
    }

    /// Get time until idle timeout (if not yet timed out)
    pub async fn time_until_idle(&self) -> Option<Duration> {
        let last_activity = self.last_activity.lock().await;
        let elapsed = last_activity.elapsed();
        if elapsed < self.idle_timeout {
            Some(self.idle_timeout - elapsed)
        } else {
            None
        }
    }

    /// Get elapsed time since last activity
    pub async fn elapsed_since_last_activity(&self) -> Duration {
        let last_activity = self.last_activity.lock().await;
        last_activity.elapsed()
    }
}

impl Default for DaemonLifecycle {
    fn default() -> Self {
        Self::new(60) // Default 60 second idle timeout
    }
}

/// Background task that monitors idle timeout and shuts down daemon if needed
pub async fn run_idle_timer(lifecycle: Arc<Mutex<DaemonLifecycle>>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        // Check if we should shut down
        if lifecycle.lock().await.should_shutdown().await {
            tracing::info!("Idle timeout exceeded, shutting down daemon");
            lifecycle.blocking_lock().shutdown();
            break;
        }
    }
}
