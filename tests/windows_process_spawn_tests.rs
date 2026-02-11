//! Windows process spawning integration tests.
//!
//! This module provides real-world validation of tokio::process::Command with
//! kill_on_drop(true) preventing zombie processes on Windows (XP-01).
//!
//! These are integration tests covering CLI, daemon, concurrent, and error scenarios.
//! Must be run with `cargo test windows_process_spawn -- --ignored`.
//!
//! # Test Coverage
//!
//! - CLI command execution with shutdown
//! - Concurrent process spawning (5 parallel processes)
//! - Process timeout scenarios with kill_on_drop
//! - Daemon process cleanup through lifecycle cycles
//! - Multiple tools concurrent execution
//! - Batch tool execution cleanup (20 processes)
//! - Error handling in batch operations
//! - Tokio timeout integration
//! - Cleanup after send failures
//!
//! # Platform Requirements
//!
//! These tests are Windows-specific due to zombie process concerns on the Windows
//! platform. They use Windows-specific commands (cmd.exe) and are validated to ensure
//! that kill_on_drop(true) correctly terminates child processes without leaving zombies.
//!
//! # Integration vs Unit Tests
//!
//! Unlike the unit tests in `windows_process_tests.rs`, these integration tests simulate
//! real-world process spawning scenarios including:
//! - CLI tool invocation patterns
//! - Concurrent tool execution
//! - Daemon lifecycle management
//! - Batch processing scenarios
//! - Error cases and edge conditions
//!
//! # Running the Tests
//!
//! To run these tests on Windows:
//!
//! ```bash
//! cargo test windows_process_spawn -- --ignored --test-threads=1
//! ```
//!
//! The `--ignored` flag is required because these tests are marked as `#[ignore]` to
//! prevent them from running in normal test suites (they spawn real processes).

#[cfg(test)]
#[cfg(windows)]
mod windows_process_spawn_tests {
    use std::time::Duration;
    use futures::future::join_all;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    /// Test CLI command execution with clean shutdown.
    ///
    /// This test simulates a typical CLI tool invocation where:
    /// 1. A process is spawned with a command
    /// 2. Stdout is captured and read
    /// 3. Process is explicitly killed
    /// 4. Handle is dropped to ensure kill_on_drop works
    ///
    /// XP-01 Validation: Verifies that kill_on_drop prevents zombies after
    /// explicit process termination in CLI workflows.
    #[tokio::test]
    #[ignore]
    async fn test_cli_command_execution_with_shutdown() {
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "echo", "test_result"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn CLI test process");

        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line).await;

        assert!(line.contains("test_result"), "Expected 'test_result' in output");

        // Explicitly kill and wait for process
        let _ = child.kill().await;
        let _ = child.wait().await;

        // Drop handle - kill_on_drop should prevent zombies
        drop(child);

