# Phase 14: Duplication Elimination - Research

**Researched:** 2026-02-13
**Domain:** Rust CLI refactoring, Model-View separation, Trait unification
**Confidence:** HIGH

## Summary

This phase consolidates duplicate code across JSON command functions and connection interfaces. The codebase has 16 command functions (8 human + 8 JSON pairs) that can be reduced to 8 multi-mode commands using an OutputMode parameter. Similarly, three separate connection interfaces (daemon/pool.rs, client/mod.rs, ipc/mod.rs) need unification under a single McpClient trait.

**Primary recommendation:** Implement Model + Formatter pattern first (source of truth for testing), then unify McpClient trait with async_trait, starting with connection interfaces before command consolidation.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Model + Formatter Architecture**: Internal model types capture daemon responses (not formatted strings), separate formatters convert models to human or JSON output, commands return models not formatted output
- **Consolidation Strategy**: Start with connection interfaces (unified McpClient trait), then move to commands
- **McpClient Trait Design**: Async-only with async_trait, minimal operations (list_tools, call_tool, server_info, subscribe, unsubscribe), `&mut self` ownership, concrete return types
- **Error Handling**: Generic `Result<T, McpError>` with thiserror for nested errors
- **Backward Compatibility**: Breaking changes allowed (internal APIs only), atomic commits for trait changes + caller updates
- **Test Locations**: `tests/cli/command_models_test.rs` and `tests/cli/formatters_test.rs`
- **Model Location**: `src/cli/models.rs`

### Claude's Discretion
- Error handling approach - thiserror vs other options
- Exact implementation details of formatters

### Deferred Ideas (OUT OF SCOPE)
- None - all ideas stayed within phase scope
</user_constraints>

---

## Standard Stack

The project already uses the required dependencies:

### Core Dependencies
| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| async-trait | 0.1 | Async trait objects | Already in Cargo.toml |
| thiserror | 1.0 | Error enum derivation | Already in Cargo.toml |
| serde | 1.0 | Serialization | Already in Cargo.toml |
| serde_json | 1.0 | JSON handling | Already in Cargo.toml |

### No New Dependencies Required
All libraries needed for this phase are already in the project's Cargo.toml.

---

## Architecture Patterns

### Pattern 1: Model + Formatter (Model-View Separation)

This pattern separates data models from presentation, enabling:
- Single source of truth for testing (models)
- Multiple output formats from same data (formatters)
- Cleaner command logic (just orchestrates model → format)

**Implementation Structure:**

```
src/cli/
├── models.rs           # Internal model types (daemon responses)
├── formatters.rs       # Human and JSON formatters
├── list.rs            # Returns ListModel, delegates to formatter
├── info.rs            # Returns ServerInfoModel/ToolInfoModel
├── call.rs            # Returns CallResultModel
└── search.rs          # Returns SearchResultModel
```

**Model Example:**
```rust
// src/cli/models.rs
use serde::{Deserialize, Serialize};

/// Model for list command output (captures all daemon response fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListServersModel {
    pub servers: Vec<ServerModel>,
    pub total_servers: usize,
    pub connected_servers: usize,
    pub failed_servers: usize,
    pub total_tools: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerModel {
    pub name: String,
    pub status: String,
    pub tool_count: usize,
    pub tools: Vec<ToolModel>,
    pub error: Option<String>,
}

// Similar models for: ServerInfoModel, ToolInfoModel, CallResultModel, SearchResultModel
```

**Formatter Example:**
```rust
// src/cli/formatters.rs
use crate::cli::models::*;
use crate::format::OutputMode;
use crate::output::{print_json, print_error};

/// Format list servers result for human or JSON output
pub fn format_list_servers(model: &ListServersModel, output_mode: OutputMode) {
    match output_mode {
        OutputMode::Human => format_list_servers_human(model),
        OutputMode::Json => print_json(model),
    }
}

fn format_list_servers_human(model: &ListServersModel) {
    // Human-readable formatting with colored output
    println!("{} {}", "MCP Servers".bold(), /* ... */);
}
```

### Pattern 2: Unified McpClient Trait

The trait unifies three existing interfaces (pool, client, IPC) with async_trait:

```rust
// src/client/trait.rs
use async_trait::async_trait;
use crate::error::{McpError, Result};
use crate::daemon::protocol::ToolInfo;

/// Unified MCP client trait
#[async_trait]
pub trait McpClient: Send + Sync {
    /// List available tools on a server
    async fn list_tools(&mut self, server_name: &str) -> Result<Vec<ToolInfo>>;
    
    /// Call a tool on a server
    async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value>;
    
    /// Get server information
    async fn server_info(&mut self, server_name: &str) -> Result<ServerInfo>;
    
    /// Subscribe to server notifications
    async fn subscribe(&mut self, server_name: &str, method: &str) -> Result<()>;
    
    /// Unsubscribe from server notifications
    async fn unsubscribe(&mut self, server_name: &str, method: &str) -> Result<()>;
}

/// Server information returned by server_info
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub capabilities: serde_json::Value,
    pub protocol_version: Option<String>,
}
```

### Pattern 3: Multi-Mode Command Pattern

Consolidates human/JSON pairs into single function with OutputMode parameter:

```rust
// Before: 16 functions (8 pairs)
pub async fn cmd_list_servers(...) -> Result<()>;
pub async fn cmd_list_servers_json(...) -> Result<()>;

// After: 8 functions
pub async fn cmd_list_servers(
    daemon: Box<dyn ProtocolClient>,
    detail_level: DetailLevel,
    output_mode: OutputMode,  // <-- Added parameter
) -> Result<()>;
```

