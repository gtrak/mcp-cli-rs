//! Common test patterns shared across platforms
//!
//! Provides utilities and patterns used by both Unix and Windows tests

use crate::helpers;
use mcp_cli_rs::daemon::protocol::{DaemonRequest, DaemonResponse};
use mcp_cli_rs::ipc;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::BufReader;
use tokio::time::timeout;

/// Test helper: IPC roundtrip with timeout
///
/// Common pattern used across platform-specific tests
pub async fn test_ipc_roundtrip_with_timeout(
    socket_path: PathBuf,
    request: DaemonRequest,
    expected_response: DaemonResponse,
) -> anyhow::Result<()> {
    // Implementation using helpers and common patterns
    let server = ipc::create_ipc_server(&socket_path)?;
    let expected_response_clone = expected_response.clone();
    let server_handle = tokio::spawn(async move {
        let result = timeout(
            std::time::Duration::from_secs(5),
            server.accept()
        ).await;
        let (mut stream, _addr) = match result {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => panic!("Server accept failed: {}", e),
            Err(e) => panic!("Server accept timed out: {}", e),
        };

        let mut buf_reader = tokio::io::BufReader::new(stream);
        let req = mcp_cli_rs::daemon::protocol::receive_request(&mut buf_reader)
            .await
            .expect("Failed to receive request");

        assert!(matches!(req, request));

        mcp_cli_rs::daemon::protocol::send_response(&mut buf_reader, &expected_response_clone)
            .await
            .expect("Failed to send response");
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let config = Arc::new(mcp_cli_rs::config::Config::with_socket_path(socket_path.clone()));
    let mut client = ipc::create_ipc_client(&*config)?;

    let response = client.send_request(&request).await?;
    assert!(
        std::mem::discriminant(&response) == std::mem::discriminant(&expected_response)
    );

    server_handle.await?;

    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}
