# Phase 5: Unified Daemon Architecture - Research

**Researched:** 2026-02-09
**Domain:** Rust CLI Architecture, Process Management, IPC
**Confidence:** HIGH

## Summary

This research covers refactoring the MCP CLI from a two-binary architecture (CLI + daemon) to a unified single binary with three operational modes. The key technical challenges are:

1. **Subcommand Architecture**: Converting daemon from separate binary to a `daemon` subcommand using clap's derive macros while maintaining backward compatibility.

2. **Operational Mode Parsing**: Implementing three distinct modes (standalone daemon, auto-spawn, require-daemon) with appropriate flag handling and mutual exclusion.

3. **TTL Configuration Hierarchy**: Implementing the standard precedence chain (CLI flag > env var > config file > default) for daemon idle timeout.

4. **Race Condition Handling**: Managing concurrent daemon spawns through existing PID file mechanism and connection retry logic.

5. **Build Configuration**: Removing the `[[bin]]` entry for daemon from Cargo.toml and consolidating code.

**Primary recommendation:** Use clap's derive macro subcommands with a `DaemonArgs` struct for the daemon subcommand, implement configuration resolution following the 12-factor app pattern, and leverage the existing PID file mechanism for race condition prevention.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Architecture:**
- Single binary: mcp-cli-rs.exe (or mcp)
- Daemon logic becomes a library module called by main CLI
- Three explicit modes instead of implicit behavior

**Operational Modes:**

**Mode 1: Standalone Daemon**
- Command: `mcp daemon [--ttl <seconds>]`
- Behavior: Run as persistent background daemon
- Use case: Long-running sessions, shared daemon across multiple CLI invocations
- If TTL specified, daemon shuts down after period of inactivity

**Mode 2: Auto-spawn (One-shot with daemon)**
- Command: `mcp --auto-daemon [--ttl <seconds>] <command>`
- Behavior: 
  1. Check if daemon running
  2. If not, spawn daemon with specified TTL
  3. Execute command
  4. Daemon auto-shuts down after TTL of inactivity
- Use case: Best performance without manual daemon management
- Default TTL: 60 seconds (configurable)

**Mode 3: Require Daemon (One-shot, explicit dependency)**
- Command: `mcp --require-daemon <command>`
- Behavior:
  1. Check if daemon running
  2. If not, fail with clear error message
  3. If yes, execute command
- Use case: Scripts that need guaranteed daemon, CI/CD pipelines
- Error message: "Daemon not running. Start with: mcp daemon"

**Default Behavior (no flags):**
- Current behavior: Try to use daemon, spawn if not running
- This becomes: Mode 2 with default TTL (auto-spawn)
- No breaking change for existing users

**TTL Configuration Priority:**
- Environment variable: `MCP_DAEMON_TTL` (seconds)
- Command-line flag: `--daemon-ttl <seconds>`
- Config file: `daemon_ttl` field
- Priority: CLI flag > env var > config file > default (60s)

### Claude's Discretion

- Exact implementation of daemon command parsing
- How to handle concurrent daemon spawns (race condition)
- Signal handling coordination between modes
- Logging/verbosity in different modes

### Deferred Ideas (OUT OF SCOPE)

- Daemon status command (check if running, get PID) — future enhancement
- Daemon restart command — can be done manually (stop + start)
- Multiple daemon instances — out of scope, single daemon per user

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing with derive macros | Industry standard for Rust CLI, supports subcommands and global flags |
| tokio | 1.35 | Async runtime for daemon lifecycle | Already used in codebase, handles signals and process spawning |
| serde | 1.0 | Config serialization | Standard for Rust, already used |
| anyhow | 1.0 | Error handling | Standard for application code, already used |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::process | built-in | Async process spawning | For spawning daemon in background |
| tokio::signal | built-in | Cross-platform signal handling | For graceful shutdown in daemon mode |
| std::sync::Mutex/Arc | stdlib | Shared state management | For daemon state (already used in lifecycle.rs) |

**Installation:**
```bash
# Already in Cargo.toml - no changes needed
cargo add clap --features derive  # Already present
```

## Architecture Patterns

### Recommended Project Structure After Refactor

