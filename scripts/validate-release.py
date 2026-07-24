#!/usr/bin/env python3
import argparse,hashlib,json, pathlib,sys
p=argparse.ArgumentParser(); p.add_argument('manifest'); p.add_argument('--verify-tree',action='store_true'); p.add_argument('--verify-receipts',action='store_true'); a=p.parse_args()
m=json.load(open(a.manifest)); base=pathlib.Path(a.manifest).parent
if m.get('manifest_version') != 2: raise SystemExit('manifest_version must be 2')
for k in ('source','toolchain','artifacts','receipts','sbom','advisory_policy'):
 if k not in m: raise SystemExit('missing '+k)
if m['source'].get('dirty'): raise SystemExit('dirty source')
def sha(p): return hashlib.sha256(open(p,'rb').read()).hexdigest()
if a.verify_tree:
 for x in m['artifacts']:
  f=base/x['path'];
  if not f.is_file() or sha(f)!=x['sha256']: raise SystemExit('artifact digest mismatch: '+str(f))
 s=base/m['sbom']['path']
 if not s.is_file() or sha(s)!=m['sbom']['sha256']: raise SystemExit('SBOM digest mismatch')
if a.verify_receipts:
 for x in m['receipts']:
  f=base/x['path']
  if x['exit_code']!=0 or not f.is_file() or sha(f)!=x['sha256']: raise SystemExit('receipt invalid: '+x['name'])
print('release manifest valid')
