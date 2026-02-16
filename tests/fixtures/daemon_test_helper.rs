//! Test helpers for daemon IPC integration tests
//!
//! Provides utilities for:
//! - Spawning test daemon processes
//! - Managing daemon lifecycle (start, shutdown, cleanup)
//! - Creating IPC clients connected to test daemon
//! - Configuring mock MCP servers for daemon tests

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// Thread-safe counter for generating unique daemon socket paths
static DAEMON_SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

use mcp_cli_rs::config::{Config, ServerConfig, ServerTransport};
use mcp_cli_rs::daemon::lifecycle::DaemonLifecycle;
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc::{self, ProtocolClient};

/// Handle to a running test daemon
///
/// Provides methods for:
/// - Creating IPC clients connected to this daemon
/// - Sending shutdown requests
/// - Clean resource cleanup
pub struct TestDaemon {
    /// Socket/pipe path for IPC communication
    pub socket_path: PathBuf,
    /// Daemon configuration
    pub config: Arc<Config>,
    /// Channel to signal daemon shutdown
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Daemon task handle
    daemon_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// Temp directory holding the socket file (must be kept alive)
    #[allow(dead_code)]
    temp_dir: TempDir,
}

impl TestDaemon {
    /// Create a new IPC client connected to this daemon
    ///
    /// Returns a boxed ProtocolClient ready for communication
    pub fn client(&self) -> Result<Box<dyn ProtocolClient>> {
        Ok(ipc::create_ipc_client(&self.config)?)
    }

    /// Shutdown the daemon gracefully
    ///
    /// Sends shutdown request and waits for acknowledgment
    pub async fn shutdown(mut self) -> Result<()> {
        // Try graceful shutdown via IPC first
        if let Ok(mut client) = self.client() {
            let _ = client.send_request(&DaemonRequest::Shutdown).await;
        }

        // Signal the daemon task to stop
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Wait for daemon to complete with timeout
        if let Some(handle) = self.daemon_handle.take() {
            let _ = timeout(Duration::from_secs(5), handle).await;
        }

        // Clean up socket file (Unix only)
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        Ok(())
    }

    /// Get the socket path for this daemon
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Get a reference to the daemon configuration
    pub fn config(&self) -> &Arc<Config> {
        &self.config
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        // Clean up socket file on drop (Unix only)
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// Spawn a test daemon with the given configuration
///
/// # Arguments
/// * `config` - Daemon configuration with MCP server definitions
///
/// # Returns
/// * `TestDaemon` handle for interacting with the spawned daemon
///
/// # Example
/// ```rust
/// let config = create_test_config().await?;
/// let daemon = spawn_test_daemon(config).await?;
/// let mut client = daemon.client()?;
/// let response = client.send_request(&DaemonRequest::Ping).await?;
/// daemon.shutdown().await?;
/// ```
pub async fn spawn_test_daemon(config: Config) -> Result<TestDaemon> {
    // Create unique socket path in temp directory
    let temp_dir = TempDir::new()?;
    let socket_path = get_daemon_socket_path(&temp_dir);

    // Update config with socket path
    let mut config = config;
    config.socket_path = socket_path.clone();

    let config = Arc::new(config);

    // Create lifecycle manager with short idle timeout for tests
    let lifecycle = DaemonLifecycle::new(300); // 5 minute timeout for tests

    // Create shutdown channel
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    // Clone values for daemon task
    let daemon_config = Arc::clone(&config);
    let daemon_socket = socket_path.clone();

    // Spawn daemon in background task
    let daemon_handle = tokio::spawn(async move {
        // Run daemon with shutdown signal
        let daemon_future = mcp_cli_rs::daemon::run_daemon(
            (*daemon_config).clone(),
            daemon_socket,
            lifecycle,
        );

        tokio::select! {
            result = daemon_future => result,
            _ = &mut shutdown_rx => {
                tracing::info!("Daemon received shutdown signal");
                Ok(())
            }
        }
    });

    // Wait for socket file to exist (daemon is ready)
    let socket_ready = tokio::time::timeout(
        Duration::from_secs(5),
        async {
            loop {
                if socket_path.exists() {
                    // Give a bit more time for daemon to start listening
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    ).await;

    if socket_ready.is_err() {
        return Err(anyhow::anyhow!("Daemon failed to create socket within 5 seconds"));
    }

    Ok(TestDaemon {
        socket_path: socket_path.clone(),
        config,
        shutdown_tx: Some(shutdown_tx),
        daemon_handle: Some(daemon_handle),
        temp_dir,
    })
}

/// Default tools configuration for mock server
fn default_mock_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "echo",
            "description": "Echo back the input message",
            "input_schema": {
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "Message to echo"}
                },
                "required": ["message"]
            }
        }),
        serde_json::json!({
            "name": "add",
            "description": "Add two numbers",
            "input_schema": {
                "type": "object",
                "properties": {
                    "a": {"type": "number", "description": "First number"},
                    "b": {"type": "number", "description": "Second number"}
                },
                "required": ["a", "b"]
            }
        }),
    ]
}

