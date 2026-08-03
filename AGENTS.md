# AGENTS.md — agent-graph-mcp

Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, etc.) working on this repository.

## Project identity

`agent-graph-mcp` is an MCP server that exposes the `ri-agent-graph` runtime engine as 25 typed tools. It compiles declarative JSON workflow specs, executes LLM graphs with parallel fan-out, checkpoint/resume, human-in-the-loop approvals, source witnessing, and HMAC-authenticated receipts.

**Stack:** Rust (edition 2021, MSRV 1.75), Tokio async, rmcp, SQLite.

## Build, test, lint

```bash
cargo build                    # debug build
cargo build --release          # release binary
cargo test --lib               # 58 lib tests (1 known fixture-path failure)
cargo test --test daemon_recovery --test mcp_integration  # integration tests
cargo fmt --check              # formatting
cargo clippy --all-targets -- -D warnings  # lint (must pass clean)
cargo deny check               # dependency auditing
```

The binary installs to `~/.cargo/bin/agent-graph-mcp`. There is also a daemon binary (`agent-graph-mcpd`) for persistent multi-client mode.

## Project structure

```
src/
├── main.rs              # CLI entry point (direct mode, daemon client)
├── cli.rs               # Argument parsing
├── server.rs            # MCP tool router (25 tools)
├── tools.rs             # Tool parameter types + JSON schemas
├── daemon.rs             # Daemon process (agent-graph-mcpd)
├── run_manager.rs       # Graph execution lifecycle
├── store.rs             # SQLite persistence
├── migrations.rs        # Schema migrations
├── compiler.rs          # JSON spec → executable graph compilation
├── spec.rs              # Graph spec types
├── nodes.rs             # Node type definitions (LLM, join, passthrough, etc.)
├── lifecycle.rs         # Create, validate, delete graph operations
├── templates.rs         # Built-in templates (council_deliberation, etc.)
├── evidence.rs          # Source witnessing, HMAC receipts
├── policy.rs            # Graph execution policy checks
├── promotion.rs         # Template promotion to built-in status
├── transport.rs         # Daemon transport layer (Unix socket)
├── proxy.rs             # MCP proxy between client and daemon
├── owner_lock.rs        # Single-owner daemon lock
├── operator.rs          # Operator IPC
├── operator_auth.rs     # Operator authentication
├── operator_ipc.rs      # Operator IPC protocol
├── auth.rs              # Client authentication
├── fs_security.rs       # Filesystem security controls
├── codex_app_server.rs  # Codex app server integration
└── lib.rs               # Module declarations + re-exports
tests/
├── daemon_recovery.rs   # Daemon crash recovery tests
├── mcp_integration.rs   # MCP protocol integration tests
├── lifecycle.rs         # Graph lifecycle tests
├── operator_authority.rs # Operator permission tests
├── migrations.rs        # Schema migration tests
├── template_promotion.rs # Template promotion tests
└── ...                  # Additional integration tests
```

## Coding conventions

- **No `unwrap()` or `expect()` in library code.** Use `anyhow::Result` or `thiserror` for error handling.
- **All public items need `///` doc comments.**
- **Tool handlers return `Result<Json<Output>, ErrorData>`** — the `Output` struct carries the JSON schema required by MCP spec.
- **Tests go in `#[cfg(test)] mod tests` at file bottom** (unit) or in `tests/` (integration).
- **Do not add new dependencies without a clear reason.** Prefer extending the existing stack.
- **Schema migrations** go in `src/migrations.rs` with versioned migration functions.
- **Graph spec validation** happens at `graph_create` time — invalid specs are rejected before execution.

## What NOT to do

- **Do not fabricate tool capabilities.** Tools must match actual rmcp `#[tool]` handlers.
- **Do not add speculative features.** New tools, node types, or templates need a concrete consumer.
- **Do not break the daemon protocol.** The Unix socket framed transport between proxy and daemon is a stability boundary.
- **Do not modify SQLite schema without a versioned migration.** Schema changes must be backward-compatible or gated behind a migration.
- **Do not expose internal errors to MCP clients.** Tool errors should be descriptive but must not leak stack traces or internal state.
- **Do not merge PRs with failing tests.** The known `evidence::tests::witness_dependencies_verify_sqlite_content_and_span` failure is tracked but should not be joined by new failures.

## Security boundaries

- **HMAC receipts** in `src/evidence.rs` use SHA-256 HMAC for content authentication. Do not weaken or bypass.
- **Daemon authentication** via Unix socket peer credentials (`src/auth.rs`). Do not add unauthenticated TCP listeners.
- **Operator IPC** requires explicit authorization (`src/operator_auth.rs`). Never skip operator permission checks.
- **Source witnessing** captures caller-supplied content with HMAC verification. Never weaken the authentication tag check.
- **Dependencies** are audited via `cargo deny`. New dependencies must pass advisory, ban, license, and source checks.

## Publication

- **crates.io:** `cargo publish -p agent-graph-mcp`
- **npm:** `npm publish` (package includes prebuilt binaries)
- Version bumps follow the existing `Cargo.toml` version. Update both crates.io and npm on release.

## License

MIT. All contributions are under the same license.
