import asyncio
import copy
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest

PATH = pathlib.Path(__file__).resolve().parents[1] / 'scripts' / 'graph_control_client.py'
SPEC = {'name':'advice','entry':'one','nodes':[{'id':'one','type':'llm','model':'fixture','prompt':'{input}','config':{'timeout_ms':1000}}], 'edges':[{'from':'one','to':'END'}]}
BUDGETS = {'max_nodes':1,'max_llm_calls':1,'max_wall_clock_ms':2500}
POLICY = {'passed':True,'authorization_granted':False,'provider_generation_verified':False,'limits':{'node_timeout_multiplier':2,'node_timeout_ceiling_ms':120000,'input_bytes':65536,'graph_bytes':65536}, 'build':{'source_content_sha256':'sha256:' + 'a'*64}}

class ControllerTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.assertTrue(PATH.exists(), 'maintained controller submission seam is missing')
        spec=importlib.util.spec_from_file_location('graph_control_client',PATH)
        self.m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=self.m;spec.loader.exec_module(self.m)
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.out=pathlib.Path(self.temp.name)/'submission';self.calls=[];self.transport_error=None;self.policy=copy.deepcopy(POLICY)
    async def call(self,tool,args):
        self.calls.append((tool,copy.deepcopy(args)))
        if tool=='graph_status':return {'ok':True,'data':{'build':self.policy.get('build',{})}}
        if tool=='graph_create' and args.get('action')=='validate':return {'ok':True}
        if tool=='graph_create':return {'ok':True,'graph_version':'v1','graph_id':'advice'}
        if tool=='graph_policy_check':return {'ok':True,'data':self.policy}
        if tool=='graph_run_start':
            if self.transport_error:raise self.transport_error
            return {'ok':True,'run_id':'run-1','graph_version':'v1','graph_id':'advice'}
        raise AssertionError(tool)
    async def submit(self,**kw):
        return await self.m.submit_once(self.call,SPEC,{'evidence':'source'},kw.get('budgets',BUDGETS),self.out,'intent-key',expected_build='sha256:' + 'a'*64)
    async def test_explicit_submission_binds_request_before_transport(self):
        result=await self.submit()
        self.assertEqual(result['run_id'],'run-1')
        self.assertEqual([c[0] for c in self.calls],['graph_status','graph_create','graph_create','graph_policy_check','graph_run_start'])
        saved=json.loads((self.out/'START_REQUEST.json').read_text())
        self.assertEqual(saved,self.calls[-1][1]);self.assertEqual(saved['idempotency_key'],'intent-key')
    async def test_unknown_start_is_quarantined_without_redispatch(self):
        self.transport_error=TimeoutError('lost reply')
        with self.assertRaises(self.m.SubmissionUnknown):await self.submit()
        self.assertTrue((self.out/'START_REQUEST.json').exists());self.assertFalse((self.out/'START_RESPONSE.json').exists())
        before=len(self.calls)
        with self.assertRaises(FileExistsError):await self.submit()
        self.assertEqual(len(self.calls),before)
    async def test_policy_success_never_grants_authority(self):
        self.policy={'passed':True,'build':copy.deepcopy(POLICY['build'])}
        with self.assertRaisesRegex(self.m.AdmissionError,'UNQUALIFIED_PREFLIGHT'):await self.submit()
        self.assertNotIn('graph_run_start',[x[0] for x in self.calls])
    async def test_effective_timeout_budget_is_checked(self):
        with self.assertRaisesRegex(self.m.AdmissionError,'EFFECTIVE_DEADLINE'):await self.submit(budgets={**BUDGETS,'max_wall_clock_ms':1900})
        self.assertNotIn('graph_run_start',[x[0] for x in self.calls])
    async def test_build_drift_blocks_submission(self):
        self.policy['build']['source_content_sha256']='sha256:' + 'b'*64
        with self.assertRaisesRegex(self.m.AdmissionError,'BUILD_IDENTITY'):await self.submit()
        self.assertNotIn('graph_create',[x[0] for x in self.calls])
    async def test_rejected_policy_blocks_submission(self):
        self.policy['passed']=False
        with self.assertRaisesRegex(self.m.AdmissionError,'PREFLIGHT_REJECTED'):await self.submit()
    async def test_nonpositive_bool_and_unknown_budgets_reject(self):
        for bad in [{**BUDGETS,'max_llm_calls':True},{**BUDGETS,'max_nodes':0},{**BUDGETS,'extra':1}]:
            with self.assertRaises(self.m.AdmissionError):self.m.validate_advisory(SPEC,{},bad,POLICY,'sha256:' + 'a'*64)
    async def test_cycles_and_effectful_nodes_are_not_admitted_by_advisory_helper(self):
        cyclic=copy.deepcopy(SPEC);cyclic['edges']=[{'from':'one','to':'one'}]
        with self.assertRaisesRegex(self.m.AdmissionError,'DAG_REQUIRED'):self.m.validate_advisory(cyclic,{},BUDGETS,POLICY,'sha256:' + 'a'*64)
        effect=copy.deepcopy(SPEC);effect['nodes'][0]['type']='tool'
        with self.assertRaisesRegex(self.m.AdmissionError,'ADVISORY_NODE_REQUIRED'):self.m.validate_advisory(effect,{},BUDGETS,POLICY,'sha256:' + 'a'*64)
    async def test_malformed_start_response_is_unknown_not_success(self):
        original=self.call
        async def malformed(tool,args):
            if tool=='graph_run_start':return {'ok':True}
            return await original(tool,args)
        with self.assertRaises(self.m.SubmissionUnknown):
            await self.m.submit_once(malformed,SPEC,{},BUDGETS,self.out,'intent-key',expected_build='sha256:' + 'a'*64)
    async def test_invalid_build_token_is_rejected_before_lookup(self):
        with self.assertRaisesRegex(self.m.AdmissionError,'INVALID_BUILD_DIGEST'):
            await self.m.submit_once(self.call,SPEC,{},BUDGETS,self.out,'intent-key',expected_build='sha256:not-a-digest')
        self.assertEqual(self.calls, [])

    async def test_cancelled_submit_preserves_intent_and_does_not_retry(self):
        self.transport_error=asyncio.CancelledError()
        with self.assertRaises(asyncio.CancelledError):await self.submit()
        self.assertTrue((self.out/'START_REQUEST.json').exists())
        self.assertEqual(sum(t=='graph_run_start' for t,a in self.calls),1)

if __name__=='__main__':unittest.main()
