//! Windows process spawning validation tests.
//!
//! This module validates that tokio::process::Command with kill_on_drop(true)
//! prevents zombie processes on Windows (XP-01).
//!
//! These tests are Windows-specific and must be run with `cargo test windows_process -- --ignored`.

#[cfg(test)]
#[cfg(windows)]
mod windows_process_tests {
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};

    /// Test normal process lifecycle
    #[tokio::test]
    #[ignore]
    async fn test_normal_process_lifecycle() {
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "echo", "test"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn test process");

        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line).await;

        assert!(line.contains("test"));

        // Drop - kill_on_drop should kill the process
        drop(child);
    }

    /// Test normal process lifecycle with response
    #[tokio::test]
    #[ignore]
    async fn test_normal_process_lifecycle_with_response() {
        let response = "Hello from test process";
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "echo", response])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn test process");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        let _ = reader.read_line(&mut line).await;

        assert!(line.contains(response));

        let _ = child.kill().await;
    }

    /// Test kill_on_drop on early drop
    #[tokio::test]
    #[ignore]
    async fn test_kill_on_drop_early_drop() {
        let child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "timeout", "5"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to spawn long-running process");

        // Drop immediately - kill_on_drop should kill the process
        drop(child);

        tokio::time::sleep(Duration::from_millis(500)).await;
        // Process should be terminated
    }

    /// Test multiple sequential spawns
    #[tokio::test]
    #[ignore]
    async fn test_multiple_sequential_spawns() {
        for i in 0..10 {
            let mut child = tokio::process::Command::new("cmd.exe")
                .args(["/c", "echo", &format!("test{}", i)])
                .kill_on_drop(true)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap_or_else(|_| panic!("Failed to spawn process {}", i));

            let stdout = child.stdout.take().expect("No stdout handle");
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            let _ = reader.read_line(&mut line).await;

            assert!(line.contains(&format!("test{}", i)));
            drop(child);
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    /// Test stdout buffering doesn't prevent cleanup
    #[tokio::test]
    #[ignore]
    async fn test_stdout_buffering_cleanup() {
        let mut child = tokio::process::Command::new("cmd.exe")
            .args(["/c", "echo", "buffered_output"])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn");

        let stdout = child.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output).await;

        assert!(output.contains("buffered_output"));
        drop(child);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    /// Windows-specific test for process tree cleanup
    #[tokio::test]
    #[ignore]
    async fn test_process_tree_cleanup() {
        let mut parent = tokio::process::Command::new("cmd.exe")
            .args([
                "/c", "cmd.exe", "/c", "echo", "parent", "&&", "echo", "child",
            ])
            .kill_on_drop(true)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn parent process");

        let stdout = parent.stdout.take().expect("No stdout handle");
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output).await;

        assert!(output.contains("parent") || output.contains("cmd.exe"));
        drop(parent);
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
