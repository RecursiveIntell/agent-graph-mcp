#!/usr/bin/env bash
# Prompt-only workers retain Codex-owned authentication. Discovery must use
# the same plugin/app suppression as the Rust Codex worker adapter.
set -euo pipefail

export AGENT_GRAPH_CODEX_MAX_PROCESSES="${AGENT_GRAPH_CODEX_MAX_PROCESSES:-9}"
if [[ -z "${AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON+x}" ]]; then
  AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON="$(
    "${AGENT_GRAPH_CODEX_COMMAND:-codex}" mcp list --json \
      -c features.plugins=false -c features.apps=false | python3 -c '
import json
import re
import sys
servers = json.load(sys.stdin)
names = sorted({item["name"] for item in servers if item.get("enabled", True)})
if any(not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", name) for name in names):
    raise SystemExit("unsafe Codex MCP server name")
print(json.dumps(names, separators=(",", ":")))
'
  )"
  export AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON
fi
exec "${AGENT_GRAPH_DAEMON_COMMAND:-agent-graph-mcpd}" \
  --base-url "codex-app-server://" \
  --model "${AGENT_GRAPH_MODEL:-gpt-5.6-terra}" \
  "$@"
