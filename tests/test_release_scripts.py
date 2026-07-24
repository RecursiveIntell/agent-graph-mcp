import json, pathlib, subprocess, sys
ROOT = pathlib.Path(__file__).parents[1]

def test_no_deleted_worktree_or_suppressed_gates():
    s = (ROOT/'scripts/build-release.sh').read_text()
    assert 'rev-parse --show-toplevel' in s
    assert 'agent-graph-remediation' not in s
    assert 'read -r response' not in s
    assert '|| true' not in s

def test_schema_and_advisory_policy():
    schema=json.loads((ROOT/'docs/release/manifest-v2.schema.json').read_text())
    assert schema['properties']['manifest_version']['const']==2
    policy=json.loads((ROOT/'docs/release/advisory-adjudication.json').read_text())
    assert 'RUSTSEC-2026-0190' in policy['advisories']

def test_validator_rejects_dirty_manifest(tmp_path):
    m={'manifest_version':2,'source':{'dirty':True}}
    p=tmp_path/'m.json'; p.write_text(json.dumps(m))
    r=subprocess.run([sys.executable,str(ROOT/'scripts/validate-release.py'),str(p)],capture_output=True,text=True)
    assert r.returncode != 0
