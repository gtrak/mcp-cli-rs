---
phase: 27
name: CI/CD Setup
created: 2026-02-16
---

# Phase 27: CI/CD Setup

## Goal
Set up automated testing across Linux, Windows, and macOS using GitHub Actions

## Requirements (from ROADMAP.md)
- CI-01: GitHub Actions workflow for Linux testing
- CI-02: GitHub Actions workflow for Windows testing  
- CI-03: GitHub Actions workflow for macOS testing
- CI-04: All workflows run on PR and push to main

## Decisions (Locked)

### CI Platform
- **GitHub Actions** — Use GitHub Actions for CI/CD
  - Rationale: Native integration with GitHub, free for public repos, good cross-platform support

### Testing Strategy
- **Matrix builds** — Use GitHub Actions matrix for multiple platforms
- **Rust toolchain** — Use dtolnay/rust-cache for efficient caching
- **Test commands:**
  - `cargo test --lib` for library tests
  - `cargo test --test '*'` for integration tests
  - `cargo build --release` for release builds
  - `cargo clippy` for linting
  - `cargo fmt --check` for formatting

### Workflow Triggers
- **Triggers:** Push to main, pull requests to main
- **No manual triggers needed** — Standard push/PR triggers are sufficient

## Claude's Discretion

### Workflow Organization
- Single workflow file with matrix builds OR separate workflow files
- Recommendation: Single matrix workflow is cleaner, but separate files allow platform-specific customization
- Decision: Start with single matrix workflow, split if needed for platform-specific needs

### Rust Version Strategy
- Use `stable` toolchain by default
- Can add `nightly` or specific versions later if needed
- Decision: Stick with stable for reliability

### Artifact Strategy
- No artifact uploads needed for basic testing
- Can add binary uploads later for releases
- Decision: No artifacts for this phase

### Cache Strategy
- Use dtolnay/rust-cache for Cargo cache
- Consider cross-compilation cache if needed
- Decision: Standard rust-cache should be sufficient

## Deferred Ideas (Out of Scope)

| Idea | Why Deferred |
|------|--------------|
| Release automation | Only testing needed now, releases can be manual |
| Artifact uploads | Not needed for basic CI |
| Code coverage reporting | Can add with codecov later |
| Documentation site deployment | Not needed for CLI tool |
| Benchmark tracking | Performance testing out of scope |

## Constraints

### Platform-Specific Considerations
- **Linux:** Full test suite (all tests pass)
- **Windows:** Full test suite (tests verified in Phase 25)
- **macOS:** Test suite (similar to Linux, should pass)

### Time Constraints
- CI runs should complete in < 10 minutes per platform
- Caching is essential for reasonable run times

## Success Criteria
1. CI runs on every PR and push to main
2. All 3 platforms (Linux, Windows, macOS) have passing workflows
3. Library tests pass on all platforms
4. Integration tests pass on all platforms
5. Build succeeds on all platforms
