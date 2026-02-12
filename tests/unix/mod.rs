//! Unix-specific IPC tests
//!
//! Tests Unix socket communication on Linux and macOS

#[cfg(all(test, unix))]
pub mod tests;