```
src/
├── main.rs              # Entry point with mode detection
├── cli/
│   ├── mod.rs
│   ├── commands.rs      # Existing command handlers
│   └── daemon.rs        # Modified: daemon lifecycle management (spawning, connecting)
├── daemon/
│   ├── mod.rs           # run_daemon() function - called by main for daemon mode
│   ├── protocol.rs      # IPC protocol (unchanged)
│   ├── lifecycle.rs     # Idle timeout management (needs TTL config support)
│   ├── pool.rs          # Connection pool (unchanged)
│   ├── fingerprint.rs   # Config fingerprinting (unchanged)
│   └── orphan.rs        # PID file management (unchanged)
├── ipc/                 # IPC abstraction (unchanged)
├── config/              # Config with daemon_ttl field added
└── lib.rs               # Library exports
```

### Pattern 1: Subcommand-Based Daemon Mode

**What:** Add a `Daemon` variant to the existing `Commands` enum for the standalone daemon mode.

**When to use:** When user explicitly wants to run a persistent daemon process.

**Example:**
```rust
// Source: Based on existing main.rs + clap derive patterns
#[derive(Parser)]
#[command(name = "mcp")]
struct Cli {
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Run without daemon (direct mode)
    #[arg(long, global = true)]
    no_daemon: bool,

    /// Auto-spawn daemon if not running (with optional TTL)
    #[arg(long, global = true, value_name = "TTL")]
    auto_daemon: Option<Option<u64>>, // None = use default, Some(None) = flag present no value, Some(Some(t)) = explicit value

    /// Require daemon to be running (fail if not)
    #[arg(long, global = true)]
    require_daemon: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Subcommand)]
enum Commands {
    // ... existing commands (List, Info, Tool, Call, Search)
    
    /// Run as a persistent background daemon
    Daemon {
        /// Idle timeout in seconds (0 = no timeout)
        #[arg(short, long, value_name = "SECONDS")]
        ttl: Option<u64>,
    },
}
```

### Pattern 2: Global Mode Flags with Mutual Exclusion

**What:** Use clap's `group` feature to ensure `--no-daemon`, `--auto-daemon`, and `--require-daemon` are mutually exclusive.

**When to use:** To provide clear error messages when conflicting flags are provided.

**Example:**
```rust
// Source: Based on clap 4.5 documentation
use clap::{Parser, Args, ArgGroup};

#[derive(Parser)]
#[command(group = ArgGroup::new("daemon_mode")
    .args(&["no_daemon", "auto_daemon", "require_daemon"])
    .multiple(false)
)]
struct Cli {
    #[arg(long, group = "daemon_mode")]
    no_daemon: bool,
    
    #[arg(long, group = "daemon_mode")]
    auto_daemon: bool,
    
    #[arg(long, group = "daemon_mode")]
    require_daemon: bool,
}
```

### Pattern 3: Configuration Resolution (12-Factor Pattern)

**What:** Implement hierarchical configuration with clear precedence.

**When to use:** For TTL and other configurable daemon parameters.

**Example:**
```rust
// Source: Based on config-rs patterns + existing codebase
/// Resolve daemon TTL from multiple sources
/// Priority: CLI arg > env var > config file > default
fn resolve_ttl(
    cli_ttl: Option<u64>,
    config: &Config,
) -> u64 {
    // 1. CLI argument (highest priority)
    if let Some(ttl) = cli_ttl {
        return ttl;
    }
    
    // 2. Environment variable
    if let Ok(env_ttl) = std::env::var("MCP_DAEMON_TTL") {
        if let Ok(ttl) = env_ttl.parse::<u64>() {
            return ttl;
        }
    }
    
    // 3. Config file
    if let Some(config_ttl) = config.daemon_ttl {
        return config_ttl;
    }
    
    // 4. Default (60 seconds)
    60
}
```

### Pattern 4: Self-Executing Binary for Daemon Spawning

**What:** Use the current executable path to spawn the daemon subcommand.

**When to use:** When auto-spawning the daemon from a CLI command.

