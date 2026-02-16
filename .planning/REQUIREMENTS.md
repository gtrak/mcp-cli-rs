# Requirements: MCP CLI v1.7 Linux Compatibility & Documentation

**Defined:** 2026-02-16
**Core Value:** Reliable cross-platform MCP server interaction without dependencies

## v1.7 Requirements

### Linux Compatibility

- [ ] **LINUX-01**: Project compiles successfully on Linux (cargo build)
- [ ] **LINUX-02**: All library tests pass on Linux (cargo test --lib)
- [ ] **LINUX-03**: All integration tests pass on Linux (cargo test --test '*')
- [ ] **LINUX-04**: Missing nix crate dependency added for Unix signal handling
- [ ] **LINUX-05**: Windows-only exports properly gated with cfg attributes
- [ ] **LINUX-06**: IPC method signatures compatible across platforms
- [ ] **LINUX-07**: Unix socket address handling uses platform-appropriate APIs
- [ ] **LINUX-08**: Error handling covers all McpError variants exhaustively
- [ ] **LINUX-09**: windows-sys dependency is Windows-only (optional/target-specific)

### Documentation

- [ ] **DOC-01**: README.md exists at project root
- [ ] **DOC-02**: README includes installation instructions
- [ ] **DOC-03**: README includes basic usage examples
- [ ] **DOC-04**: README includes configuration guide
- [ ] **DOC-05**: README includes all command examples (list, info, call, grep)
- [ ] **DOC-06**: README includes development setup instructions
- [ ] **DOC-07**: README includes troubleshooting section

### CI/CD & Testing

- [ ] **CI-01**: GitHub Actions workflow for Linux testing
- [ ] **CI-02**: GitHub Actions workflow for Windows testing
- [ ] **CI-03**: GitHub Actions workflow for macOS testing
- [ ] **CI-04**: All workflows run on PR and push to main

## v2 Requirements (Future)

### Distribution

- **DIST-01**: crates.io publication
- **DIST-02**: Homebrew formula
- **DIST-03**: Chocolatey package
- **DIST-04**: APT repository

## Out of Scope

| Feature | Reason |
|---------|--------|
| SSE/Streamable HTTP transports | Requires significant async streaming work, defer to v2+ |
| Built-in MCP servers | Tool is client-only by design |
| GUI/Web interface | CLI-only tool, web UI would be separate project |
| Package manager integrations | Focus on binary distribution first |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| LINUX-01 | Phase 24 | Pending |
| LINUX-02 | Phase 24 | Pending |
| LINUX-03 | Phase 25 | Pending |
| LINUX-04 | Phase 24 | Pending |
| LINUX-05 | Phase 24 | Pending |
| LINUX-06 | Phase 24 | Pending |
| LINUX-07 | Phase 24 | Pending |
| LINUX-08 | Phase 24 | Pending |
| LINUX-09 | Phase 24 | Pending |
| DOC-01 | Phase 26 | Pending |
| DOC-02 | Phase 26 | Pending |
| DOC-03 | Phase 26 | Pending |
| DOC-04 | Phase 26 | Pending |
| DOC-05 | Phase 26 | Pending |
| DOC-06 | Phase 26 | Pending |
| DOC-07 | Phase 26 | Pending |
| CI-01 | Phase 27 | Pending |
| CI-02 | Phase 27 | Pending |
| CI-03 | Phase 27 | Pending |
| CI-04 | Phase 27 | Pending |

**Coverage:**
- v1.7 requirements: 20 total
- Mapped to phases: 20
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-16*
*Last updated: 2026-02-16 after milestone v1.7 definition*
