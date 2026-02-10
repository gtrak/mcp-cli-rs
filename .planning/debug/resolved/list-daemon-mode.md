---
status: investigating
trigger: "Investigate why `cargo run list` doesn't work correctly in `--require-daemon` mode"
created: 2026-02-09T00:00:00.000Z
updated: 2026-02-09T00:00:00.000Z
---

## Current Focus

hypothesis: CONFIRMED - The Windows named pipe path includes `std::process::id()` in its name
test: N/A - Root cause identified
expecting: N/A
next_action: Apply fix by removing process ID from Windows named pipe path

## Symptoms

expected: `cargo run list` should work correctly both in normal mode and in `--require-daemon` mode
actual: `cargo run list` with `--require-daemon` flag fails
errors: "Failed to accept named pipe connection" or "Failed to connect to daemon"
reproduction: Run `cargo run list --require-daemon`
started: Unknown when this broke, appears to be a UAT discovery

## Evidence

- timestamp: 2025-02-09T00:00:00.000Z
  checked: src/ipc/mod.rs, get_socket_path() function
  found: On Windows, returns `\\\\.\\pipe\\mcp-cli-daemon-{std::process::id()}`
  implication: The named pipe path includes the current process ID, which means daemon and client will use DIFFERENT pipe names since they are separate processes

- timestamp: 2025-02-09T00:00:01.000Z
  checked: src/ipc/mod.rs, Unix implementation
  found: On Unix, uses fixed path based on user ID: `/run/user/{uid}/mcp-cli/daemon.sock` or `/tmp/mcp-cli-{uid}/daemon.sock`
  implication: Unix implementation is correct - same path for all processes from the same user
  conclusion: Windows implementation should use a fixed named pipe name, not process ID

## Eliminated
<!-- APPEND only -->

## Resolution

root_cause: The Windows named pipe path in `get_socket_path()` includes `std::process::id()`, which causes the daemon and client to use DIFFERENT pipe names. The daemon creates a pipe with its PID (e.g., `\\\\.\\pipe\\mcp-cli-daemon-12345`), but the client tries to connect using its own PID (e.g., `\\\\.\\pipe\\mcp-cli-daemon-67890`). Since these are different named pipes, the client cannot find or connect to the daemon.

fix: Removed `std::process::id()` from the Windows named pipe path and changed it to use a fixed pipe name: `\\\\.\\pipe\\mcp-cli-daemon-socket`, similar to how Unix uses a fixed socket path.

verification: Successfully tested `cargo run --release -- list --require-daemon` and confirmed the daemon-client IPC connection now works. The command progressed to attempting to list tools from the configured MCP server (which failed due to a separate MCP server initialization issue, unrelated to the daemon-client IPC).

files_changed: [src/ipc/mod.rs]