**Example:**
```rust
// Source: Based on existing src/cli/daemon.rs spawn_daemon_and_wait()
async fn spawn_daemon(ttl: u64) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    
    let mut cmd = tokio::process::Command::new(current_exe);
    cmd.arg("daemon");
    if ttl > 0 {
        cmd.arg("--ttl").arg(ttl.to_string());
    }
    
    // Spawn detached process
    cmd.spawn()?;
    
    // Wait for daemon to be ready
    wait_for_daemon().await
}
```

### Pattern 5: PID File-Based Race Condition Prevention

**What:** Use existing PID file mechanism to prevent concurrent daemon spawns.

**When to use:** When multiple CLI processes try to spawn daemon simultaneously.

**Current implementation:** Already exists in `src/daemon/orphan.rs` and `src/cli/daemon.rs`:
- `cleanup_orphaned_daemon()` - Checks if PID file exists and process is running
- `write_daemon_pid()` - Writes PID after successful spawn
- `spawn_daemon_and_wait()` - Already implements retry logic with exponential backoff

**Key insight:** The existing `wait_for_daemon_startup()` function handles the race condition by retrying connection with exponential backoff.

### Anti-Patterns to Avoid

- **Don't use static/global state for mode detection:** Parse mode from CLI args and pass explicitly through function calls.
- **Don't spawn daemon synchronously in arg parser:** Parse args first, then decide whether to spawn based on mode.
- **Don't ignore spawn errors in auto-daemon mode:** Fall back to direct mode or provide clear error message.
- **Don't use different socket paths for different modes:** Single socket path ensures daemon singleton.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Manual argv parsing | clap derive macros | Type-safe, generates help, validation |
| TTL configuration merging | Manual if-else chain | Hierarchical resolution function | Clear precedence, testable |
| Process spawning | std::process | tokio::process | Async-aware, integrates with runtime |
| Signal handling | signal-hook directly | tokio::signal | Cross-platform, integrates with tokio |
| PID file locking | Manual file operations | Existing orphan.rs module | Already implemented, tested |
| Daemon singleton | Complex distributed locking | PID file + socket existence check | Simple, reliable, already implemented |

**Key insight:** The codebase already has robust daemon lifecycle management. Focus on refactoring to call it from the right places based on operational mode, not reimplementing.

## Common Pitfalls

### Pitfall 1: Flag Confusion in Default Mode

**What goes wrong:** Users expect `--auto-daemon` to be required, but it's the default. This can cause confusion in scripts.

**Why it happens:** Default behavior is implicit, not explicit.

**How to avoid:** Document clearly that no flags = auto-daemon mode. Consider logging at INFO level when auto-spawning.

**Warning signs:** User reports that "daemon spawned unexpectedly" or "script behavior changed".

### Pitfall 2: TTL=0 Interpretation

**What goes wrong:** Users might expect `ttl=0` to mean "immediate shutdown" or "default TTL" instead of "no timeout".

**Why it happens:** Ambiguous semantics of zero value.

**How to avoid:** Document clearly that `ttl=0` means "no idle timeout" (daemon runs forever). Use `Option<u64>` to distinguish "not specified" from "specified as 0".

### Pitfall 3: Concurrent Spawn Race Condition

**What goes wrong:** Two CLI processes spawn daemon simultaneously, leaving orphaned processes.

**Why it happens:** Check-then-act race between connection check and spawn.

**How to avoid:** Rely on existing `wait_for_daemon_startup()` retry logic. The first spawner wins, the second connects to existing daemon.

**Warning signs:** Multiple daemon processes visible in process list, "address already in use" errors.

### Pitfall 4: Signal Handling in Different Modes

**What goes wrong:** Signal handling differs between standalone daemon and CLI-with-daemon modes, causing inconsistent shutdown behavior.

**Why it happens:** Different code paths for signal registration.

**How to avoid:** Use unified signal handling in `main.rs` for all modes. In daemon subcommand mode, signals should trigger graceful daemon shutdown. In other modes, signals should propagate to daemon if running.

### Pitfall 5: Config File Not Found in Daemon Mode

**What goes wrong:** Daemon spawned by auto-daemon mode can't find config file because CWD differs from original CLI invocation.

**Why it happens:** Daemon inherits environment but working directory may differ.