/// Default responses configuration for mock server
fn default_mock_responses() -> std::collections::HashMap<String, serde_json::Value> {
    let mut responses = std::collections::HashMap::new();
    responses.insert(
        "echo".to_string(),
        serde_json::json!({
            "content": [{"type": "text", "text": "Echo: {message}"}]
        }),
    );
    responses.insert(
        "add".to_string(),
        serde_json::json!({
            "content": [{"type": "text", "text": "Result: {result}"}]
        }),
    );
    responses
}

/// Create a test configuration with a mock stdio server
///
/// Returns a Config with one mock MCP server configured
pub async fn create_test_config() -> Result<Config> {
    let temp_dir = TempDir::new()?;

    // Get path to mock MCP server binary
    let mock_server_path = find_mock_server_binary()?;

    // Configure mock server with default tools
    let tools = default_mock_tools();
    let responses = default_mock_responses();

    let mut env = std::collections::HashMap::new();
    env.insert("MOCK_TOOLS".to_string(), serde_json::to_string(&tools)?);
    env.insert("MOCK_RESPONSES".to_string(), serde_json::to_string(&responses)?);

    let server_config = ServerConfig {
        name: "mock-server".to_string(),
        transport: ServerTransport::Stdio {
            command: mock_server_path.to_string_lossy().to_string(),
            args: vec![],
            env,
            cwd: None,
        },
        description: None,
        allowed_tools: None,
        disabled_tools: None,
    };

    let config = Config {
        servers: vec![server_config],
        concurrency_limit: 5,
        retry_max: 3,
        retry_delay_ms: 1000,
        timeout_secs: 1800,
        daemon_ttl: 60,
        socket_path: temp_dir.path().join("daemon.sock"),
    };

    Ok(config)
}

/// Create a test configuration with multiple mock tools
///
/// Useful for testing concurrent tool execution
pub async fn create_test_config_with_tools(tool_count: usize) -> Result<Config> {
    let temp_dir = TempDir::new()?;
    let mock_server_path = find_mock_server_binary()?;

    // Create environment variables for multiple tools
    let mut env = std::collections::HashMap::new();

    // Build tools JSON with numbered tools
    let tools: Vec<serde_json::Value> = (0..tool_count)
        .map(|i| {
            serde_json::json!({
                "name": format!("tool_{}", i),
                "description": format!("Test tool {}", i),
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "value": {"type": "number"}
                    }
                }
            })
        })
        .collect();

    // Build responses for each tool
    let responses: std::collections::HashMap<String, serde_json::Value> = (0..tool_count)
        .map(|i| {
            (
                format!("tool_{}", i),
                serde_json::json!({
                    "content": [{"type": "text", "text": format!("Result from tool {}", i)}]
                }),
            )
        })
        .collect();

    env.insert("MOCK_TOOLS".to_string(), serde_json::to_string(&tools)?);
    env.insert("MOCK_RESPONSES".to_string(), serde_json::to_string(&responses)?);

    let server_config = ServerConfig {
        name: "mock-server-multi".to_string(),
        transport: ServerTransport::Stdio {
            command: mock_server_path.to_string_lossy().to_string(),
            args: vec![],
            env,
            cwd: None,
        },
        description: None,
        allowed_tools: None,
        disabled_tools: None,
    };

    let config = Config {
        servers: vec![server_config],
        concurrency_limit: 5,
        retry_max: 3,
        retry_delay_ms: 1000,
        timeout_secs: 1800,
        daemon_ttl: 60,
        socket_path: temp_dir.path().join("daemon.sock"),
    };

    Ok(config)
}

