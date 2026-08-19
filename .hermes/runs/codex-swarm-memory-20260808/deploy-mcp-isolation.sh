#!/usr/bin/env bash
set -euo pipefail

stage=/home/sikmindz/.hermes/staging/agent-graph-mcp-isolation-20260808T153517Z
source_root=/home/sikmindz/Coding/agent-graph-mcp-release
installed=/home/sikmindz/.local/bin
launcher=/home/sikmindz/.local/bin/agent-graph-mcpd-launch-luna.sh
previous_launcher=/home/sikmindz/.hermes/staging/agent-graph-persistent-appserver-final-20260808T124014Z/agent-graph-mcpd-launch-luna.sh.previous
unit=agent-graph-mcpd.service
mkdir -p "$stage"
cp -a "$installed/agent-graph-mcpd" "$stage/agent-graph-mcpd.previous"
cp -a "$installed/agent-graph-mcp" "$stage/agent-graph-mcp.previous"
cp -a "$previous_launcher" "$stage/agent-graph-mcpd-launch-luna.sh.previous"
rollback() {
  systemctl --user stop "$unit" || true
  fuser -k "$installed/agent-graph-mcp" || true
  install -m 0755 "$stage/agent-graph-mcpd.previous" "$installed/agent-graph-mcpd"
  install -m 0755 "$stage/agent-graph-mcp.previous" "$installed/agent-graph-mcp"
  install -m 0755 "$stage/agent-graph-mcpd-launch-luna.sh.previous" "$launcher"
  systemctl --user start "$unit" || true
}
trap rollback ERR
systemctl --user stop "$unit"
fuser -k "$installed/agent-graph-mcp" || true
install -m 0755 "$source_root/target/release/agent-graph-mcpd" "$installed/agent-graph-mcpd"
install -m 0755 "$source_root/target/release/agent-graph-mcp" "$installed/agent-graph-mcp"
systemctl --user start "$unit"
systemctl --user is-active --quiet "$unit"
test "$(sha256sum "$source_root/target/release/agent-graph-mcpd" | cut -d' ' -f1)" = "$(sha256sum "$installed/agent-graph-mcpd" | cut -d' ' -f1)"
test "$(sha256sum "$source_root/target/release/agent-graph-mcp" | cut -d' ' -f1)" = "$(sha256sum "$installed/agent-graph-mcp" | cut -d' ' -f1)"
trap - ERR