**How to avoid:** Pass absolute config path to spawned daemon using `--config` flag, or ensure config file path is resolved to absolute before spawning.

## Code Examples

### Configuration Resolution Function

```rust
// Source: Pattern from research - to be implemented
/// Resolve daemon TTL from all configuration sources
/// 
/// Priority (highest to lowest):
/// 1. CLI --ttl argument
/// 2. MCP_DAEMON_TTL environment variable
/// 3. config.daemon_ttl field
/// 4. Default (60 seconds)
pub fn resolve_daemon_ttl(
    cli_ttl: Option<u64>,
    config: &Config,
) -> u64 {
    // CLI argument takes precedence
    if let Some(ttl) = cli_ttl {
        return ttl;
    }
    
    // Environment variable
    if let Ok(env_ttl) = std::env::var("MCP_DAEMON_TTL") {
        if let Ok(ttl) = env_ttl.parse::<u64>() {
            return ttl;
        } else {
            tracing::warn!("Invalid MCP_DAEMON_TTL value: {}", env_ttl);
        }
    }
    
    // Config file field (needs to be added to Config struct)
    if let Some(ttl) = config.daemon_ttl {
        return ttl;
    }
    
    // Default
    60
}
```

### Main Entry Point with Mode Detection

```rust
// Source: Based on existing main.rs + research patterns
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    // Determine operational mode
    let mode = determine_mode(&cli);
    
    match mode {
        OperationalMode::Daemon { ttl } => {
            // Run as standalone daemon
            run_daemon_mode(ttl).await;
        }
        OperationalMode::Command { cmd } => {
            // Execute command with appropriate daemon handling
            run_command_mode(cmd, &cli).await;
        }
    }
}

fn determine_mode(cli: &Cli) -> OperationalMode {
    // Check if daemon subcommand was invoked
    if let Some(Commands::Daemon { ttl }) = &cli.command {
        return OperationalMode::Daemon { ttl: *ttl };
    }
    
    // Otherwise, it's a command mode
    OperationalMode::Command { cmd: cli.command.clone() }
}
```

### Command Mode with Daemon Handling

```rust
// Source: Based on existing main.rs patterns
async fn run_command_mode(
    command: Option<Commands>,
    cli: &Cli,
) -> Result<()> {
    let config = load_config(cli.config.as_ref()).await?;
    
    // Determine daemon handling strategy
    let daemon_strategy = if cli.no_daemon {
        DaemonStrategy::Direct
    } else if cli.require_daemon {
        DaemonStrategy::Require
    } else {
        // Default: auto-daemon
        DaemonStrategy::Auto {
            ttl: resolve_daemon_ttl(None, &config),
        }
    };
    
    match daemon_strategy {
        DaemonStrategy::Direct => {
            run_direct(command, config).await
        }
        DaemonStrategy::Require => {
            let client = connect_to_daemon(config.clone()).await
                .map_err(|_| anyhow!("Daemon not running. Start with: mcp daemon"))?;
            execute_command(command, client).await
        }
        DaemonStrategy::Auto { ttl } => {
            let client = ensure_daemon_with_ttl(config.clone(), ttl).await?;
            execute_command(command, client).await
        }
    }
}
```

### Modified Cargo.toml (Build Configuration)

```toml
# Source: Current Cargo.toml with daemon bin removed
[package]
name = "mcp-cli-rs"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "mcp"
path = "src/main.rs"

# REMOVED: daemon binary is now a subcommand
# [[bin]]
# name = "mcp-daemon"
# path = "src/bin/daemon.rs"

[dependencies]
# ... existing dependencies unchanged
```

### Config Struct with daemon_ttl

