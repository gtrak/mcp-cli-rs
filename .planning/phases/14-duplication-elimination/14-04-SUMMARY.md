---
phase: 14
code: "04"
name: "Connection Interface Deduplication"
subsystem: "ipc, daemon"
tags: ["deduplication", "trait", "refactoring"]
requires:
  - "14-01 (Transport trait consolidation)"
provides:
  - "DUP-03: Connection interface deduplication"
  - "DUP-04: Shared MCP initialization"
affects:
  - "14-05 (JSON consolidation review)"
  - "14-06 (Final verification)"
tech-stack:
  added: []
  patterns:
    - "Trait delegation to inherent methods"
    - "Shared helper extraction"
key-files:
  created: []
  modified:
    - src/ipc/mod.rs
    - src/daemon/pool.rs
    - src/pool/mod.rs
decisions:
  - id: "DUP-03-IMPL"
    description: "ProtocolClient trait impl delegates to IpcClientWrapper methods"
    rationale: "Eliminates ~60 lines of duplicated request/response matching logic"
  - id: "DUP-04-IMPL"
    description: "Extracted initialize_mcp_connection helper in pool.rs"
    rationale: "Both execute() and list_tools() share MCP init logic"
metrics:
  duration: "30 minutes"
  completed: "2026-02-12"
---

# Phase 14 Plan 04: Connection Interface Deduplication Summary

**One-liner:** Deduplicated ProtocolClient trait implementation and extracted shared MCP initialization helper.

## What Was Changed

### Task 1: Deduplicated ProtocolClient trait implementation (src/ipc/mod.rs)

**Before:** The `ProtocolClient` trait implementation for `IpcClientWrapper<T>` duplicated the same request/response matching logic that already existed in the inherent methods of `IpcClientWrapper`.

- `list_servers()` - duplicated in both inherent impl and trait impl
- `list_tools()` - duplicated in both inherent impl and trait impl  
- `execute_tool()` - duplicated in both inherent impl and trait impl

**After:** The trait implementation now delegates to the inherent methods:

```rust
async fn list_servers(&mut self) -> Result<Vec<String>, McpError> {
    IpcClientWrapper::list_servers(self).await
}

async fn list_tools(&mut self, server_name: &str) -> Result<Vec<ToolInfo>, McpError> {
    IpcClientWrapper::list_tools(self, server_name).await
}

async fn execute_tool(&mut self, server_name: &str, tool_name: &str, arguments: Value) -> Result<Value, McpError> {
    IpcClientWrapper::execute_tool(self, server_name, tool_name, arguments).await
}
```

**Impact:** ~60 lines of duplicated code removed. Single implementation path for each operation.

### Task 2: Extracted shared MCP initialization helper (src/daemon/pool.rs)

**Before:** Both `execute()` and `list_tools()` methods had identical MCP initialization code (initialize request + initialized notification) duplicated inline.

**After:** Shared helper method extracted:

```rust
async fn initialize_mcp_connection(transport: &mut BoxedTransport) -> Result<()> {
    // Send initialize request
    transport.send(init_request).await?;
    // Send initialized notification
    transport.send_notification(initialized_notification).await?;
    Ok(())
}
```

Both methods now call `Self::initialize_mcp_connection(&mut conn.transport).await?;`.

**Impact:** ~35 lines of duplicated code removed. DUP-04 partially satisfied.

## Deviations from Plan

### Deviation 1: Kept ProtocolClient name (not renamed to McpClient)

**Reason:** There's already a `McpClient` struct in `src/client/mod.rs` that is publicly exported. Renaming the trait would create a naming collision.

**Resolution:** Kept the trait name as `ProtocolClient` but achieved the deduplication goal by having the trait implementation delegate to inherent methods.

### Deviation 2: Removed server_info, subscribe, unsubscribe from trait

**Reason:** The daemon protocol doesn't have `Subscription` type or `ServerInfo`, `Subscribe`, `Unsubscribe` request/response variants. These methods would have no protocol support.

**Resolution:** The trait contains only methods that map to existing protocol functionality: `list_servers`, `list_tools`, `execute_tool`, `shutdown`.

## Verification

- `cargo check` - PASS
- `cargo clippy --lib` - PASS (zero warnings)
- `cargo test --lib` - PASS (98/98 tests)

## Lines Changed

| File | Before | After | Delta |
|------|--------|-------|-------|
| src/ipc/mod.rs | ~253 lines | ~217 lines | -36 lines |
| src/daemon/pool.rs | ~300 lines | ~260 lines | -40 lines |
| **Total** | | | **~76 lines removed** |

## Success Criteria

| Criteria | Status |
|----------|--------|
| DUP-03: Connection interface deduplication eliminated | ✅ PASS - Trait impl delegates to inherent methods |
| DUP-04: Duplicate list_tools/call_tool implementations consolidated | ✅ PARTIAL - pool.rs shares init logic |
| Single implementation path for each operation | ✅ PASS |
| All tests pass | ✅ PASS (98/98) |

## Next Steps

- 14-05: JSON consolidation review (verify no duplicate JSON handling)
- 14-06: Final verification of all duplication elimination
