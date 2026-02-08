# Phase 3: Performance & Reliability - Research

**Researched:** 2026-02-08
**Domain:** Concurrent execution, retry logic, terminal output, signal handling
**Confidence:** HIGH

## Summary

Phase 3 focuses on making the MCP CLI faster and more reliable. The research identified standard Rust patterns for:

1. **Concurrent server discovery** using `futures::stream` combinators with semaphore-based concurrency limiting
2. **Automatic retry logic** with exponential backoff using the `backoff` crate or custom implementation
3. **Operation timeout enforcement** using `tokio::time` module
4. **Colored terminal output** using the `colored` crate with TTY detection and NO_COLOR support
5. **Graceful signal handling** using `tokio::signal` for cross-platform SIGINT/SIGTERM support

The recommended stack uses minimal dependencies, reusing tokio for async operations and adding only the `colored` crate for terminal output. Retry logic can be implemented with the `backoff` crate (well-maintained) or manually to avoid an extra dependency.

**Primary recommendation:** Use futures combinators for concurrency, backoff crate for retries, colored crate for output, and tokio::signal for graceful shutdown.

## User Constraints

No CONTEXT.md exists for Phase 3. All implementation choices are at Claude's discretion based on the research findings below.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.49 | Async runtime (already in use) | Provides spawn, timeout, signal, and sync primitives |
| futures | 0.3 | Stream combinators (futures-util already in use) | Provides buffer_unordered for concurrent stream processing |
| colored | 3.1 | Terminal color support | Simple API, zero conditional compilation, widely used (10M+ downloads/month) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| backoff | 0.4 | Exponential backoff retry with tokio | Recommended for retry logic (includes tokio feature) |
| std::io::IsTerminal | - | TTY detection | Use instead of atty crate (available in Rust 1.70+) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| futures::buffer_unordered | tokio::spawn with manual collection | buffer_unordered provides built-in concurrency limiting and ordered results |
| backoff crate | Custom retry implementation | backoff is battle-tested; custom implementation avoids dependency but increases maintenance |
| colored crate | termcolor or ansi_term | colored has the simplest API (method chaining) and zero conditional compilation required |
| tokio::signal | ctrlc crate | tokio::signal provides unified API for both SIGINT/SIGTERM, cross-platform |

**Installation:**
```bash
# Required additions to Cargo.toml:
[dependencies]
colored = "3.1"

# Optional additions (if using backoff crate):
backoff = { version = "0.4", features = ["tokio"] }
```

Note: `tokio`, `futures-util` are already in dependencies from Phase 1/2.

## Architecture Patterns

### Pattern 1: Concurrent Server Discovery with Semaphore

**What:** Process multiple servers in parallel with configurable concurrency limit (default 5). Uses `futures::stream::buffer_unordered` with `tokio::sync::Semaphore` to limit concurrent operations.

**When to use:** When listing tools or executing operations on multiple servers simultaneously (DISC-05).

**Source:** https://docs.rs/futures/latest/futures/stream/trait.StreamExt.html#method.buffer_unordered

**Example:**
```rust
use futures::stream::{self, StreamExt};
use tokio::sync::Semaphore;
use std::sync::Arc;
use crate::client::McpClient;
use crate::error::Result;

/// List tools from all servers in parallel with concurrency limit
pub async fn list_tools_parallel(
    config: Arc<Config>,
    concurrency_limit: usize,
) -> Result<(Vec<(String, Vec<ToolInfo>)>, Vec<String>)> {
    // Create semaphore to limit concurrent operations
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));

    // Create stream of server names
    let servers: Vec<String> = config.servers.iter().map(|s| s.name.clone()).collect();

    // Process each server with concurrency control
    let results = stream::iter(servers)
        .map(|server_name| {
            let semaphore = semaphore.clone();
            let config = config.clone();

            async move {
                // Acquire permit before starting operation
                let _permit = semaphore.acquire().await.unwrap();

                // Create client and list tools
                let transport = config.create_transport(&server_name);
                let mut client = McpClient::new(server_name.clone(), transport);

                match client.list_tools().await {
                    Ok(tools) => Ok((server_name, tools)),
                    Err(e) => {
                        tracing::warn!("Failed to list tools for {}: {}", server_name, e);
                        Err(server_name)
                    }
                }
            }
        })
        .buffer_unordered(concurrency_limit) // Limit concurrent futures
        .collect::<Vec<Result<(String, Vec<ToolInfo>)>>>()
        .await;

    // Separate successes and failures
    let mut successes = Vec::new();
    let mut failures = Vec::new();

    for result in results {
        match result {
            Ok(success) => successes.push(success),
            Err(server_name) => failures.push(server_name),
        }
    }

    // Warn if some servers failed (ERR-07)
    if !failures.is_empty() {
        eprintln!(
            "{}: Failed to connect to {} of {} servers: {}",
            "Warning".yellow(),
            failures.len(),
            successes.len() + failures.len(),
            failures.join(", ")
        );
    }

    Ok((successes, failures))
}
```

