# Contributing to agent-graph-mcp

Thanks for your interest. This is a focused Rust project — contributions that align with the existing architecture and quality bar are welcome.

## Setup

```bash
git clone https://github.com/RecursiveIntell/agent-graph-mcp.git
cd agent-graph-mcp

# Build
cargo build

# Run the test suite
cargo test --lib
cargo test --test daemon_recovery --test mcp_integration

# Lint (must pass clean)
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo deny check
```

**Prerequisites:** Rust ≥ 1.75, an LLM endpoint for integration testing (local Ollama or any OpenAI-compatible API).

## Development workflow

1. **Open an issue first** to discuss what you want to change. This avoids wasted effort on something that doesn't fit the project's direction.
2. **Fork and branch** from `main`. Use a descriptive branch name: `fix/daemon-recovery-race` or `feat/new-template-type`.
3. **Write tests.** New features need tests. Bug fixes need a regression test that fails before the fix.
4. **Run the full gate** before pushing:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test --lib
   cargo test --test daemon_recovery --test mcp_integration
   cargo build --release
   ```
5. **Open a PR** with a clear description of what changed and why. Reference the issue number.

## What makes a good PR

- **Targeted.** One concern per PR. A bug fix shouldn't also refactor unrelated code.
- **Tested.** New or changed behavior has test coverage. Run the existing suite — don't break it.
- **Documented.** Public API changes include `///` doc comments. Architecture changes update `AGENTS.md` or the module doc comment.
- **Clean.** `cargo fmt` and `cargo clippy` pass. No warnings.
- **Small.** Prefer multiple small PRs over one large one. If a change touches more than 5 files, consider splitting it.

## Code style

This is a standard Rust 2021 project. Follow the conventions already present:

- **Error handling:** `anyhow::Result` for application code, `thiserror` for library error types.
- **Async:** All MCP handlers and daemon operations are `async`. Use Tokio.
- **Tool handlers:** Return `Result<Json<Output>, ErrorData>`. The `Output` struct carries the JSON schema required by the MCP specification.
- **No `unwrap()` or `expect()` in library code.** Use `?` or explicit error handling.
- **Derive macros** in order: `Debug, Clone, Serialize, Deserialize, JsonSchema`.
- **Module structure:** One module per concern. `lib.rs` declares modules and re-exports the public API.

## What we're looking for

- **Bug fixes** with a reproduction and a test.
- **New graph node types** with clear use cases and template integration.
- **New built-in templates** that demonstrate real workflow patterns.
- **Documentation improvements** — especially for the MCP tool contract, graph spec format, or daemon operations.
- **Performance fixes** with benchmarks showing the improvement.

## What we're not looking for

- **Speculative features** without a concrete consumer or workflow.
- **New dependencies** without a strong justification. The dependency tree is intentionally lean.
- **Large refactors** that change the module structure without a clear gain. Open an issue to discuss first.
- **Breaking daemon protocol changes.** The Unix socket framed transport is a stability boundary.
- **"AI-generated" filler** — PRs should show evidence of understanding the codebase, not just model output.

## Review process

PRs are reviewed by the maintainer ([Josh Stevenson / RecursiveIntell](https://github.com/RecursiveIntell)). Expect:

- A response within a week.
- Constructive feedback focused on correctness, safety, and fit.
- Requests for tests, documentation, or simplification if needed.

If your PR sits unreviewed for more than two weeks, ping the issue or PR thread.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE-MIT).
