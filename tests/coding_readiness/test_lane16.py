"""Focused regression tests for graph_run_client read authority and artifact identity."""
import hashlib
import importlib.util
import pathlib
import sys
import unittest


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / 'scripts' / 'graph_run_client.py'
CLIENT_SPEC = importlib.util.spec_from_file_location('lane16_graph_run_client', CLIENT_PATH)
client = importlib.util.module_from_spec(CLIENT_SPEC)
sys.modules[CLIENT_SPEC.name] = client
CLIENT_SPEC.loader.exec_module(client)


def page(run_id, artifact, offset, data, total_bytes, digest, *, done, next_offset,
         artifact_id='artifact-1', graph_version='graph-v1'):
    return {
        'schema': 'agent-graph-artifact-page-v1',
        'encoding': 'json-utf8',
        'run_id': run_id,
        'graph_version': graph_version,
        'artifact': artifact,
        'offset': offset,
        'digest': digest,
        'artifact_id': artifact_id,
        'total_bytes': total_bytes,
        'data': data,
        'done': done,
        'next_offset': next_offset,
    }


class GraphRunClientReadinessTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        from reader_unit_transport import stub_session_context
        stub_session_context(self, client.RunReader)

    async def test_execution_methods_are_rejected_before_transport(self):
        reader = client.RunReader('/private/test.sock')
        transport_calls = []

        async def forbidden_transport(tool, args):
            transport_calls.append((tool, args))
            raise AssertionError('transport must not be invoked for execution methods')

        reader._read_once = forbidden_transport
        for tool in ('graph_run_start', 'graph_create', 'graph_run_cancel'):
            with self.subTest(tool=tool):
                with self.assertRaisesRegex(client.ReadError, '^READ_ONLY_METHOD_REQUIRED$'):
                    await reader.read(tool, {'run_id': 'run-1'})
        self.assertEqual(transport_calls, [])
        self.assertEqual(reader.reconnections, 0)

    async def test_artifact_rejects_changed_source_identity_before_returning_bytes(self):
        run_id, artifact = 'run-1', 'output'
        serialized = '{"value":"ok"}'.encode('utf-8')
        digest = 'sha256:' + hashlib.sha256(serialized).hexdigest()
        first_text, second_text = '{"value":', '"ok"}'
        split = len(first_text.encode('utf-8'))
        pages = [
            page(run_id, artifact, 0, first_text, len(serialized), digest,
                 done=False, next_offset=split),
            page(run_id, artifact, split, second_text, len(serialized), digest,
                 done=True, next_offset=None, artifact_id='changed-artifact'),
        ]
        calls = []
        reader = client.RunReader('/private/test.sock')

        async def transport_boundary(tool, args):
            calls.append((tool, args))
            self.assertEqual(tool, 'graph_run_artifact')
            return pages[len(calls) - 1]

        reader._read_once = transport_boundary
        with self.assertRaisesRegex(client.ReadError, '^ARTIFACT_CHANGED$'):
            await reader.artifact(run_id, artifact, limit=64)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][1]['expected_digest'], digest)
        self.assertEqual(calls[1][1]['expected_artifact_id'], 'artifact-1')

    async def test_artifact_returns_exact_bytes_for_consistent_utf8_pages(self):
        run_id, artifact = 'run-2', 'output'
        expected = '{"value":"café 🌱"}'.encode('utf-8')
        # Split on character boundaries while exercising byte-based offsets.
        first_text, second_text = '{"value":"café ', '🌱"}'
        split = len(first_text.encode('utf-8'))
        digest = 'sha256:' + hashlib.sha256(expected).hexdigest()
        pages = [
            page(run_id, artifact, 0, first_text, len(expected), digest,
                 done=False, next_offset=split),
            page(run_id, artifact, split, second_text, len(expected), digest,
                 done=True, next_offset=None),
        ]
        calls = []
        reader = client.RunReader('/private/test.sock')

        async def transport_boundary(tool, args):
            calls.append((tool, args))
            self.assertEqual(tool, 'graph_run_artifact')
            return pages[len(calls) - 1]

        reader._read_once = transport_boundary
        data, identity = await reader.artifact(run_id, artifact, limit=64)
        self.assertEqual(data, expected)
        self.assertEqual(identity['digest'], digest)
        self.assertEqual(identity['artifact_id'], 'artifact-1')
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][1]['offset'], split)
        self.assertEqual(calls[1][1]['expected_digest'], digest)
        self.assertEqual(calls[1][1]['expected_artifact_id'], 'artifact-1')


if __name__ == '__main__':
    unittest.main()
