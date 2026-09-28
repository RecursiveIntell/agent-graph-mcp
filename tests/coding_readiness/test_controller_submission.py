"""Real native controller submission and lost-ACK witnesses; fixture stores only."""
import asyncio
import json
import os
import pathlib
import subprocess
import sys
import unittest
from support import GraphFixture, PROXY
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / 'scripts'))
from graph_control_client import submit_once, SubmissionUnknown


def manifest():
    return {'spec':{'name':'controlled-fixture','entry':'n','output_key':'answer','nodes':[{'id':'n','type':'state_transform','config':{'operations':[{'op':'copy','from':'__input__','path':'answer'}]}}],'edges':[{'from':'n','to':'END'}]},'input':{'marker':'controller-fixture'},'budgets':{'max_nodes':2,'max_llm_calls':1,'max_wall_clock_ms':10000}}


class NativeController(unittest.TestCase):
    def test_cli_uses_live_schema_and_collects_same_run_artifacts(self):
        with GraphFixture() as f:
            m=manifest();path=f.root/'manifest.json';path.write_text(json.dumps(m))
            digest=f.call('graph_status',{'resource':'server'})['build']['source_content_sha256']
            script=pathlib.Path(__file__).resolve().parents[2]/'scripts/graph_control_client.py'
            out=f.root/'submission'
            result=subprocess.run([sys.executable,str(script),'--manifest',str(path),'--out',str(out),'--socket',f.socket,'--proxy',PROXY,'--expected-build',digest,'--idempotency-key','controller-fixture','--submit'],env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1'},capture_output=True,text=True,timeout=30)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)
            self.assertEqual(json.loads((out/'output.json').read_bytes()),m['input'])
            status=json.loads((out/'TERMINAL_STATUS.json').read_bytes())
            started=json.loads((out/'START_RESPONSE.json').read_bytes())
            self.assertEqual(status['run_id'],started['run_id'])
            self.assertEqual(status['persistence_status'],'durable_terminal')
            self.assertFalse(json.loads((out/'READBACK.json').read_bytes())['cleanup_verified'])

    def test_lost_start_ack_does_not_start_second_execution(self):
        with GraphFixture() as f:
            m=manifest();digest=f.call('graph_status',{'resource':'server'})['build']['source_content_sha256']
            starts=[];out=f.root/'uncertain'
            async def check():
                async with f.session() as s:
                    async def call(tool,args):
                        response=await f.raw_async(s,tool,args)
                        if tool=='graph_run_start':
                            starts.append(response['run_id'])
                            raise OSError('test injection: discard observed start ACK')
                        return response
                    with self.assertRaises(SubmissionUnknown):
                        await submit_once(call,m['spec'],m['input'],m['budgets'],out,'lost-ack',expected_build=digest)
                    with self.assertRaises(FileExistsError):
                        await submit_once(call,m['spec'],m['input'],m['budgets'],out,'lost-ack',expected_build=digest)
            asyncio.run(check())
            self.assertEqual(len(starts),1)
            observed=f.call('graph_run_get',{'run_id':starts[0],'compact':True})
            self.assertEqual(observed['run_id'],starts[0])
            with f.db() as db:self.assertEqual(db.execute('SELECT COUNT(*) FROM executions').fetchone()[0],1)
            self.assertTrue((out/'START_REQUEST.json').exists())
            self.assertFalse((out/'START_RESPONSE.json').exists())