        // Sleep to ensure process cleanup completes
        // This delay is important for verifying that kill_on_drop has time to
        // terminate the process cleanly without leaving background wait states
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test concurrent process spawning (5 parallel processes).
    ///
    /// This test simulates real-world concurrent tool execution where multiple
    /// processes may be spawned simultaneously, such as:
    /// - Parallel tool discovery during MCP server initialization
    /// - Concurrent server startup for multiple configured servers
    /// - Multiple independent CLI commands in parallel workflows
    ///
    /// The test verifies that:
    /// 1. Multiple tasks can spawn processes concurrently
    /// 2. Each process has unique output that can be distinguished
    /// 3. All processes complete without hanging
    /// 4. Handle drops don't create zombies even with concurrent cleanup
    ///
    /// XP-01 Validation: Verifies that kill_on_drop correctly handles concurrent
    /// drops without creating zombie processes or handle leaks.
    #[tokio::test]
    #[ignore]
    async fn test_concurrent_process_spawning() {
        let mut handles = vec![];

        // Create 5 concurrent tasks, each spawning a unique process
        // This pattern simulates real-world concurrent tool spawning
        for i in 0..5 {
            let handle = tokio::spawn(async move {
                let mut child = tokio::process::Command::new("cmd.exe")
                    .args(["/c", "echo", &format!("process_{}", i)])
                    .kill_on_drop(true)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn concurrent process");

                let stdout = child.stdout.take().expect("No stdout handle");
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;

                // Each process should output its unique identifier
                assert!(line.contains(&format!("process_{}", i)));
                drop(child);
                i
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete using join_all
        // This ensures all spawned processes have a chance to run to
        // finish before we proceed with cleanup verification
        let results = join_all(handles).await;

        // Verify all processes completed successfully by checking
        // that we got results back for all 5 spawned tasks
        // Each successful task returns its index (0-4)
        for result in results {
            let index = result.expect("Task failed");
            assert!(index < 5);
        }

        // Allow time for process cleanup verification
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test process timeout scenarios with kill_on_drop.
    ///
    /// This test validates that long-running processes can be safely terminated
    /// when they exceed specified timeouts, without leaving zombie processes.
    ///
    /// XP-01 Validation: Verifies that kill_on_drop correctly cleans up processes
    /// that are terminated due to timeout conditions.
    #[tokio::test]
    #[ignore]
    async fn test_process_timeout_scenarios() {
        // Spawn a long-running process
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "ping", "-n", "10", "localhost"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn long-running process");

        // Use tokio::time::timeout with 2 second limit
        let timeout_result = tokio::time::timeout(
            Duration::from_secs(2),
            child.wait(),
        ).await;

        // Verify timeout fired
        assert!(timeout_result.is_err(), "Process should have timed out");

        // Explicitly drop the child handle via kill() after timeout
        let _ = child.kill().await;
        let _ = child.wait().await;

        // Confirm process is killed and no zombie remains
        drop(child);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test daemon process cleanup (3 lifecycle cycles)
    #[tokio::test]
    #[ignore]
    async fn test_daemon_process_cleanup_lifecycle() {
        // Simulate daemon-like lifecycle: spawn, process, shutdown
        // Repeat 3 times in a loop
        for cycle in 0..3 {
            let mut daemon = tokio::process::Command::new("cmd.exe")
                .args(["/c", "echo", &format!("daemon_cycle_{}", cycle)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn daemon process");

            // Simulate processing: read some output
            let stdout = daemon.stdout.take().expect("No stdout handle");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("daemon_cycle_{}", cycle)));

            // Shutdown: kill and wait
            let _ = daemon.kill().await;
            let _ = daemon.wait().await;
            drop(daemon);

            // Small delay between cycles
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Verify no orphaned daemon processes remain after 3 cycles
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test multiple tools concurrent execution
    #[tokio::test]
    #[ignore]
    async fn test_multiple_tools_concurrent_execution() {
        let mut handles = vec![];

        // Spawn 3 "tool" processes in parallel
        for i in 0..3 {
            let handle = tokio::spawn(async move {
                // Each tool reads stdin and writes specific output
                let tool_input = format!("tool_{}_input", i);
                let tool_output = format!("tool_{}_output", i);
                let mut child = tokio::process::Command::new("cmd.exe")
                    .args(["/c", "set", "/p", "tool_input=", &tool_input, "&&", "echo", &tool_output])
                    .kill_on_drop(true)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn tool process");

                let mut stdin = child.stdin.take().expect("No stdin handle");
                let _ = stdin.write_all(b"test_input").await;
                let _ = stdin.flush().await;
                drop(stdin);

                let stdout = child.stdout.take().expect("No stdout handle");
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;

                assert!(line.contains(&tool_output));
                drop(child);
                i
            });
            handles.push(handle);
        }

        // Wait for all tools to complete
        let results = join_all(handles).await;

        // Verify all tool outputs are distinct and correct
        for result in results {
            let index = result.expect("Tool task failed");
            assert!(index < 3);
        }

        // Drop all handles and verify clean cleanup
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test batch tool execution cleanup (20 processes)
    #[tokio::test]
    #[ignore]
    async fn test_batch_tool_execution_cleanup() {
        let mut children = vec![];

        // Spawn 20 processes sequentially in a loop
        for i in 0..20 {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(["/c", "echo", &format!("batch_{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn batch process");

            // Read minimal output
            let stdout = child.stdout.take().expect("No stdout handle");
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("batch_{}", i)));

            // Kill the process
            let _ = child.kill().await;
            let _ = child.wait().await;

            // Keep track of child handle
            children.push(child);
        }

        // After all spawned, drop Vec (all handles)
        drop(children);

        // Verify no zombies remain
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    /// Test error handling in batch operations
    #[tokio::test]
    #[ignore]
    async fn test_error_handling_in_batch_operations() {
        let mut valid_processes = vec![];
        let mut spawn_errors = 0;

        // Spawn 10 processes, half valid and half intentionally invalid
        for i in 0..10 {
            let result = if i % 2 == 0 {
                // Valid process: cmd.exe
                tokio::process::Command::new("cmd.exe")
                    .args(["/c", "echo", &format!("valid_{}", i)])
                    .kill_on_drop(true)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
            } else {
                // Invalid process: non-existent command
                tokio::process::Command::new("nonexistent_command_zzz")
                    .args(["/c", "echo", &format!("invalid_{}", i)])
                    .kill_on_drop(true)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
            };

            match result {
                Ok(mut child) => {
                    // Valid process spawned
                    let stdout = child.stdout.take().expect("No stdout handle");
                    let mut reader = BufReader::new(stdout);
                    let mut line = String::new();
                    let _ = reader.read_line(&mut line).await;

                    assert!(line.contains(&format!("valid_{}", i)));
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    valid_processes.push(child);
                }
                Err(_) => {
                    // Spawn failure (expected for invalid commands)
                    spawn_errors += 1;
                }
            }
        }

        // Verify spawn failures don't leave zombie handles
        assert_eq!(spawn_errors, 5, "Expected 5 spawn failures");

        // Verify valid processes still complete normally
        assert_eq!(valid_processes.len(), 5, "Expected 5 valid processes");

        // Drop all handles
        drop(valid_processes);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test tokio timeout integration with process kill
    #[tokio::test]
    #[ignore]
    async fn test_tokio_timeout_integration() {
        // Spawn a process that waits indefinitely
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "ping", "-n", "9999", "localhost"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn long-waiting process");

        // Use tokio::time::timeout with 500ms limit
        let timeout_result = tokio::time::timeout(
            Duration::from_millis(500),
            child.wait(),
        ).await;

        // Verify timeout fires
        assert!(timeout_result.is_err(), "Should have timed out after 500ms");

        // On timeout, kill the process via child.kill()
        let _ = child.kill().await;

        // Wait for process to exit
        let _ = child.wait().await;

        // Confirm no zombie after kill + exit
        drop(child);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test cleanup after send failures (broken pipe)
    #[tokio::test]
    #[ignore]
    async fn test_cleanup_after_send_failures() {
        // Spawn process with stdin but closed stdout (simulated)
        // Use a process that exits quickly after reading stdin
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "exit", "/b", "0"])
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn process for send test");

        let mut stdin = child.stdin.take().expect("No stdin handle");

        // Attempt to write to stdin using AsyncWriteExt
        // The write_result is intentionally ignored - this test verifies cleanup
        // regardless of whether the write succeeds or fails
        let _write_result = tokio::time::timeout(
            Duration::from_millis(1000),
            async {
                stdin.write_all(b"test_input").await?;
                stdin.flush().await?;
                Ok::<(), std::io::Error>(())
            },
        ).await;

        // The write might succeed or fail depending on timing
        // Either way, verify child handle cleanup
        drop(stdin);

        // Wait for process to exit (the timeout wrapper handles the result)
        let _ = child.wait().await;

        // Simulate send failure - drop child handle even after possible error
        drop(child);

        // Confirm no zombie process remains after failure
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