```rust
// Source: Based on existing src/config/mod.rs
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
    
    #[serde(default = "default_concurrency_limit")]
    pub concurrency_limit: usize,
    
    #[serde(default = "default_retry_max")]
    pub retry_max: u32,
    
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
    
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    
    /// NEW: Default daemon idle timeout in seconds
    /// Used when daemon is auto-spawned without explicit TTL
    #[serde(default)]
    pub daemon_ttl: Option<u64>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Two binaries (mcp + mcp-daemon) | Single binary with subcommands | Phase 5 (current) | Simpler deployment, clearer operational model |
| Hardcoded 60s TTL | Configurable TTL (flag/env/config) | Phase 5 (current) | Flexibility for different use cases |
| Implicit daemon management | Explicit operational modes | Phase 5 (current) | Predictable behavior, better for scripting |
| Manual argv parsing | clap derive macros | Already in codebase | Type-safe, maintainable |
| Separate signal handlers | Unified graceful shutdown | Already in codebase (shutdown.rs) | Consistent behavior across platforms |

**Deprecated/outdated:**
- `src/bin/daemon.rs` as separate binary: Will be removed, functionality moved to library
- Direct daemon binary execution: Users must use `mcp daemon` subcommand instead

## Open Questions

### 1. How should TTL=0 be interpreted in different contexts?

- **What we know:** TTL controls idle timeout before daemon self-terminates
- **What's unclear:** Should TTL=0 mean "no timeout" (run forever) or "immediate shutdown" or "use default"?
- **Recommendation:** TTL=0 means "no timeout" (run forever). Use `Option<u64>` to distinguish unspecified from explicit 0.

### 2. What happens when --require-daemon is used but daemon has incompatible config?

- **What we know:** Current `ensure_daemon()` restarts daemon if fingerprint differs
- **What's unclear:** Should --require-daemon fail if daemon exists but has wrong config, or should it restart daemon?
- **Recommendation:** Document that --require-daemon requires daemon to be running with compatible config. If config differs, return clear error suggesting manual restart.

### 3. How should logging work in auto-spawn mode?

- **What we know:** Current daemon logs to stdout/stderr
- **What's unclear:** Should auto-spawned daemon be silent? Where should its logs go?
- **Recommendation:** Auto-spawned daemon should redirect output to null unless `--verbose` or `RUST_LOG` is set. Standalone `mcp daemon` keeps current behavior (logs to stdout).

## Sources

### Primary (HIGH confidence)

- **Existing codebase analysis:**
  - `src/main.rs` - Current CLI structure with clap derive macros
  - `src/bin/daemon.rs` - Current daemon binary entry point (to be refactored)
  - `src/cli/daemon.rs` - Daemon lifecycle management (spawning, connecting)
  - `src/daemon/mod.rs` - Daemon library code (run_daemon function)
  - `src/daemon/lifecycle.rs` - Idle timeout management (needs TTL config)
  - `src/shutdown.rs` - Cross-platform signal handling implementation
  - `src/config/mod.rs` - Configuration structure (needs daemon_ttl field)
  - `Cargo.toml` - Current binary configuration

- **Context7/clap documentation:** Subcommand derive patterns, global arguments, argument groups for mutual exclusion

### Secondary (MEDIUM confidence)

- **Configuration hierarchy patterns:** 
  - https://rust-cli-recommendations.sunshowers.io/hierarchical-config.html
  - https://github.com/rust-cli/config-rs (standard Rust configuration library)
  - 12-factor app configuration methodology

- **Process spawning patterns:**
  - tokio::process documentation for async process management
  - PID file patterns for daemon singleton enforcement

### Tertiary (LOW confidence)

- **Web search findings on concurrent daemon spawning:** 
  - Race condition prevention through file locking (not verified with code)
  - Various blog posts on daemon patterns (may not reflect current best practices)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - clap and tokio are established, existing codebase already uses them
- Architecture patterns: HIGH - Based on existing codebase patterns + clap documentation
- Pitfalls: MEDIUM-HIGH - Based on common issues in similar daemon implementations

**Research date:** 2026-02-09
**Valid until:** 90 days (clap and tokio are stable, unlikely to change significantly)

**Files examined:**
- src/main.rs (288 lines)
- src/bin/daemon.rs (60 lines)  
- src/cli/daemon.rs (243 lines)
- src/daemon/mod.rs (463 lines)
- src/daemon/lifecycle.rs (151 lines)
- src/shutdown.rs (158 lines)
- src/config/mod.rs (270 lines)
- src/ipc/mod.rs (239 lines)
- Cargo.toml (30 lines)
- .planning/phases/05-unified-daemon/05-CONTEXT.md
