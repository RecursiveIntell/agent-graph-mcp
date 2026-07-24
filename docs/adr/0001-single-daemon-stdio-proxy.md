# ADR 0001: Single daemon with stdio proxies

- **Status:** Accepted
- **Scope:** durable Agent Graph MCP operation

## Decision

A durable data directory has exactly one long-lived daemon. The daemon acquires an OS-released exclusive lock before opening SQLite, owns the store, run manager, recovery, and Unix listener, and is the only process allowed to mutate durable state. Each MCP invocation is a thin stdio proxy which forwards framed JSON-RPC to that daemon.

The socket is a private Unix-domain socket below `$XDG_RUNTIME_DIR` (with a per-user fallback only when the runtime directory is explicitly configured by the launcher). Its containing directory is `0700` and socket is `0600`; the daemon verifies the peer UID and rejects other users. TCP, abstract sockets, and network listeners are unsupported. There is no ephemeral or in-memory fallback when a durable daemon is absent.

## Protocol and failure boundaries

Frames have a bounded length (default 1 MiB; implementations must reject larger frames before allocation). The proxy applies bounded write/read backpressure and propagates disconnects. Protocol JSON-RPC is the only data written to proxy stdout; diagnostics, startup failures, and tracing go to stderr. The daemon stops accepting new work during shutdown, cancels active work, waits for a bounded grace period, commits or rolls back transactions, marks its instance stopped, and releases the lock.

A second daemon for the same data directory fails with the typed `DATA_DIR_ALREADY_OWNED` error before database open or migration. A proxy that cannot connect returns a typed daemon-unavailable startup error. Neither condition silently creates a new database or ephemeral store.

## Consequences

This topology prevents proxy startup from running global recovery and makes ownership explicit through `server_instances` and `executions.owner_instance_id`. It requires a daemon lifecycle supervisor and a local runtime directory, but provides safe concurrent MCP clients and auditable shutdown/restart semantics.
