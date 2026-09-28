import importlib.util, pathlib, sys, unittest, hashlib, asyncio
from contextlib import asynccontextmanager
from unittest.mock import patch
PATH = pathlib.Path(__file__).resolve().parents[1] / 'scripts' / 'graph_run_client.py'
spec = importlib.util.spec_from_file_location('graph_run_client', PATH)
m = importlib.util.module_from_spec(spec); sys.modules[spec.name] = m; spec.loader.exec_module(m)

def page(data='"λ"'):
    b=data.encode();return {'schema':'agent-graph-artifact-page-v1','run_id':'r','graph_version':'v','artifact':'output','encoding':'json-utf8','digest':'sha256:'+hashlib.sha256(b).hexdigest(),'artifact_id':'identity','total_bytes':len(b),'offset':0,'next_offset':None,'done':True,'data':data}

class ClientTests(unittest.TestCase):
    def test_page_uses_bytes_and_verifies_full_digest(self):
        p=page();identity=m.validate_page(p, 'r', 'output', 0, None, 16384, 100)
        self.assertEqual(m.verify_assembled([p['data']],identity),p['data'].encode())
    def test_changed_identity_and_wrong_offset_are_rejected(self):
        p=page();identity=m.validate_page(p,'r','output',0,None,16384,100)
        for field,value in [('run_id','other'),('artifact_id','other'),('digest','bad'),('graph_version','other'),('offset',5)]:
            q=dict(p);q[field]=value
            with self.assertRaises(m.ReadError):m.validate_page(q,'r','output',0,identity,16384,100)
    def test_progress_and_bounds_fail_closed(self):
        for changes in [{'data':'' ,'done':False,'next_offset':0},{'total_bytes':101},{'next_offset':1,'done':True},{'data':'x'*30}]:
            p=page();p.update(changes)
            with self.assertRaises(m.ReadError):m.validate_page(p,'r','output',0,None,16,100)
    def test_bad_assembled_digest_rejected(self):
        p=page();identity=m.validate_page(p,'r','output',0,None,16384,100)
        with self.assertRaises(m.ReadError):m.verify_assembled(['xxxx'],identity)
    def test_terminal_status_does_not_skip_pending_persistence(self):
        self.assertFalse(m.durably_terminal({'status':'completed','success':True,'persistence_status':'pending'}))
        self.assertTrue(m.durably_terminal({'status':'failed','success':False,'persistence_status':'durable_terminal'}))
        self.assertFalse(m.durably_terminal({'status':'running','persistence_status':'durable_terminal'}))

class PersistentSessionTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.events = []
        self.calls = []
        self.fail_next = None
        self.reader = m.RunReader('/tmp/unused')

        class FakeResult:
            def __init__(self, data): self.data = data; self.is_error = False
            def model_dump(self, **kwargs): return {'structured_content':{'data':self.data}}
        self.fake_result = FakeResult

        @asynccontextmanager
        async def transport():
            number = sum(event == 'open' for event in self.events) + 1
            self.events.append('open')
            try:
                yield self
            finally:
                self.events.append('close')

        self.reader._transport_context = transport

    async def initialize(self):
        pass

    async def call_tool(self, tool, args):
        self.calls.append((tool, dict(args), self.events.count('open')))
        if self.fail_next is not None:
            error, self.fail_next = self.fail_next, None
            raise error
        if tool == 'graph_run_get':
            data = {'projection':'compact','run_id':args['run_id'],'status':'running','persistence_status':'pending'}
        else:
            text = '"λ"'
            encoded = text.encode()
            offset = args['offset']
            limit = args['limit']
            end = min(offset + limit, len(encoded))
            while end > offset:
                try:
                    chunk = encoded[offset:end].decode('utf-8')
                    break
                except UnicodeDecodeError:
                    end -= 1
            data = {'schema':'agent-graph-artifact-page-v1','run_id':'r','graph_version':'v','artifact':'output','encoding':'json-utf8','digest':'sha256:'+hashlib.sha256(encoded).hexdigest(),'artifact_id':'identity','total_bytes':len(encoded),'offset':offset,'next_offset':end if end < len(encoded) else None,'done':end >= len(encoded),'data':chunk}
        return self.fake_result(data)

    async def test_repeated_reads_reuse_one_initialized_session(self):
        async with self.reader:
            await self.reader.status('r')
            await self.reader.status('r')
        self.assertEqual(self.events, ['open','close'])
        self.assertEqual([call[2] for call in self.calls], [1,1])

    async def test_one_shot_status_and_artifact_api_remain_available(self):
        await self.reader.status('r')
        self.assertEqual(self.events, ['open','close'])
        self.events.clear(); self.calls.clear()
        data, _ = await self.reader.artifact('r','output',limit=3)
        self.assertEqual(data, '"λ"'.encode())
        self.assertEqual(self.events, ['open','close'])
        self.assertEqual({call[2] for call in self.calls}, {1})

    async def test_explicit_close_is_idempotent_and_context_exit_remains_safe(self):
        async with self.reader:
            await self.reader.close()
            await self.reader.close()
        self.assertEqual(self.events, ['open','close'])

    async def test_context_tears_down_on_normal_error_and_cancellation(self):
        async with self.reader:
            await self.reader.status('r')
        self.assertEqual(self.events, ['open','close'])

        self.events.clear(); self.calls.clear()
        with self.assertRaisesRegex(ValueError, 'boom'):
            async with self.reader:
                raise ValueError('boom')
        self.assertEqual(self.events, ['open','close'])

        self.events.clear(); self.calls.clear()
        entered = asyncio.Event()
        async def blocked():
            async with self.reader:
                entered.set()
                await asyncio.Event().wait()
        task = asyncio.create_task(blocked())
        await entered.wait(); task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await task
        self.assertEqual(self.events, ['open','close'])

    async def test_reconnect_closes_old_transport_before_opening_new(self):
        self.fail_next = OSError('transport down')
        async with self.reader:
            await self.reader.status('r')
        self.assertEqual(self.events, ['open','close','open','close'])
        self.assertEqual(self.reader.reconnections, 1)
        self.assertEqual(self.calls[0][:2], self.calls[1][:2])

    async def test_exhausted_transport_retries_are_bounded(self):
        @asynccontextmanager
        async def always_fails_transport():
            self.events.append('open')
            class BrokenSession:
                async def call_tool(inner, tool, args):
                    self.calls.append((tool, dict(args), self.events.count('open')))
                    raise OSError('transport down')
            try:
                yield BrokenSession()
            finally:
                self.events.append('close')
        self.reader._transport_context = always_fails_transport
        with self.assertRaises(OSError):
            async with self.reader:
                await self.reader.status('r')
        self.assertEqual(len(self.calls), 2)
        self.assertEqual(self.events, ['open','close','open','close'])

    async def test_application_failure_is_not_retried(self):
        self.fail_next = m.ReadError('PAGE_TARGET_MISMATCH')
        with self.assertRaisesRegex(m.ReadError, 'PAGE_TARGET_MISMATCH'):
            async with self.reader:
                await self.reader.status('r')
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.events, ['open','close'])

    async def test_mutation_method_is_rejected_before_transport(self):
        with self.assertRaisesRegex(m.ReadError, 'READ_ONLY_METHOD_REQUIRED'):
            await self.reader.read('graph_run_cancel', {'run_id':'r'})
        self.assertEqual(self.events, [])

    async def test_all_artifact_pages_share_the_persistent_session(self):
        async with self.reader:
            data, _ = await self.reader.artifact('r','output',limit=3)
        self.assertEqual(data, '"λ"'.encode())
        self.assertEqual(len(self.calls), 2)
        self.assertEqual({call[2] for call in self.calls}, {1})
        self.assertEqual(self.events, ['open','close'])

    async def test_reader_rejects_cross_task_use(self):
        async with self.reader:
            async def foreign_read():
                await self.reader.status('r')
            with self.assertRaisesRegex(m.ReadError, 'SESSION_TASK_MISMATCH'):
                await asyncio.create_task(foreign_read())

    async def test_cli_enters_one_persistent_scope(self):
        entered = []
        class FakeReader:
            reconnections = 0
            def __init__(self, *args): pass
            async def __aenter__(self): entered.append('enter'); return self
            async def __aexit__(self, *args): entered.append('exit')
            async def status(self, run_id): return {'status':'failed','persistence_status':'durable_terminal'}
            async def artifact(self, run_id, artifact): return b'{}', {'digest':'d'}
        import tempfile
        with tempfile.TemporaryDirectory(prefix='graph-cli-test-') as temp:
            out = pathlib.Path(temp) / 'read'
            args = type('Args',(),{'socket':'s','run_id':'r','out':str(out),'proxy':'p','wait_seconds':1})()
            with patch.object(m, 'RunReader', FakeReader), patch.object(m.argparse.ArgumentParser, 'parse_args', return_value=args):
                await m.main()
        self.assertEqual(entered, ['enter','exit'])
if __name__=='__main__':unittest.main()
