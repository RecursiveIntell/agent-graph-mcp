#!/usr/bin/env python3
"""Bounded controller-mediated Codex batch invocation with process receipts.
Not a task-truth store: caller must independently verify every changed artifact.
Each worker has a distinct systemd cgroup, workspace, timeout and output files.
"""
import argparse, concurrent.futures, hashlib, json, os, pathlib, re, shutil, subprocess, threading, time, uuid
P=pathlib.Path

def snapshot(root):
    raw=subprocess.check_output(['git','-C',str(root),'ls-files','-z','--cached','--others','--exclude-standard'])
    files={}
    for rel in raw.decode().split('\0'):
        if rel and (root/rel).is_file():files[rel]=hashlib.sha256((root/rel).read_bytes()).hexdigest()
    return files

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--manifest',required=True);ap.add_argument('--out',required=True);a=ap.parse_args();manifest=json.loads(P(a.manifest).read_text());n=manifest['max_workers'];tasks=manifest['tasks']
    if not isinstance(n,int) or isinstance(n,bool) or not 1<=n<=10:raise ValueError('worker cap must be 1..10')
    if len({t['id'] for t in tasks})!=len(tasks):raise ValueError('duplicate task identity')
    roots=[str(P(t['workspace']).resolve()) for t in tasks]
    if len(set(roots))!=len(roots):raise ValueError('each live worker requires an isolated workspace')
    for t in tasks:
        if not re.fullmatch('[a-z0-9-]{1,32}',t['id']):raise ValueError('invalid task identity')
        if t['model']!='gpt-6-luna':raise ValueError('operator model mismatch')
        if not 10<=t['timeout_seconds']<=600:raise ValueError('timeout outside admission envelope')
    codex = shutil.which('codex')
    if codex is None:raise ValueError('codex executable is required on PATH')
    worker_home = str(P.home())
    worker_path = str(P(codex).parent) + ':/usr/bin:/bin'
    cgroup_base = P('/sys/fs/cgroup/user.slice')/f'user-{os.getuid()}.slice'/f'user@{os.getuid()}.service'/'app.slice'/'app-graphcoding.slice'
    out=P(a.out);out.mkdir(parents=True,exist_ok=False);(out/'INPUT.json').write_text(json.dumps(manifest,indent=2));batch_id=uuid.uuid4().hex[:10];units={};lock=threading.Lock();samples=[];done=threading.Event()
    def observe():
        base=cgroup_base
        while not done.wait(.25):
            with lock:current=dict(units)
            row={'time':time.time(),'workers':{}}
            for task_id,unit in current.items():
                cg=base/(unit+'.service');pids=[]
                try:
                    for pid in (cg/'cgroup.procs').read_text().split():
                        try:
                            if P(os.readlink(P('/proc')/pid/'exe')).name=='codex':pids.append(int(pid))
                        except OSError:pass
                    row['workers'][task_id]={'native_codex_pids':pids,'memory_current':int((cg/'memory.current').read_text())}
                except OSError:pass
            samples.append(row)
    sampler=threading.Thread(target=observe,daemon=True);sampler.start()
    def run(t):
        root=P(t['workspace']).resolve();td=out/t['id'];td.mkdir();before=snapshot(root);unit='graph-code-'+batch_id+'-'+t['id'];prompt=P(t['prompt']).read_bytes();record={'task_id':t['id'],'model':t['model'],'workspace':str(root),'unit':unit,'prompt_sha256':hashlib.sha256(prompt).hexdigest(),'accepted':False,'acceptance_state':'requires_controller_verification','started_at':time.time()}
        cmd=['/usr/bin/systemd-run','--user','--wait','--pipe','--collect','--slice=app-graphcoding.slice','--unit='+unit,'--property=MemoryMax=768M','--property=RuntimeMaxSec='+str(t['timeout_seconds']),'--property=TimeoutStopSec=5','--property=KillMode=control-group','--property=WorkingDirectory='+str(root),'/usr/bin/env','-i','HOME='+worker_home,'PATH='+worker_path,'LANG=C.UTF-8',codex,'-a','never','exec','--ignore-user-config','--sandbox','workspace-write','--ephemeral','--json','--model',t['model'],'-c','model_reasoning_effort="high"','-C',str(root),'-']
        with lock:units[t['id']]=unit
        try:
            with (td/'stdout.jsonl').open('wb') as stdout,(td/'stderr.log').open('wb') as stderr:
                proc=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=stdout,stderr=stderr)
                try:proc.communicate(prompt,timeout=t['timeout_seconds']+30)
                except subprocess.TimeoutExpired:
                    subprocess.run(['/usr/bin/systemctl','--user','stop',unit+'.service'],capture_output=True,timeout=15)
                    proc.wait(timeout=15);record['outer_timeout']=True
                record['exit_code']=proc.returncode
            after=snapshot(root);changed=sorted(k for k in before.keys()|after.keys() if before.get(k)!=after.get(k));record['changed_files']=[{'path':k,'before':before.get(k),'after':after.get(k)} for k in changed];record['scope_violations']=[k for k in changed if k not in t['allowed_files']]
            messages=[];commands=[]
            for line in (td/'stdout.jsonl').read_text(errors='replace').splitlines():
                try:event=json.loads(line)
                except ValueError:continue
                item=event.get('item',{})
                if event.get('type')=='item.completed' and item.get('type')=='agent_message':messages.append(item.get('text',''))
                if event.get('type')=='item.completed' and item.get('type')=='command_execution':commands.append({'command':item.get('command'),'exit_code':item.get('exit_code')})
            (td/'report.txt').write_text(messages[-1] if messages else 'NO_FINAL_REPORT');record['commands']=commands
            record['process_outcome']='returned' if record['exit_code']==0 and not record['scope_violations'] and messages else 'failed_or_partial'
        except Exception as e:
            record['process_outcome']='controller_error';record['error_type']=type(e).__name__
        finally:
            subprocess.run(['/usr/bin/systemctl','--user','stop',unit+'.service'],capture_output=True,timeout=15)
            cg=cgroup_base/(unit+'.service')
            record['cleanup_complete']=not cg.exists() or not (cg/'cgroup.procs').read_text().strip()
            record['finished_at']=time.time();(td/'RECEIPT.json').write_text(json.dumps(record,indent=2));print(json.dumps({k:record.get(k) for k in ['task_id','process_outcome','exit_code','scope_violations','cleanup_complete']}),flush=True)
            with lock:units.pop(t['id'],None)
        return record
    results=[]
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=n) as pool:
            futures=[pool.submit(run,t) for t in tasks]
            for f in concurrent.futures.as_completed(futures):results.append(f.result())
    finally:
        done.set();sampler.join(timeout=2);(out/'SAMPLES.json').write_text(json.dumps(samples,indent=2))
        receipt={'schema':'AresCodingBatchExecutionV1','model':'gpt-6-luna','requested_cap':n,'results':sorted(results,key=lambda r:r['task_id']),'maximum_overlapping_coding_workers':max((sum(bool(x['native_codex_pids']) for x in s['workers'].values()) for s in samples),default=0),'accepted_implementations':None,'acceptance_state':'controller_verification_required'}
        (out/'BATCH_RECEIPT.json').write_text(json.dumps(receipt,indent=2));print(json.dumps({k:v for k,v in receipt.items() if k!='results'}))
if __name__=='__main__':main()
