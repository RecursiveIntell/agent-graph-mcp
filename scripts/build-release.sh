#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
PKG=agent-graph-mcp
OUT="${RELEASE_DIR:-$ROOT/dist/agent-graph-mcp/$(sed -n 's/^version = "\([^"]*\)"/\1/p' agent-graph-mcp/Cargo.toml | head -1)/$(git rev-parse HEAD)/$(rustc -vV | sed -n 's/^host: //p')}"
mkdir -p "$OUT/receipts" "$OUT/artifacts"
if [[ -n "$(git status --porcelain)" ]]; then echo 'release requires a clean source tree' >&2; exit 2; fi
LOCK="$ROOT/Cargo.lock"; [[ -f "$LOCK" ]] || { echo 'root Cargo.lock required' >&2; exit 2; }
COMMIT=$(git rev-parse HEAD); LOCK_SHA=$(sha256sum "$LOCK"|cut -d' ' -f1); START=$(date -u +%Y-%m-%dT%H:%M:%SZ)
run_gate() { local n="$1"; shift; set +e; "$@" >"$OUT/receipts/$n.log" 2>&1; local rc=$?; set -e; python3 - "$OUT/receipts/$n.log" "$rc" "$n" <<'PY'
import hashlib,json,sys
p,rc,n=sys.argv[1],int(sys.argv[2]),sys.argv[3]
print(json.dumps({'name':n,'command':sys.argv[3:],'exit_code':rc,'output_sha256':hashlib.sha256(open(p,'rb').read()).hexdigest()}))
PY
[[ $rc -eq 0 ]] || { echo "gate failed: $n" >&2; cat "$OUT/receipts/$n.log" >&2; exit $rc; }; }
run_gate fmt cargo fmt --check -p "$PKG"
run_gate clippy cargo clippy -p "$PKG" --all-targets -- -D warnings
run_gate test cargo test -p "$PKG" --no-fail-fast
command -v cargo-audit >/dev/null || { echo 'cargo-audit is required' >&2; exit 2; }
run_gate audit cargo audit --json
run_gate build cargo build --release --locked -p "$PKG" --bins
for pair in 'proxy:agent-graph-mcp' 'daemon:agent-graph-mcpd'; do role=${pair%%:*}; bin=${pair##*:}; src="$ROOT/target/release/$bin"; [[ -x "$src" ]] || { echo "missing $bin" >&2; exit 2; }; cp "$src" "$OUT/artifacts/$role"; done
python3 "$ROOT/agent-graph-mcp/scripts/validate-advisories.py" "$OUT/receipts/audit.log"
python3 - "$OUT" "$COMMIT" "$LOCK_SHA" "$START" <<'PY'
import hashlib,json,os,subprocess,sys
out,commit,lock,start=sys.argv[1:]
def digest(p):
 h=hashlib.sha256(); s=os.stat(p); h.update(open(p,'rb').read()); return {'path':os.path.relpath(p,out),'sha256':h.hexdigest(),'size_bytes':s.st_size,'mode':oct(s.st_mode&0o777)}
arts=[digest(os.path.join(out,'artifacts',x)) for x in os.listdir(os.path.join(out,'artifacts'))]
sbom={'bomFormat':'CycloneDX','specVersion':'1.5','version':1,'components':[]}
open(os.path.join(out,'sbom.cdx.json'),'w').write(json.dumps(sbom,indent=2)+'\n')
manifest={'manifest_version':2,'source':{'commit':commit,'branch':subprocess.check_output(['git','branch','--show-current'],text=True).strip(),'dirty':False,'root':subprocess.check_output(['git','rev-parse','--show-toplevel'],text=True).strip(),'lockfiles':[{'path':'Cargo.lock','sha256':lock}]},'toolchain':{'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'cargo':subprocess.check_output(['cargo','--version'],text=True).strip(),'target':subprocess.check_output(['rustc','-vV'],text=True).split('host: ')[1].splitlines()[0]},'artifacts':arts,'receipts':[],'sbom':{'path':'sbom.cdx.json','sha256':digest(os.path.join(out,'sbom.cdx.json'))['sha256']},'advisory_policy':{'path':'../../../../agent-graph-mcp/docs/release/advisory-adjudication.json'},'build':{'started_at':start,'ended_at':__import__('datetime').datetime.now(__import__('datetime').timezone.utc).isoformat()}}
for n in ('fmt','clippy','test','audit','build'):
 p=os.path.join(out,'receipts',n+'.log'); manifest['receipts'].append({'name':n,'path':'receipts/'+n+'.log','sha256':hashlib.sha256(open(p,'rb').read()).hexdigest(),'exit_code':0})
open(os.path.join(out,'build-manifest.json'),'w').write(json.dumps(manifest,indent=2)+'\n')
PY
python3 "$ROOT/agent-graph-mcp/scripts/validate-release.py" "$OUT/build-manifest.json" --verify-tree
printf '%s\n' "$OUT/build-manifest.json"
