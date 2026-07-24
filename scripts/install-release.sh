#!/usr/bin/env bash
set -euo pipefail
MANIFEST=${1:?manifest path}; DEST=${2:-"$HOME/.local/libexec/agent-graph"}
python3 "$(dirname "$0")/validate-release.py" "$MANIFEST" --verify-tree --verify-receipts
base=$(cd "$(dirname "$MANIFEST")" && pwd); stage=$(mktemp -d "${DEST}.stage.XXXXXX"); trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage" "$DEST"
python3 - "$MANIFEST" "$base" "$stage" <<'PY'
import json,os,shutil,sys
m=json.load(open(sys.argv[1])); base,stage=sys.argv[2:]
for a in m['artifacts']:
 src=os.path.join(base,a['path']); dst=os.path.join(stage,os.path.basename(a['path'])); shutil.copy2(src,dst); os.chmod(dst,int(a['mode'],8))
PY
for f in "$stage"/*; do mv -f "$f" "$DEST/$(basename "$f")"; done
printf '{"manifest":"%s","destination":"%s","verified":true}\n' "$MANIFEST" "$DEST/install-receipt.json" > "$DEST/install-receipt.json"
