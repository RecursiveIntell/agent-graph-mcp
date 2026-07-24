# Build Manifest Schema

The build manifest binds a release artifact to its source provenance.

## Required fields

```json
{
  "manifest_version": 1,
  "build_timestamp": "ISO-8601",
  "source": {
    "git_commit": "40-char hex SHA",
    "git_dirty": false,
    "git_branch": "branch name",
    "cargo_lock_sha256": "SHA-256 of Cargo.lock",
    "workspace_root": "/path/to/workspace"
  },
  "toolchain": {
    "rustc_version": "rustc X.Y.Z",
    "cargo_version": "cargo X.Y.Z",
    "target": "x86_64-unknown-linux-gnu"
  },
  "artifact": {
    "name": "agent-graph-mcp",
    "path": "target/release/agent-graph-mcp",
    "sha256": "SHA-256 of the built binary",
    "size_bytes": 12345678
  },
  "features": {
    "crate_features": [],
    "hash_algorithm_version": 1,
    "checkpoint_schema_version": 2,
    "graph_spec_version": "2",
    "migration_version": 1
  },
  "test_receipts": {
    "engine_tests": "path/to/engine-test-output",
    "mcp_tests": "path/to/mcp-test-output",
    "clippy": "path/to/clippy-output",
    "fmt_check": "path/to/fmt-output",
    "cargo_audit": "path/to/audit-output"
  }
}
```

## Validation rules

1. `git_dirty` must be `false` for release builds
2. `sha256` must match the installed binary
3. `cargo_lock_sha256` must match the workspace lockfile
4. All test receipt paths must exist and show passing results
5. `manifest_version` is required for forward compatibility
6. No secret material (keys, tokens, passwords) may appear in the manifest