---

## Don't Hand-Roll

The project already has existing solutions for these problems:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|------------|-----|
| Async trait objects | Manual Pin<Box<dyn Future>> | async-trait crate | Stable, well-tested, handles Send/Sync |
| Error enum | Custom Error impl | thiserror derive | Already in Cargo.toml, clean derive macro |
| JSON serialization | Manual serialization | serde + serde_json | Already in Cargo.toml |
| Output formatting | Duplicated formatting | Formatter functions | Centralizes formatting logic |

---

## Common Pitfalls

### Pitfall 1: Trait Object Lifetime Issues

**What goes wrong:** Using `&self` in async_trait methods with boxed futures causes lifetime errors.

**Why it happens:** async_trait erases lifetimes, but `&self` borrows require careful handling.

**How to avoid:** Use `&mut self` (as locked in decisions) which works correctly with async_trait. The existing code already uses `&mut self` in ProtocolClient.

**Warning signs:**
- Lifetime errors in trait method signatures
- Future not satisfying trait bounds

### Pitfall 2: Breaking Changes Without Atomic Commits

**What goes wrong:** Updating trait then forgetting to update all callers causes compilation failures.

**Why it happens:** Multiple files depend on the trait, easy to miss one.

**How to avoid:** Atomic commits that update trait + all callers together. The locked decision requires this approach.

**Warning signs:**
- Compiler errors after partial updates
- Multiple files need simultaneous changes

### Pitfall 3: Incomplete Model Coverage

**What goes wrong:** Model missing fields that human formatter needs, causing runtime errors.

**Why it happens:** Daemon response fields not fully captured in model struct.

**How to avoid:** Create model tests that verify all daemon response fields are captured. The locked decision requires `tests/cli/command_models_test.rs`.

**Warning signs:**
- Missing fields when formatting
- Runtime panics in formatter

### Pitfall 4: Formatter Duplication

**What goes wrong:** Each command creates its own formatter instead of sharing.

**Why it happens:** No centralized formatter module.

**How to avoid:** Create `src/cli/formatters.rs` with shared formatting functions, as specified in the locked decisions.

---

## Code Examples

### Converting Existing Command to Model + Formatter

The `cmd_list_servers` function in `src/cli/list.rs` (461 lines) shows:
1. The JSON mode is handled separately (lines 37-39, 339-460)
2. Both modes do identical daemon queries (lines 66-109 human, 360-403 JSON)
3. Only the output formatting differs

**Migration approach:**

```rust
// src/cli/list.rs - After refactoring
pub async fn cmd_list_servers(
    daemon: Box<dyn ProtocolClient>,
    detail_level: DetailLevel,
    output_mode: OutputMode,
) -> Result<()> {
    // Single query logic (not duplicated)
    let model = query_list_servers(daemon).await?;
    
    // Format with appropriate formatter
    format_list_servers(&model, output_mode);
    Ok(())
}

async fn query_list_servers(mut daemon: Box<dyn ProtocolClient>) -> Result<ListServersModel> {
    // Query logic - returns model
    // ... existing parallel query logic ...
    Ok(model)
}
```

### Unified Trait from Existing ProtocolClient

The existing `ProtocolClient` trait in `src/ipc/mod.rs` (lines 149-167) already provides:
- `list_tools` → maps to McpClient::list_tools
- `execute_tool` → maps to McpClient::call_tool
- `list_servers` → additional method

The McpClient trait should be designed to replace or wrap this trait.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate cmd_xxx and cmd_xxx_json functions | Single function with OutputMode parameter | This phase | 16→8 functions |
| Three connection interfaces | Unified McpClient trait | This phase | Single source of truth |
| Inline formatting in commands | Separate formatter module | This phase | Testable, reusable |
| Duplicate list_tools implementations | Single trait implementation | This phase | Maintainable |

---

## Open Questions

1. **Should McpClient trait replace ProtocolClient or wrap it?**
   - What we know: ProtocolClient exists in ipc/mod.rs with specific methods
   - What's unclear: Whether to make McpClient the primary trait or create a wrapper
   - Recommendation: Create McpClient as new primary trait, adapt ProtocolClient to delegate

2. **How to handle subscribe/unsubscribe methods in McpClient?**
   - What we know: The locked decisions mention these as core operations
   - What's unclear: Current codebase doesn't show explicit subscribe/unsubscribe implementation
   - Recommendation: Add as optional methods or stub implementations for future use

3. **Transport duplication (DUP-05)?**
   - What we know: src/transport.rs (81 lines) and src/client/transport.rs (68 lines) are similar
   - What's unclear: Whether to consolidate or keep separate (different purposes)
   - Recommendation: Keep in src/transport.rs only if possible, remove client/transport.rs duplication

---

## Sources

### Primary (HIGH confidence)
- async-trait crate documentation (docs.rs) - async_trait usage patterns
- thiserror crate documentation (docs.rs) - Error enum patterns
- Existing codebase: src/cli/*.rs, src/ipc/mod.rs, src/daemon/pool.rs, src/client/mod.rs

### Secondary (MEDIUM confidence)
- Code patterns verified against existing implementations
- Error handling patterns from src/error.rs

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all dependencies already in Cargo.toml
- Architecture: HIGH - patterns verified against existing codebase structure
- Pitfalls: HIGH - based on existing code issues observed during research

**Research date:** 2026-02-13
**Valid until:** 2026-03-13 (stable - no fast-moving dependencies)
