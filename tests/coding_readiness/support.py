"""Shared private-daemon harness for coding-lane acceptance tests.
Fixtures are deterministic state-transform graphs: no provider calls or live DB.
"""
import asyncio, contextlib, json, os, pathlib, signal, socket, sqlite3, subprocess, tempfile, time, uuid
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

_ROOT = pathlib.Path(__file__).resolve().parents[2]
_TARGET = pathlib.Path(os.environ.get('CARGO_TARGET_DIR', _ROOT / 'target'))
if not _TARGET.is_absolute():
    _TARGET = _ROOT / _TARGET
DAEMON=os.environ.get('AGENT_GRAPH_TEST_BINARY',str(_TARGET/'debug'/'agent-graph-mcpd'))
PROXY=os.environ.get('AGENT_GRAPH_TEST_PROXY',str(_TARGET/'debug'/'agent-graph-mcp'))
class GraphFixture:
    def __init__(self,keyed=True):
        self.temp=tempfile.TemporaryDirectory(prefix='ag-code-');self.root=pathlib.Path(self.temp.name);self.store=self.root/'store';self.socket=str(self.root/'m.sock');self.key=self.root/'key';self.key.write_bytes(os.urandom(32));self.key.chmod(0o600);self.keyed=keyed;self.proc=None
    def start(self):
        env={'HOME':str(self.root),'PATH':'/usr/bin:/bin','RUST_LOG':'warn'}
        if self.keyed:env['AGENT_GRAPH_INTEGRITY_KEY_PATH']=str(self.key)
        self.log=(self.root/'daemon.log').open('ab')
        self.proc=subprocess.Popen([DAEMON,'--data-dir',str(self.store),'--socket',self.socket,'--base-url','http://127.0.0.1:9','--model','fixture-no-provider'],env=env,stdout=self.log,stderr=self.log,start_new_session=True)
        end=time.monotonic()+15
        while time.monotonic()<end:
            if self.proc.poll() is not None:raise RuntimeError('fixture daemon exited')
            sock=socket.socket(socket.AF_UNIX)
            try:sock.connect(self.socket);return self
            except OSError:time.sleep(.02)
            finally:sock.close()
        raise TimeoutError('fixture daemon readiness')
    def stop(self):
        if self.proc:
            try:os.killpg(self.proc.pid,signal.SIGTERM)
            except ProcessLookupError:pass
            try:self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:os.killpg(self.proc.pid,signal.SIGKILL);self.proc.wait(timeout=3)
            self.log.close();self.proc=None
    def restart(self):self.stop();return self.start()
    def __enter__(self):return self.start()
    def __exit__(self,*exc):self.stop();self.temp.cleanup()
    @contextlib.asynccontextmanager
    async def session(self):
        params=StdioServerParameters(command=PROXY,args=['--socket',self.socket,'--connect-timeout-ms','5000'],env={'HOME':str(self.root),'PATH':'/usr/bin:/bin'})
        async with stdio_client(params) as (read,write):
            async with ClientSession(read,write,read_timeout_seconds=10) as session:
                await session.initialize();yield session
    async def raw_async(self,session,tool,args):
        result=await session.call_tool(tool,args);raw=result.model_dump(mode='json',exclude_none=True)
        return raw.get('structured_content') or raw.get('structuredContent') or json.loads(next(x['text'] for x in raw['content'] if x.get('type')=='text'))
    def raw(self,tool,args):
        async def go():
            async with self.session() as session:return await self.raw_async(session,tool,args)
        return asyncio.run(go())
    def call(self,tool,args):
        r=self.raw(tool,args)
        if not r.get('ok'):raise AssertionError(r)
        return r['data']
    def run(self,value,*,copies=1,budgets=None):
        name='fixture-'+uuid.uuid4().hex
        nodes=[{'id':f'n{i}','type':'state_transform','config':{'operations':[{'op':'copy','from':'__input__','path':f'value{i}'}]}} for i in range(copies)]
        spec={'name':name,'entry':'n0','output_key':f'value{copies-1}','max_iterations':copies+2,'nodes':nodes,'edges':[{'from':f'n{i}','to':f'n{i+1}' if i+1<copies else 'END'} for i in range(copies)]}
        self.call('graph_create',{'spec':spec})
        raw=self.raw('graph_run_start',{'graph_id':name,'input':value,'budgets':budgets or {'max_nodes':copies+2,'max_wall_clock_ms':10000}})
        assert raw['ok'],raw
        rid=raw['run_id'];end=time.monotonic()+15
        while time.monotonic()<end:
            status=self.call('graph_run_get',{'run_id':rid,'compact':True})
            if status['status'] in ['completed','failed','cancelled'] and status.get('persistence_status')=='durable_terminal':return rid
            # Keyless fixtures intentionally cannot produce verified pages.
            if not self.keyed and status['status'] in ['completed','failed','cancelled']:return rid
            time.sleep(.02)
        raise TimeoutError('fixture run did not settle')
    def db(self):return sqlite3.connect(self.store/'agent-graph.db')
    def pages(self,rid,artifact='output',limit=16384):
        offset=0;first=None;result=[]
        for _ in range(4096):
            args={'run_id':rid,'artifact':artifact,'offset':offset,'limit':limit}
            if first:args.update(expected_digest=first['digest'],expected_artifact_id=first['artifact_id'])
            p=self.call('graph_run_artifact',args);first=first or p;result.append(p)
            if p['done']:return result
            assert p['next_offset']>offset;offset=p['next_offset']
        raise AssertionError('fixture page count exceeded')