### Pattern 2:Retry Logic with Exponential Backoff

**What:** Automatically retry failed operations with exponential backoff delay. Detect transient errors (network timeouts, HTTP 502/503/504/429) and retry up to a maximum limit.

**When to use:** For tool execution or any operation that may experience transient failures (EXEC-05, EXEC-07).

**Source:** https://docs.rs/backoff/latest/backoff/

**Example using backoff crate:**
```rust
use backoff::{ExponentialBackoff, future::retry, Error as BackoffError};
use std::time::Duration;

/// Retry configuration for tool execution
#[derive(Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

/// Execute tool with automatic retry on transient errors
pub async fn execute_tool_with_retry(
    client: &mut McpClient,
    tool_name: &str,
    arguments: serde_json::Value,
    config: RetryConfig,
) -> Result<serde_json::Value> {
    let mut attempt = 0;

    retry(ExponentialBackoff {
        current_interval: Duration::from_millis(config.base_delay_ms),
        randomization_factor: 0.5,
        multiplier: 2.0,
        max_interval: Duration::from_millis(config.max_delay_ms),
        max_elapsed_time: None,
        start_time: Instant::now(),
        clock: SystemClock,
    }, || async {
        attempt += 1;

        match client.call_tool(tool_name, arguments.clone()).await {
            Ok(result) => Ok(result),
            Err(McpError::Timeout { .. }) |
            Err(McpError::ConnectionError { .. }) |
            Err(McpError::IOError { .. }) => {
                if attempt >= config.max_attempts {
                    Err(BackoffError::Permanent(McpError::Timeout {
                        timeout: config.max_attempts * config.base_delay_ms / 1000,
                    }))
                } else {
                    Err(BackoffError::transient(McpError::Timeout {
                        timeout: config.base_delay_ms / 1000,
                    }))
                }
            },
            Err(e) => Err(BackoffError::Permanent(e)),
        }
    }).await.map_err(|e| match e {
        BackoffError::Permanent(e) | BackoffError::Transient(e) => e,
        _ => McpError::Timeout { timeout: config.base_delay_ms / 1000 },
    })
}
```

### Pattern 3: Operation Timeout with Cancellation

**What:** Enforce overall operation timeout that cancels ongoing retries when time budget is exhausted.

**When to use:** To prevent operations from running indefinitely (EXEC-06).

**Source:** https://docs.rs/tokio/latest/tokio/time/fn.timeout.html

**Example:**
```rust
use tokio::time::{timeout, Duration};

/// Execute operation with overall timeout
pub async fn with_timeout<F, T>(
    operation: F,
    timeout_secs: u64,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    let duration = Duration::from_secs(timeout_secs);

    match timeout(duration, operation).await {
        Ok(result) => result,
        Err(_) => {
            tracing::error!("Operation timed out after {}s", timeout_secs);
            Err(McpError::Timeout { timeout: timeout_secs })
        },
    }
}
```

### Pattern 4: Colored Terminal Output with NO_COLOR Support

**What:** Display colored output when stdout is a TTY and NO_COLOR environment variable is not set.

**When to use:** For CLI output formatting to improve readability (ERR-04).

**Source:** https://docs.rs/colored/latest/colored/

**Example:**
```rust
use colored::Colorize;
use std::io::IsTerminal;

/// Check if colored output should be used
fn use_color() -> bool {
    // NO_COLOR environment variable takes precedence
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Only use colors if stdout is a TTY
    std::io::stdout().is_terminal()
}

/// Print error message with appropriate color
pub fn print_error(message: &str) {
    if use_color() {
        eprintln!("{}: {}", "Error".red(), message);
    } else {
        eprintln!("Error: {}", message);
    }
}
```

