"""Pure regression tests for graph_run_client artifact assembly."""
import hashlib
import importlib.util
import pathlib
import sys
import unittest


CLIENT_PATH = pathlib.Path(__file__).resolve().parents[2] / "scripts" / "graph_run_client.py"
SPEC = importlib.util.spec_from_file_location("lane11_graph_run_client", CLIENT_PATH)
CLIENT = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CLIENT
SPEC.loader.exec_module(CLIENT)


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def make_pages(chunks, *, digest_value=None):
    """Build transport pages with byte offsets for the supplied text chunks."""
    data = "".join(chunks).encode("utf-8")
    identity = {
        "schema": "agent-graph-artifact-page-v1",
        "run_id": "run-1",
        "graph_version": "graph-v3",
        "artifact": "output",
        "encoding": "json-utf8",
        "digest": digest_value or digest(data),
        "artifact_id": "artifact-1",
        "total_bytes": len(data),
    }
    pages = []
    offset = 0
    for index, chunk in enumerate(chunks):
        size = len(chunk.encode("utf-8"))
        last = index == len(chunks) - 1
        pages.append({
            **identity,
            "offset": offset,
            "data": chunk,
            "done": last,
            "next_offset": None if last else offset + size,
        })
        offset += size
    return pages


class PageReader(CLIENT.RunReader):
    """Stub only the transport boundary; keep RunReader assembly logic intact."""
    def __init__(self, pages):
        super().__init__(socket="unused")
        self.pages = pages
        self.calls = []

    async def read(self, tool, args):
        self.calls.append((tool, dict(args)))
        self.assert_page_call(tool, args)
        index = len(self.calls) - 1
        return self.pages[index]

    def assert_page_call(self, tool, args):
        if tool != "graph_run_artifact":
            raise AssertionError(f"unexpected tool: {tool}")


class ArtifactAssemblyTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        from reader_unit_transport import stub_session_context
        stub_session_context(self, CLIENT.RunReader)

    async def test_corrupted_digest_is_rejected_after_assembly(self):
        pages = make_pages(["{\"ok\":", "true}"], digest_value="sha256:" + "0" * 64)
        reader = PageReader(pages)

        with self.assertRaisesRegex(CLIENT.ReadError, "ASSEMBLED_ARTIFACT_MISMATCH"):
            await reader.artifact("run-1", "output", limit=16)

    async def test_altered_final_page_bytes_are_rejected(self):
        pages = make_pages(["{\"ok\":", "true}"])
        pages[1]["data"] = "nope}"
        # The source's immutable total and digest still describe the original bytes.
        reader = PageReader(pages)

        with self.assertRaisesRegex(CLIENT.ReadError, "ASSEMBLED_ARTIFACT_MISMATCH"):
            await reader.artifact("run-1", "output", limit=16)

    async def test_continuation_rejects_changed_identity_fields(self):
        original = make_pages(["{\"x\":", "1}"])
        changed_fields = (
            ("artifact_id", "artifact-2", "ARTIFACT_CHANGED"),
            ("graph_version", "graph-v4", "ARTIFACT_CHANGED"),
            ("run_id", "run-2", "PAGE_TARGET_MISMATCH"),
            ("artifact", "receipt", "PAGE_TARGET_MISMATCH"),
        )
        for field, value, error in changed_fields:
            with self.subTest(field=field):
                pages = [dict(page) for page in original]
                pages[1][field] = value
                reader = PageReader(pages)
                with self.assertRaisesRegex(CLIENT.ReadError, error):
                    await reader.artifact("run-1", "output", limit=16)

    async def test_unicode_offsets_use_utf8_bytes_and_valid_pages_assemble(self):
        text = '{"name":"猫", "ok":true}'
        # The first page is four bytes; the continuation offset accounts for 猫's UTF-8 bytes.
        pages = make_pages(['{"na', 'me":"猫",', ' "ok":true}'])
        reader = PageReader(pages)

        assembled, identity = await reader.artifact("run-1", "output", limit=16)

        self.assertEqual(assembled, text.encode("utf-8"))
        self.assertEqual(identity["total_bytes"], len(text.encode("utf-8")))
        self.assertEqual([call[1]["offset"] for call in reader.calls], [0, 4, 14])
        self.assertEqual(reader.calls[1][1]["expected_digest"], identity["digest"])
        self.assertEqual(reader.calls[1][1]["expected_artifact_id"], identity["artifact_id"])


if __name__ == "__main__":
    unittest.main()
