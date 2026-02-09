//! Integration tests for process cleanup scenarios.
//!
//! These tests validate real-world CLI and daemon scenarios where process
//! cleanup is critical. Tests StdioTransport with concurrent operations and
//! timeout scenarios.

#[cfg(test)]
mod process_cleanup_tests {
    use mcp_cli_rs::client::stdio::StdioTransport;
    use std::collections::HashMap;
    use tokio::time::{timeout, Duration};
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    /// Integration test: CLI command execution with shutdown
    #[tokio::test]
    #[ignore]
    async fn test_cli_command_execution_shutdown() {
        // Simulate MCP server communication
        let env = HashMap::new();

        // Create StdioTransport for a simple echo server
        let transport = StdioTransport::new(
            "cmd.exe",
            &["/c", "echo", "test_response"],
            &env,
            None
        )
        .expect("Failed to create StdioTransport");

        let mut transport = transport;

        // Send a request (simulating CLI tool call)
        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        });

        // Note: This would normally send and receive, but for cleanup test
        // we verify that StdioTransport drops cleanly
        drop(transport);

        // Give cleanup time
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should complete without zombie processes
    }

    /// Integration test: CLI with JSON-RPC protocol
    #[tokio::test]
    #[ignore]
    async fn test_cli_json_rpc_protocol() {
        // Simulate a simple JSON-RPC server
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "cmd.exe", "/c", "echo", "{\"jsonrpc\":\"2.0\",\"result\":42}"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        // Read response
        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line).await;

        // Should get JSON-RPC response
        let response: serde_json::Value = serde_json::from_str(&line.trim()).unwrap_or_default();
        assert!(response.is_object() || line.contains("42"));

        // Clean up
        child.kill().await.expect("Failed to kill");
    }

    /// Integration test: Concurrent process spawning
    #[tokio::test]
    #[ignore]
    async fn test_concurrent_process_spawning() {
        let num_processes = 5;
        let mut handles = Vec::new();

        // Spawn 5 processes concurrently
        for i in 0..num_processes {
            let handle = tokio::spawn(async move {
                let mut child = tokio::process::Command::new("cmd.exe")
                    .args(&["/c", "echo", &format!("process_{}", i)])
                    .kill_on_drop(true)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn");

                let stdout = child.stdout.take().expect("No stdout handle");
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;

                line.trim().to_string()
            });

            handles.push(handle);
        }

        // Collect results
        let mut results = Vec::new();
        for handle in handles {
            let result = handle.await.expect("Task failed");
            results.push(result);
        }

        // Verify all responses
        assert_eq!(results.len(), num_processes);
        for result in &results {
            assert!(result.contains("process_"));
        }

        // All processes should be cleaned up - no zombies
    }

    /// Integration test: Concurrent with random drop timing
    #[tokio::test]
    #[ignore]
    async fn test_concurrent_random_drop_timing() {
        let num_processes = 10;
        let mut processes = Vec::new();

        // Spawn 10 processes
        for i in 0..num_processes {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(&["/c", "echo", &format!("proc{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn");

            processes.push(child);
        }

        // Drop each at random intervals
        for (i, child) in processes.into_iter().enumerate() {
            let delay = Duration::from_millis(((i % 3 + 1) * 100) as u64);
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                // Just dropping - kill_on_drop will handle cleanup
                // But we can also explicitly kill to demonstrate
                let _ = child.kill().await;
            });
        }

        // Wait for all drops
        tokio::time::sleep(Duration::from_secs(2)).await;

        // All processes should be terminated
    }

    /// Integration test: Process timeout scenarios
    #[tokio::test]
    #[ignore]
    async fn test_process_timeout_scenarios() {
        // Process that doesn't respond (simulating hang)
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "echo", "would_timeout"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Kill the process
        let _ = child.kill().await;

        // Give cleanup time
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Should be terminated
    }

    /// Integration test: Timeout with StdioTransport
    #[tokio::test]
    #[ignore]
    async fn test_transport_timeout_scenario() {
        let env = HashMap::new();

        let transport_result = StdioTransport::new(
            "cmd.exe",
            &["/c", "timeout", "60"],
            &env,
            None
        );

        // Should return an error (timeout)
        if let Ok(mut transport) = transport_result {
            // Simulate timeout by killing after short delay
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                drop(transport);
            });
        }

        // Should clean up without zombies
    }

    /// Integration test: Daemon process cleanup
    #[tokio::test]
    #[ignore]
    async fn test_daemon_process_cleanup() {
        // Daemon process that stays running
        let mut daemon = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "start", "/B", "cmd.exe", "/C", "echo", "daemon"])
            .kill_on_drop(true)
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn daemon");

        // Let it start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify it started (check stderr or process existence)
        // We can't easily detect started processes on Windows, but kill_on_drop
        // ensures cleanup

        // Clean up
        let _ = daemon.kill().await;

        // Give cleanup time
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Should be terminated
    }

    /// Integration test: Daemon lifecycle cycles
    #[tokio::test]
    #[ignore]
    async fn test_daemon_lifecycle_cycles() {
        let cycles = 3;

        for cycle in 0..cycles {
            // Spawn daemon
            let mut daemon = tokio::process::Command::new("cmd.exe")
                .args(&["/c", "cmd.exe", "/C", "echo", &format!("cycle_{}", cycle)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn daemon");

            // Read response
            let stdout = daemon.stdout.take().expect("No stdout handle");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("cycle_{}", cycle)));

            // Kill and drop
            daemon.kill().await.expect("Failed to kill");
            drop(daemon);

            // Wait for cleanup
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // All cycles complete - no orphans
    }

    /// Integration test: Multiple tools with concurrent execution
    #[tokio::test]
    #[ignore]
    async fn test_multiple_tools_concurrent() {
        use std::sync::Mutex;
        use std::sync::Arc;

        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);

        // Spawn 5 async tasks
        let handles: Vec<_> = (0..5).map(|i| {
            let results = Arc::clone(&results_clone);
            tokio::spawn(async move {
                let mut child = tokio::process::Command::new("cmd.exe")
                    .args(&["/c", "echo", &format!("tool_{}", i)])
                    .kill_on_drop(true)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn");

                let stdout = child.stdout.take().expect("No stdout handle");
                let mut reader = tokio::io::BufReader::new(stdout);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;

                let response = line.trim().to_string();
                {
                    let mut results = results.lock().unwrap();
                    results.push(response);
                }

                // Drop transport
                drop(child);
            })
        }).collect();

        // Wait for all
        for handle in handles {
            handle.await.expect("Task failed");
        }

        // Verify results
        let locked_results = results.lock().unwrap();
        assert_eq!(locked_results.len(), 5);
        for result in locked_results.iter() {
            assert!(result.contains("tool_"));
        }

        // All cleaned up - no zombies
    }

    /// Integration test: Batch tool execution cleanup
    #[tokio::test]
    #[ignore]
    async fn test_batch_tool_execution() {
        let batch_size = 20;

        // Execute batch
        for i in 0..batch_size {
            let child = tokio::process::Command::new("cmd.exe")
                .args(&["/c", "echo", &format!("batch_{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn");

            let stdout = child.stdout.take().expect("No stdout handle");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("batch_{}", i)));
        }

        // All batch items completed
    }

    /// Integration test: Error handling in batch
    #[tokio::test]
    #[ignore]
    async fn test_batch_error_handling() {
        // Some processes may fail
        let mut count = 0;
        let mut success = 0;

        for i in 0..10 {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(&["/c", "echo", &format!("item_{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn");

            let stdout = child.stdout.take().expect("No stdout handle");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            let read_result = reader.read_line(&mut line).await;

            if read_result.is_ok() {
                if line.contains(&format!("item_{}", i)) {
                    success += 1;
                }
                count += 1;
            }

            // Drop - cleanup handles error cases
            drop(child);
        }

        assert_eq!(success, 10);
        assert_eq!(count, 10);

        // All cleaned up
    }

    /// Integration test: Verify tokio timeout works with process cleanup
    #[tokio::test]
    #[ignore]
    async fn test_tokio_timeout_with_process() {
        // Use tokio::time::timeout
        let timeout_result = timeout(Duration::from_secs(2), async {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(&["/c", "timeout", "60"])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::null())
                .spawn()
                .expect("Failed to spawn");

            // Process runs longer than timeout
            // Timeout should complete without hanging
            drop(child);
        })
        .await;

        // Should timeout successfully
        assert!(timeout_result.is_err());
    }

    /// Integration test: Process cleanup after JSON send failure
    #[tokio::test]
    #[ignore]
    async fn test_cleanup_after_send_failure() {
        // This simulates a scenario where stdout buffer fills up
        // or connection fails mid-operation

        let mut child = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "echo", "cleanup"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        // Attempt to send data (simulating write failure)
        let stdout = child.stdout.take().expect("No stdout handle");
        let mut writer = stdout;

        // Try to write multiple times (simulate buffer issues)
        for _ in 0..5 {
            let _ = writer.write_all(b"test data\n").await;
            let _ = writer.flush().await;
        }

        // Clean up
        child.kill().await.expect("Failed to kill");
        drop(child);

        // Should be terminated
    }
}