### Pattern 5: Graceful Signal Handling

**What:** Handle SIGINT and SIGTERM signals to perform cleanup before termination. Stop ongoing operations, close connections, and flush buffers.

**When to use:** In CLI main function to enable graceful shutdown (CLI-04).

**Source:** https://docs.rs/tokio/latest/tokio/signal/

**Example:**
```rust
use tokio::signal;

/// Run application with graceful shutdown support
pub async fn run_with_graceful_shutdown(
    config: Arc<Config>,
    connection_pool: Arc<ConnectionPool>,
) -> Result<()> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<bool>(1);

    let signal_task = tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Failed to setup SIGINT handler");
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to setup SIGTERM handler");

            tokio::select! {
                _ = sigint.recv() => println!("\nReceived SIGINT (Ctrl+C), shutting down..."),
                _ = sigterm.recv() => println!("\nReceived SIGTERM, shutting down..."),
            }
        }

        #[cfg(windows)]
        {
            if signal::ctrl_c().await.is_ok() {
                println!("\nReceived shutdown signal, shutting down...");
            }
        }

        let _ = shutdown_tx.send(true);
    });

    let result = tokio::select! {
        result = run_main_application(config, shutdown_tx.subscribe()) => result,
        _ = shutdown_rx.recv() => {
            println!("Shutting down...");
            connection_pool.clear();
            Ok(())
        },
    };

    signal_task.abort();
    result
}
```

### Anti-Patterns to Avoid

- **Blocking the executor with retry delays:** Always use `tokio::time::sleep`, not `std::thread::sleep`
- **Race conditions in concurrent execution:** Use Arc for shared state, Mutex for mutable state
- **Orphaning resources on signal:** Ensure all tasks are aborted and resources cleaned up
- **Not respecting NO_COLOR:** Always check NO_COLOR environment variable before enabling colors
- **Hard-coded concurrency limits:** Make concurrency limits configurable

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Exponential backoff with jitter | Custom retry loop with sleep | `backoff` crate | Includes jitter, proper max_elapsed_time handling, cancel-safety |
| TTY detection | Platform-specific code or atty crate | `std::io::IsTerminal` | Built into std, no dependency, cross-platform |
| Terminal colors | Manual ANSI escape code generation | `colored` crate | Handles terminal capabilities, supports NO_COLOR automatically |
| Signal handling | Platform-specific signal handling | `tokio::signal` | Unified cross-platform API, integrates with tokio |
| Concurrency limiting | Custom task limit tracking | `tokio::sync::Semaphore` + futures | Standard pattern, fairness and starvation prevention |

## Common Pitfalls

### Pitfall 1: Blocking the Async Executor

**What goes wrong:** Using `std::thread::sleep` instead of `tokio::time::sleep` blocks the entire tokio runtime.

**How to avoid:**
```rust
// WRONG - blocks the executor
std::thread::sleep(Duration::from_secs(1));

// RIGHT - async sleep that yields to other tasks
tokio::time::sleep(Duration::from_secs(1)).await;
```

### Pitfall 2: Race Conditions in Concurrent Operations

**What goes wrong:** Mutable shared state modified by concurrent tasks without synchronization.

**How to avoid:**
```rust
// WRONG - potential data race
let mut results = Vec::new();
for server in servers {
    tokio::spawn(async move {
        let tools = list_tools(server).await?;
        results.push((server, tools)); // Data race!
    });
}

// RIGHT - collect results asynchronously
let results = stream::iter(servers)
    .map(|server| async move { Ok((server, list_tools(&server).await?)) })
    .buffer_unordered(5)
    .collect::<Vec<_>>()
    .await;
```

### Pitfall 3: Not Distinguishing Transient from Permanent Errors

**What goes wrong:** Retrying errors that will never succeed wastes time and causes confusing messages.

**How to avoid:**
```rust
fn is_transient_error(error: &McpError) -> bool {
    matches!(
        error,
        McpError::Timeout { .. } |
        McpError::ConnectionError { .. } |
        McpError::IOError { .. }
    )
}

// Only retry transient errors
match result {
    Ok(value) => return Ok(value),
    Err(e) if is_transient_error(&e) && attempt < max_attempts => {
        sleep(backoff_delay).await;
    },
    Err(e) => return Err(e), // Permanent error, don't retry
}
```

### Pitfall 4: Ignoring NO_COLOR Environment Variable

