#!/usr/bin/env bash
set -euo pipefail

stage=/home/sikmindz/.hermes/staging/agent-graph-thread-cleanup-20260808T152146Z
source_root=/home/sikmindz/Coding/agent-graph-mcp-release
installed=/home/sikmindz/.local/bin

mkdir -p "$stage"
cp "$installed/agent-graph-mcpd" "$stage/agent-graph-mcpd.old"
cp "$installed/agent-graph-mcp" "$stage/agent-graph-mcp.old"
sha256sum "$stage/agent-graph-mcpd.old" "$stage/agent-graph-mcp.old" > "$stage/old.sha256"

rollback() {
  systemctl --user stop agent-graph-mcpd.service || true
  fuser -k "$installed/agent-graph-mcp" >/dev/null 2>&1 || true
  install -m 0755 "$stage/agent-graph-mcpd.old" "$installed/agent-graph-mcpd"
  install -m 0755 "$stage/agent-graph-mcp.old" "$installed/agent-graph-mcp"
  systemctl --user start agent-graph-mcpd.service || true
}
trap rollback ERR

systemctl --user stop agent-graph-mcpd.service
fuser -k "$installed/agent-graph-mcp" >/dev/null 2>&1 || true
install -m 0755 "$source_root/target/release/agent-graph-mcpd" "$installed/agent-graph-mcpd"
install -m 0755 "$source_root/target/release/agent-graph-mcp" "$installed/agent-graph-mcp"
systemctl --user start agent-graph-mcpd.service
systemctl --user is-active --quiet agent-graph-mcpd.service
sha256sum "$source_root/target/release/agent-graph-mcpd" "$installed/agent-graph-mcpd"
sha256sum "$source_root/target/release/agent-graph-mcp" "$installed/agent-graph-mcp"

trap - ERR
