"""Controller-owned native MCP/daemon acceptance; no real model calls."""
import asyncio
import json
import pathlib
import sys
import time
import unittest
from support import GraphFixture, PROXY
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / 'scripts'))
from graph_run_client import RunReader


def fds(fixture):
    return len(list((pathlib.Path('/proc') / str(fixture.proc.pid) / 'fd').iterdir()))


def released(test, fixture, baseline):
    deadline = time.monotonic() + 2
    while fds(fixture) > baseline + 2 and time.monotonic() < deadline:
        time.sleep(.02)
    test.assertLessEqual(fds(fixture), baseline + 2)


class NativeReaderSessions(unittest.TestCase):
    def test_sdk_session_churn_reclaims_descriptors(self):
        with GraphFixture() as fixture:
            baseline = fds(fixture)
            for _ in range(30):
                fixture.call('graph_status', {'resource': 'server'})
            released(self, fixture, baseline)

    def test_reused_reader_polls_and_pages_then_releases_transport(self):
        with GraphFixture() as fixture:
            value = 'λ' * 12000
            rid = fixture.run(value)
            baseline = fds(fixture)
            async def check():
                reader = RunReader(fixture.socket, proxy=PROXY)
                async with reader:
                    session = reader._session
                    for _ in range(20):
                        status = await reader.status(rid)
                        self.assertEqual(status['status'], 'completed')
                    data, identity = await reader.artifact(rid, 'output', limit=256)
                    self.assertEqual(json.loads(data), value)
                    self.assertEqual(identity['run_id'], rid)
                    self.assertIs(reader._session, session)
                    self.assertLessEqual(fds(fixture), baseline + 2)
                self.assertIsNone(reader._session)
            asyncio.run(check())
            released(self, fixture, baseline)

    def test_real_transport_loss_reconnects_to_same_durable_run(self):
        with GraphFixture() as fixture:
            rid = fixture.run('kept')
            async def check():
                reader = RunReader(fixture.socket, proxy=PROXY)
                async with reader:
                    before = await reader.status(rid)
                    receipt_before, _ = await reader.artifact(rid, 'receipt')
                    fixture.restart()
                    after = await reader.status(rid)
                    # Restart changes the projection's storage-class label,
                    # not run identity, outcome, accounting or canonical bytes.
                    for key in ('run_id','graph_id','graph_version','status','success','persistence_status','budget_counters'):
                        self.assertEqual(before[key], after[key])
                    self.assertEqual(after['storage_class'], 'sqlite_terminal_record')
                    receipt_after, _ = await reader.artifact(rid, 'receipt')
                    self.assertEqual(receipt_before, receipt_after)
                    data, _ = await reader.artifact(rid, 'output')
                    self.assertEqual(json.loads(data), 'kept')
                    self.assertEqual(reader.reconnections, 1)
            asyncio.run(check())

    def test_cancellation_closes_real_sdk_context(self):
        with GraphFixture() as fixture:
            baseline = fds(fixture)
            async def check():
                entered = asyncio.Event()
                async def hold():
                    async with RunReader(fixture.socket, proxy=PROXY):
                        entered.set()
                        await asyncio.Event().wait()
                task = asyncio.create_task(hold())
                await entered.wait()
                task.cancel()
                with self.assertRaises(asyncio.CancelledError):
                    await task
            asyncio.run(check())
            released(self, fixture, baseline)