**What goes wrong:** Colored output forced when user disabled it or when output is piped.

**How to avoid:**
```rust
fn use_color() -> bool {
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    std::io::stdout().is_terminal()
}
```

### Pitfall 5: Orphaned Resources on Signal

**What goes wrong:** Tasks continue running and resources aren't cleaned up on signal.

**How to avoid:**
```rust
let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<bool>(1);

// In tasks
tokio::select! {
    result = do_work() => result,
    _ = shutdown_rx.recv() => {
        cleanup();
        return Ok(());
    }
}
```

## Code Examples

Verified patterns from official sources:

### Example 1: TTY Detection
```rust
use std::io::IsTerminal;

fn main() {
    if std::io::stdout().is_terminal() {
        println!("Running in a terminal");
    } else {
        println!("Output is being piped or redirected");
    }
}
```

### Example 2: Signal Handling (Ctrl+C)
```rust
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    signal::ctrl_c().await?;
    println!("ctrl-c received!");
    Ok(())
}
```

### Example 3: Timeout with Cancellation
```rust
use tokio::time::{timeout, Duration};

async fn operation() -> Result<String> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok("Done".to_string())
}

#[tokio::main]
async fn main() {
    match timeout(Duration::from_secs(2), operation()).await {
        Ok(result) => println!("{}", result.unwrap()),
        Err(_) => println!("Operation timed out"),
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| std::thread::sleep | tokio::time::sleep | Always | Prevents blocking async executor |
| atty crate for TTY detection | std::io::IsTerminal | Rust 1.70+ | Removes dependency, better cross-platform support |
| Manual ANSI escape codes | colored crate | - | Simpler API, automatic NO_COLOR support |
| Manual signal handling | tokio::signal | - | Unified API, integrates with tokio |
| Custom retry loops | backoff crate | - | Includes jitter, cancel-safety, standard patterns |

**Deprecated/outdated:**
- **atty crate:** Use `std::io::IsTerminal` instead (available in Rust 1.70+, no dependency)
- **ansi_term crate:** The `colored` crate has a simpler, more modern API
- **ctrlc crate:** Use `tokio::signal` for unified signal handling

## Integration with Existing Codebase

### Where to add Phase 3 code:

1. **src/retry.rs** (new file): Retry logic with backoff config
2. **src/parallel.rs** (new file): Concurrent server discovery and execution
3. **src/output.rs** (new file): Colored terminal output utilities
4. **src/shutdown.rs** (new file): Signal handling and graceful shutdown
5. **src/cli/commands.rs**: Integrate new features into CLI commands

### Error type additions (src/error.rs):

```rust
#[error("Operation cancelled (timeout: {}s)", timeout)]
OperationCancelled { timeout: u64 },

#[error("Max retry attempts ({}) exceeded", attempts)]
MaxRetriesExceeded { attempts: u32 },
```

### Config additions (src/config/mod.rs):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,
    #[serde(default = "default_retry_max_delay_ms")]
    pub retry_max_delay_ms: u64,
}

fn default_max_retries() -> u32 { 3 }
fn default_retry_base_delay_ms() -> u64 { 1000 }
fn default_retry_max_delay_ms() -> u64 { 30000 }
```

## Open Questions

### None identified

All research questions have high-confidence answers:
- Concurrent processing: futures::stream::buffer_unordered ✓
- Retry logic: backoff crate (recommended) or custom ✓
- Timeout enforcement: tokio::time::timeout ✓
- Colored output: colored crate with IsTerminal ✓
- Signal handling: tokio::signal ✓

## Sources

### Primary (HIGH confidence)
- tokio 1.49 docs - spawn, timeout, signal, sync primitives
- futures 0.3 docs - StreamExt::buffer_unordered
- colored 3.1 docs - Colorize trait, API patterns
- Rust std 1.70+ - std::io::IsTerminal trait

### Secondary (MEDIUM confidence)
- backoff 0.4 docs - ExponentialBackoff, retry patterns
- NO_COLOR standard - https://no-color.org/

### Tertiary (LOW confidence)
- None (all verified with official documentation)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries verified via official documentation
- Architecture: HIGH - All patterns from official tokio/futures docs
- Pitfalls: HIGH - Based on async best practices and official guidance

**Research date:** 2026-02-08
**Valid until:** 2026-03-10 (30 days - tokio and futures ecosystem is stable)
