#!/usr/bin/env bash
# Prompt-only workers retain Codex-owned authentication. Discovery must use
# the same plugin/app suppression as the Rust Codex worker adapter.
set -euo pipefail

export AGENT_GRAPH_CODEX_MAX_PROCESSES="${AGENT_GRAPH_CODEX_MAX_PROCESSES:-9}"
# The daemon starts workers with the bare command `codex`. Ensure discovery
# cannot silently use a different executable or succeed when workers cannot run.
worker_codex="$(command -v codex)" || { printf '%s\n' 'codex is not on PATH' >&2; exit 1; }
discovery_codex="$(command -v "${AGENT_GRAPH_CODEX_COMMAND:-codex}")" || { printf '%s\n' 'discovery codex is unavailable' >&2; exit 1; }
python3 -c 'import os,sys; sys.exit(0 if os.path.samefile(sys.argv[1], sys.argv[2]) else 1)' "$worker_codex" "$discovery_codex" || {
  printf '%s\n' 'discovery and worker codex executables differ' >&2; exit 1;
}
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
# Match the daemon's parser, including its bound, even for explicit overrides.
printf '%s' "$AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON" | python3 -c '
import json, re, sys
names = json.load(sys.stdin)
if not isinstance(names, list) or len(names) > 128 or any(
    not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", name)
    for name in names
):
    raise SystemExit("invalid disabled MCP server list")
'
export AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON
exec "${AGENT_GRAPH_DAEMON_COMMAND:-agent-graph-mcpd}" \
  --base-url "codex-app-server://" \
  --model "${AGENT_GRAPH_MODEL:-gpt-5.6-terra}" \
  "$@"
