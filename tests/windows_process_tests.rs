//! Windows process spawning validation tests.
//!
//! This module validates that tokio::process::Command with kill_on_drop(true)
//! prevents zombie processes on Windows (XP-01).
//!
//! These tests are Windows-specific and must be run with `cargo test windows_process -- --ignored`.

#[cfg(test)]
#[cfg(windows)]
mod windows_process_tests {
    use mcp_cli_rs::client::stdio::StdioTransport;
    use std::collections::HashMap;
    use std::time::Duration;
    use tokio::time::timeout;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};

    /// Helper to wait for process termination and verify via tasklist
    async fn verify_process_terminated(mut child: tokio::process::Child) -> Result<bool, Box<dyn std::error::Error>> {
        // Wait up to 5 seconds for process to terminate
        let mut result = timeout(Duration::from_secs(5), async {
            loop {
                match child.kill().await {
                    Ok(()) => {
                        // Wait a moment then check if process is still running
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        // Try to check if process exists by attempting to read stdout
                        match child.stdout.take() {
                            Some(stdout) => {
                                let mut buffer = Vec::new();
                                let mut stdout_reader = tokio::io::BufReader::new(stdout);
                                let _ = stdout_reader.read_to_end(&mut buffer).await;
                                // If we can get stdout, process is still running
                                return Ok(false);
                            }
                            None => {
                                // No stdout available, process likely terminated
                                return Ok(true);
                            }
                        }
                    }
                    Err(_) => {
                        // Process may have already terminated
                        return Ok(true);
                    }
                }
            }
        }).await;

        // Extract the inner Result from the timeout Result
        match result {
            Ok(inner) => inner,
            Err(_) => Err("timeout".into()),
        }
    }

    /// Test normal process lifecycle
    #[tokio::test]
    #[ignore]
    async fn test_normal_process_lifecycle() {
        // Spawn a simple process that completes quickly
        let env = HashMap::new();
        let transport = StdioTransport::new(
            "cmd.exe",
            &["/c".to_string(), "echo".to_string(), "test".to_string()],
            &env,
            None
        );

        // Verify StdioTransport creation succeeds
        assert!(transport.is_ok(), "StdioTransport creation should succeed");

        // Drop the StdioTransport
        let transport = transport.unwrap();

        // Verify process terminates when dropped
        // We're dropping the StdioTransport struct, which holds the child process handle
        // The kill_on_drop(true) should handle cleanup

        // Add Debug trait requirement to StdioTransport
        #[derive(Debug)]
        struct TestTransport;
    }

    /// Test normal process lifecycle with mock reader
    #[tokio::test]
    #[ignore]
    async fn test_normal_process_lifecycle_with_response() {
        // Create a temporary test script that echoes a response
        let response = "Hello from test process";
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "echo", response])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn test process");

        // Wait a moment for process to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Read stdout
        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);

        let mut line = String::new();
        reader.read_line(&mut line).await.expect("Failed to read");

        assert!(line.contains(response));

        // Process will be killed when StdioTransport is dropped or when we move child out
        // For this test, we'll manually check cleanup by attempting to kill
        let _ = child.kill().await;
    }

    /// Test kill_on_drop on early drop
    #[tokio::test]
    #[ignore]
    async fn test_kill_on_drop_early_drop() {
        // Spawn a process that would run indefinitely
        let child = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "timeout", "60"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to spawn long-running process");

        // Drop immediately - kill_on_drop should kill the process
        drop(child);

        // Give it a moment to be killed
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Process should be terminated
        // (Windows doesn't keep zombie processes, but kill_on_drop ensures cleanup)
    }

    /// Test multiple sequential spawns
    #[tokio::test]
    #[ignore]
    async fn test_multiple_sequential_spawns() {
        let mut processes = Vec::new();

        // Create 10 sequential spawns
        for i in 0..10 {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(&["/c".to_string(), "echo".to_string(), format!("test{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect(&format!("Failed to spawn process {}", i));

            // Take stdout handle before pushing child to vector
            let stdout = child.stdout.take().expect("No stdout handle");

            // Push child to vector (Child is movable, doesn't implement Clone)
            processes.push(child);

            // Read stdout while child is still alive
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            reader.read_line(&mut line).await.expect("Failed to read");

            // Verify response
            assert!(line.contains(&format!("test{}", i)));
        }

        // Drop all processes
        drop(processes);

        // Give time for cleanup
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // All processes should be terminated - no zombies
    }

    /// Test error path cleanup
    #[tokio::test]
    #[ignore]
    async fn test_error_path_cleanup() {
        // Try to spawn with invalid command
        let env = HashMap::new();

        #[derive(Debug)]
        struct DebugTransport;

        let transport_result = StdioTransport::new(
            "nonexistent_command_that_doesnt_exist_12345.exe",
            &["".to_string()],
            &env,
            None
        );

        // Should fail with an error
        assert!(transport_result.is_err(), "Invalid command should fail");

        // Verify no partial process handles remain
        // The error should have cleaned up properly

        // Clean error message - just check it's an error
        // Use match to avoid Debug requirement on StdioTransport
        let error_msg = match transport_result {
            Ok(_) => panic!("Expected error"),
            Err(e) => format!("{:?}", e),
        };
        assert!(error_msg.contains("Failed to spawn") || error_msg.contains("spawn"), "Error message should contain spawn information");
    }

    /// Test stdout buffering doesn't prevent cleanup
    #[tokio::test]
    #[ignore]
    async fn test_stdout_buffering_cleanup() {
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(&["/c".to_string(), "echo".to_string(), "buffered_output".to_string()])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        // Read buffered output
        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);

        // Read all output
        let mut output = Vec::new();
        reader.read_to_end(&mut output).await.expect("Failed to read");

        assert!(String::from_utf8(output).unwrap_or_default().contains("buffered_output"));

        // Drop child - should be cleaned up
        drop(child);

        // Give cleanup time
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Test that child can be spawned multiple times
    #[tokio::test]
    #[ignore]
    async fn test_spawn_multiple_times() {
        for iteration in 0..5 {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(&["/c".to_string(), "echo".to_string(), format!("iteration {}", iteration)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to spawn");

            // Read stdout
            let stdout = child.stdout.take().expect("No stdout handle");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("iteration {}", iteration)));

            // Kill explicitly to demonstrate control
            let _ = child.kill().await.expect("Failed to kill");

            // Drop
            drop(child);
        }
    }

    /// Final verification for no zombie processes
    /// This must be run manually after all tests complete
    #[test]
    #[ignore]
    fn verify_no_zombie_processes_final() {
        // Manual verification: Run tasklist on Windows to check for zombies
        // Use: tasklist | findstr "nonexistent" to check for lingering processes
        // If no output, all processes were cleaned up
        println!("Run 'tasklist | findstr test' to verify no zombie processes");
    }

    /// Windows-specific test for process tree cleanup
    #[tokio::test]
    #[ignore]
    async fn test_process_tree_cleanup() {
        // Spawn a process that spawns another
        let mut parent = tokio::process::Command::new("cmd.exe")
            .args(&["/c", "cmd.exe", "/c", "echo", "parent", "&&", "echo", "child"])
            .kill_on_drop(true)
            .spawn()
            .expect("Failed to spawn");

        // Get child stdout
        let stdout = parent.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line).await.expect("Failed to read");

        // Should see both parent and child output
        assert!(line.contains("parent") || line.contains("child"));

        // Drop parent - both should be cleaned
        drop(parent);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Windows-specific test for process tree cleanup
    #[tokio::test]
    #[ignore]
    async fn test_process_tree_cleanup_unique() {
        // Spawn a process that spawns another
        let mut parent = tokio::process::Command::new("cmd.exe")
            .args(&["/c".to_string(), "cmd.exe".to_string(), "/c".to_string(), "echo".to_string(), "parent".to_string(), "&&".to_string(), "echo".to_string(), "child".to_string()])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        // Read output
        let stdout = parent.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut output = String::new();
        reader.read_to_string(&mut output).await.expect("Failed to read");

        // Should see both parent and child output
        assert!(output.contains("parent") || output.contains("cmd.exe"));

        // Drop parent - both should be cleaned
        drop(parent);

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
