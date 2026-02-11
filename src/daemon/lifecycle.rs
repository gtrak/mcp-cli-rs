use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, RwLock};

/// Manages daemon lifecycle with idle timeout
#[derive(Clone)]
pub struct DaemonLifecycle {
    /// Last timestamp when activity was detected
    pub last_activity: Arc<Mutex<Instant>>,
    /// Time after which daemon should shutdown
    pub idle_timeout: Duration,
    pub shutting_down: Arc<RwLock<bool>>,
}

impl DaemonLifecycle {
    /// Create new lifecycle manager with custom idle timeout
    pub fn new(idle_timeout_secs: u64) -> Self {
        let last_activity = Arc::new(Mutex::new(Instant::now()));

        let idle_timeout = Duration::from_secs(idle_timeout_secs);

        Self {
            last_activity,
            idle_timeout,
            shutting_down: Arc::new(RwLock::new(false)),
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
        if *self.shutting_down.read().await {
            return true;
        }
        let last_activity = self.last_activity.lock().await;
        let elapsed = last_activity.elapsed();
        elapsed > self.idle_timeout
    }

    /// Signal that daemon should shut down
    pub async fn shutdown(&self) {
        *self.shutting_down.write().await = true;
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

    /// Get the current idle timeout value
    pub fn get_idle_timeout(&self) -> Duration {
        self.idle_timeout.clone()
    }

    /// Check if the daemon is shutting down
    pub async fn is_shutting_down(&self) -> bool {
        *self.shutting_down.read().await
    }

    /// Get the current config hash
    pub fn get_config_hash(&self) -> Option<String> {
        Some(String::from(""))
    }

    /// Set the current config hash
    pub fn set_config_hash(&mut self, hash: &str) {
        // Implementation placeholder - currently not used
        let _ = hash;
    }

    /// Get the current resource count
    pub fn get_resource_count(&self) -> Option<u64> {
        None
    }

    /// Check if shutdown has proceeded
    pub async fn shutdown_proceeded(&self) -> bool {
        *self.shutting_down.read().await
    }

    /// Get error state
    pub fn get_error(&self) -> Option<String> {
        None
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
        let should_shutdown = {
            let guard = lifecycle.lock().await;
            guard.should_shutdown().await
        };

        if should_shutdown {
            tracing::info!("Idle timeout exceeded, shutting down daemon");
            let guard = lifecycle.lock().await;
            guard.shutdown().await;
            break;
        }
    }
}