/// Create a test configuration with specified mock server tools and responses
///
/// # Arguments
/// * `tools` - JSON array of tool definitions
/// * `responses` - JSON object mapping tool names to responses
///
/// # Returns
/// * `Config` with configured mock server
pub fn create_test_config_with_mock_data(
    tools: serde_json::Value,
    responses: serde_json::Value,
) -> Result<Config> {
    let temp_dir = TempDir::new()?;
    let mock_server_path = find_mock_server_binary()?;

    let mut env = std::collections::HashMap::new();
    env.insert("MOCK_TOOLS".to_string(), serde_json::to_string(&tools)?);
    env.insert("MOCK_RESPONSES".to_string(), serde_json::to_string(&responses)?);

    let server_config = ServerConfig {
        name: "mock-server-custom".to_string(),
        transport: ServerTransport::Stdio {
            command: mock_server_path.to_string_lossy().to_string(),
            args: vec![],
            env,
            cwd: None,
        },
        description: None,
        allowed_tools: None,
        disabled_tools: None,
    };

    let config = Config {
        servers: vec![server_config],
        concurrency_limit: 5,
        retry_max: 3,
        retry_delay_ms: 1000,
        timeout_secs: 1800,
        daemon_ttl: 60,
        socket_path: temp_dir.path().join("daemon.sock"),
    };

    Ok(config)
}

/// Find the mock MCP server binary path
///
/// Searches in target/debug, target/release, and current exe directory
fn find_mock_server_binary() -> Result<PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().unwrap();

    // Check for binary in various locations
    let candidates = [
        exe_dir.join("mock-mcp-server.exe"),
        exe_dir.join("mock-mcp-server"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/mock-mcp-server.exe"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/mock-mcp-server"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/mock-mcp-server.exe"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/mock-mcp-server"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    anyhow::bail!("mock-mcp-server binary not found. Run: cargo build --bin mock-mcp-server")
}

/// Get platform-specific socket path for daemon
///
/// On Unix: returns a path in the temp directory
/// On Windows: returns a named pipe path
/// Uses thread-safe atomic counter to ensure unique paths in parallel tests.
#[cfg(unix)]
fn get_daemon_socket_path(temp_dir: &TempDir) -> PathBuf {
    let counter = DAEMON_SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    temp_dir.path().join(format!(
        "daemon-test-{}-{}.sock",
        std::process::id(),
        counter
    ))
}

#[cfg(windows)]
fn get_daemon_socket_path(_temp_dir: &TempDir) -> PathBuf {
    let counter = DAEMON_SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    PathBuf::from(format!(
        r"\\.\pipe\mcp-cli-daemon-test-{}-{}",
        std::process::id(),
        counter
    ))
}

/// Wait for daemon to be ready for connections
///
/// Attempts to connect with retries until successful or timeout
pub async fn wait_for_daemon_ready(daemon: &TestDaemon, max_wait: Duration) -> Result<()> {
    let start = tokio::time::Instant::now();

    while start.elapsed() < max_wait {
        if let Ok(mut client) = daemon.client() {
            if let Ok(response) = client.send_request(&DaemonRequest::Ping).await {
                if matches!(response, DaemonResponse::Pong) {
                    return Ok(());
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    anyhow::bail!("Daemon failed to become ready within {:?}", max_wait)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_mock_server_binary() {
        // This will fail if binary not built, but that's OK for unit test
        let _result = find_mock_server_binary();
        // Just verify the function doesn't panic
    }
}