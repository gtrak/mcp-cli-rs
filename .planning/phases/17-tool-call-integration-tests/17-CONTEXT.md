# Phase 17: Tool Call Integration Tests - Context

**Gathered:** 2026-02-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Create end-to-end integration tests for tool execution (TEST-01 through TEST-05). Tests must verify full MCP server communication flow including both stdio and HTTP transports. Mock servers (stdio binary and HTTP) will simulate MCP server responses. Testing scope is comprehensive — covers happy paths and error scenarios.

</domain>

<decisions>
## Implementation Decisions

### Transport Coverage
- Both stdio and HTTP transports must be tested (both exist in codebase)
- Test both transports with same scenarios (parameterized approach preferred)

### Mock Server Strategy
- **Stdio**: Binary mock launched as subprocess (like existing test fixtures)
- **HTTP**: In-process or separate — Claude's discretion based on simplicity
- Both mocks must implement enough MCP protocol to support our calls

### Test Scenarios
- **Comprehensive coverage required**: Happy path AND error cases
- Must cover: tool discovery, tool call with arguments, successful response
- Must cover: error scenarios (tool not found, invalid args, server errors, transport failures)

### Test Data
- External fixture files preferred for tool definitions and test scenarios
- Support JSON fixture files in tests/fixtures/ directory

### Mock Fidelity
- Mock must support full MCP protocol initialization (initialize handshake, notifications/initialized)
- Must support tools/list and tools/call methods
- Should support ping for health checks
- State management (tracking calls, returning different responses) — Claude's discretion

### File Structure
- Test organization by scenario or transport — Claude's discretion
- Mock binaries can live in tests/fixtures/ or as separate targets

</decisions>

<specifics>
## Specific Ideas

- Mock servers should be reusable across multiple test cases
- HTTP mock can be simpler since HTTP transport already handles process management differently
- Stdio mock should simulate real MCP server lifecycle (init → ready → respond → exit)

</specifics>

<deferred>
## Deferred Ideas

- Load testing or performance benchmarks — out of scope for integration tests
- Testing against real MCP servers (filesystem, etc.) — keep to mocks for determinism
- WebSocket transport testing — not yet implemented

</deferred>

---

*Phase: 17-tool-call-integration-tests*
*Context gathered: 2026-02-13*
