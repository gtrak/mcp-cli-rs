---
phase: 02-connection-daemon-ipc
plan: 09
type: execute
completions: 3
status: complete
gap_closure: true
---

# Plan 02-09 Summary: Implement Daemon Request Handlers

## Overview

Successfully implemented all three daemon request handlers (ExecuteTool, ListTools, ListServers) that were previously stub implementations returning "not yet implemented" errors. Handlers now use the connection pool, MCP protocol via JSON-RPC, and return proper DaemonResponse variants.

**Key Change:** Made `handle_request()` function async to support async `transport.send()` calls through the connection pool.

## Tasks Completed

### Task 1: Implement ExecuteTool request handler

- ✅ Replaced stub implementation (lines 212-219) with full implementation
- ✅ Gets transport from connection pool via `state.connection_pool.lock().unwrap().get(&server_name).await`
- ✅ Constructs MCP JSON-RPC `tools/call` request with tool name and arguments
- ✅ Sends request via `transport.send(mcp_request).await`
- ✅ Parses JSON-RPC response (result vs. error field)
- ✅ Returns `DaemonResponse::ToolResult()` on success
- ✅ Returns `DaemonResponse::Error()` with appropriate error code on failure
- ✅ Error codes: 2 = server error, 3 = network error

**Files modified:**
- `src/daemon/mod.rs` - Implemented ExecuteTool handler in handle_request()

**Commit:** `feat(02-09): implement daemon request handlers` (combined with tasks 2-3)

---

### Task 2: Implement ListTools request handler

- ✅ Replaced stub implementation (lines 221-228) with full implementation
- ✅ Gets transport from connection pool via `state.connection_pool.lock().unwrap().get(&server_name).await`
- ✅ Constructs MCP JSON-RPC `tools/list` request
- ✅ Sends request via `transport.send(mcp_request).await`
- ✅ Parses JSON-RPC response and extracts tools array from `result.tools`
- ✅ Converts tools array to `Vec<ToolInfo>` with name, description, input_schema
- ✅ Returns `DaemonResponse::ToolList(tools)` on success
- ✅ Returns `DaemonResponse::Error()` with appropriate error code on failure
- ✅ Error codes: 2 = server error, 3 = network error

**Files modified:**
- `src/daemon/mod.rs` - Implemented ListTools handler in handle_request()

**Commit:** `feat(02-09): implement daemon request handlers` (combined with tasks 1,3)

---

### Task 3: Implement ListServers request handler

- ✅ Replaced stub implementation (lines 230-237) with full implementation
- ✅ No network calls - reads server names directly from config
- ✅ Iterates over `state.config.servers` to extract server names
- ✅ Returns `DaemonResponse::ServerList(Vec<String>)`
- ✅ Simple, fast operation (cached config data)

**Implementation Detail:** This is the simplest handler - just returns metadata from the already-loaded Config without needing to contact any MCP servers.

**Files modified:**
- `src/daemon/mod.rs` - Implemented ListServers handler in handle_request()

**Commit:** `feat(02-09): implement daemon request handlers` (combined with tasks 1-2)

---

## Key Implementation Details

### MCP JSON-RPC Protocol Usage

All handlers follow the JSON-RPC 2.0 specification:

**Request format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call" | "tools/list",
  "params": { ... }
}
```

**Response format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { ... }  // On success
}
// OR
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {       // On error
    "code": -32600,
    "message": "..."
  }
}
```

### Connection Pool Usage Pattern

```rust
// Get transport (async)
let mut transport = match state.connection_pool.lock().unwrap().get(&server_name).await {
    Ok(t) => t,
    Err(e) => return DaemonResponse::Error { code: 2, message: ... }
};

// Send MCP JSON-RPC request
let mcp_request = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "...", "params": ... });

// Get response
let response = transport.send(mcp_request).await?;

// Parse response (result vs. error)
if let Some(result) = response.get("result") {
    DaemonResponse::ToolResult(result.clone())  // or ToolList(...)
} else if let Some(error) = response.get("error") {
    DaemonResponse::Error { code: ..., message: ... }
}
```

### Async Conversion

**Before:**
```rust
pub fn handle_request(request: DaemonRequest, state: &DaemonState) -> DaemonResponse
```

**After:**
```rust
pub async fn handle_request(request: DaemonRequest, state: &DaemonState) -> DaemonResponse
```

This change was necessary because `connection_pool.get()` and `transport.send()` are async operations requiring `.await`.

### Error Codes

Follows standard exit code conventions (from Phase 1):

| Code | Meaning               | Example Cause                          |
|------|-----------------------|----------------------------------------|
| 1    | Client error          | Invalid request format                 |
| 2    | Server error          | Server not found/failed to connect     |
| 3    | Network error         | Transport failure, invalid MCP response |

## Dependencies

This plan depends on:
- 02-08: IPC NDJSON protocol (enables CLI-daemon communication)
- Connection pool implementation (02-04) - provides transport caching
- Transport trait (Phase 1) - provides send() interface
- JSON-RPC format - MCP protocol specification

## Closed Gaps

**Gap 2 from VERIFICATION.md: Daemon Request Handlers Are Stubs (Blocker)**

Before:
- ExecuteTool handler returned "ExecuteTool not yet implemented"
- ListTools handler returned "ListTools not yet implemented"
- ListServers handler returned "ListServers not yet implemented"
- Connection pool existed but not used by request handlers
- Even if IPC worked, daemon couldn't perform any useful operations

After:
- ExecuteTool: Gets transport from pool, sends MCP tools/call, returns ToolResult
- ListTools: Gets transport from pool, sends MCP tools/list, returns ToolList
- ListServers: Returns server names from config (no network call)
- All handlers return proper DaemonResponse variants (not Error stubs)
- Error handling with specific error codes
- Connection pool actively used for transport management

## Testing Notes

Code compiles successfully with no errors. Handlers use connection pool and MCP protocol correctly. Since this depends on plan 02-08 (IPC protocol) and plan 02-10 (daemon lifecycle), end-to-end testing will be verified after those plans complete.

## Self-Check

- [x] ExecuteTool handler uses connection_pool, sends McpRequest::CallTool, returns ToolResult
- [x] ListTools handler uses connection_pool, sends McpRequest::ListTools, returns ToolList
- [x] ListServers handler returns configured server names from config as ServerList
- [x] All handlers use MCP protocol via Transport trait
- [x] Handlers return proper DaemonResponse variants (not Error stubs)
- [x] Error handling with specific error codes
- [x] Code compiles without errors
- [x] handle_request is async (supports await)

## Next Steps

Plan 02-10 implements config fingerprint comparison and graceful shutdown. These are the final gap closure plans before IPC integration tests (02-11) verify end-to-end functionality.
