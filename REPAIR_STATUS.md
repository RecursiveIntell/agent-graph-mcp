# Graph repair preservation checkpoint

This branch preserves the daemon connection-lifecycle and bounded-result repair,
controller read/submission helpers and regression coverage. It is not a public
release or a native Graph-to-coding-agent dispatcher.

The private controller qualification recorded 286 passing Rust tests, 8 release
lifecycle tests and 59 release-pair read-path tests. These selections overlap and
must not be summed. The Rust bytes are preserved from that qualified candidate;
the full Rust suite has not been rerun for this preservation checkpoint.

Fresh local checks after removing machine-specific script paths:

- 27 client/controller unit tests passed.
- 59 read-path/SDK tests passed against the previously qualified release binaries.
- 4 hermetic launcher tests passed, including discovery failure and unsafe-name
  rejection without daemon launch.
- Shell syntax and Git whitespace checks passed.

The new portable launcher retains plugin/app suppression during MCP discovery.
Assignment is separate from `export` so a failed discovery cannot be masked by
`export` returning success. The test fixture makes no real provider calls.

`run_coding_batch.py` now resolves Codex from PATH and uses the current home/UID
instead of one machine's hard-coded paths. The earlier multi-worker qualification
belongs to the previous wrapper bytes; no fresh real coding batch was run for
this portability change. The helper remains Linux/systemd-specific and its
allowed-file checks are post-execution contracts, not OS per-file enforcement.

`run_codex_phase.sh` is retained to make its paired fixture tests self-contained.
It is a legacy wrapper without descendant/cgroup cleanup and must not be used to
certify batch containment.

Retained limits: provider cancellation is best-effort; restart interruption is
not generic resumability; rejected input can leave a registered definition;
unknown start acknowledgement is quarantined rather than blindly resubmitted;
Graph output does not authorize or accept coding changes. No speedup or
unattended-autonomy claim is supported.

Private logs, machine configuration, raw provider output, credentials and
operational receipts remain outside Git. This branch is intended for a draft
preservation PR, with hosted CI and publication review separately recorded.
