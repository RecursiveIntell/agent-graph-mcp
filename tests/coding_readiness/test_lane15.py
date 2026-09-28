"""Regression coverage for read-only terminal artifact paging.

These integration tests use only the disposable private daemon fixture. The
graph contains deterministic state transforms and makes no provider calls.
"""
import unittest

from support import GraphFixture


def _read_pages(fixture, run_id, artifact, limit):
    """Read an artifact using a new MCP client connection for every page."""
    offset = 0
    identity = None
    pages = []
    for _ in range(128):
        args = {
            "run_id": run_id,
            "artifact": artifact,
            "offset": offset,
            "limit": limit,
        }
        if identity is not None:
            args["expected_digest"] = identity[0]
            args["expected_artifact_id"] = identity[1]
        page = fixture.call("graph_run_artifact", args)
        current = (page["digest"], page["artifact_id"], page["total_bytes"])
        if identity is None:
            identity = current
        else:
            if current != identity:
                raise AssertionError("artifact identity changed between pages")
        pages.append(page)
        if page["done"]:
            return pages
        if page["next_offset"] <= offset:
            raise AssertionError("artifact paging made no forward progress")
        offset = page["next_offset"]
    raise AssertionError("artifact page count exceeded")


def _canonical_page_projection(pages):
    """Return stable identity and byte fields, omitting connection metadata."""
    return [
        (
            page["artifact_id"],
            page["digest"],
            page["total_bytes"],
            page["offset"],
            page["next_offset"],
            page["done"],
            page["data"],
        )
        for page in pages
    ]


def _terminal_db_snapshot(fixture, run_id):
    """Capture stored terminal JSON bytes and this fixture's execution count."""
    connection = fixture.db()
    try:
        terminal = connection.execute(
            "SELECT receipt_json, bundle_json FROM terminal_receipts WHERE run_id = ?",
            (run_id,),
        ).fetchone()
        count = connection.execute(
            "SELECT COUNT(*) FROM executions WHERE run_id = ?", (run_id,)
        ).fetchone()[0]
        return terminal, count
    finally:
        connection.close()


class RepeatedArtifactReadTests(unittest.TestCase):
    def test_repeated_independent_paged_reads_preserve_identity_and_storage(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"payload": "private deterministic fixture"})
            before = _terminal_db_snapshot(fixture, run_id)
            self.assertIsNotNone(before[0], "fixture run must have a durable terminal row")
            self.assertEqual(before[1], 1, "fixture should execute exactly once")

            first_reads = {}
            second_reads = {}
            for artifact in ("receipt", "bundle"):
                first_reads[artifact] = _read_pages(fixture, run_id, artifact, 128)
                second_reads[artifact] = _read_pages(fixture, run_id, artifact, 128)
                self.assertGreater(
                    len(first_reads[artifact]),
                    1,
                    f"{artifact} fixture artifact should exercise continuation pages",
                )
                self.assertEqual(
                    _canonical_page_projection(first_reads[artifact]),
                    _canonical_page_projection(second_reads[artifact]),
                    f"repeated {artifact} reads must return identical bytes and identity",
                )
                self.assertTrue(
                    all(page["artifact_id"] == first_reads[artifact][0]["artifact_id"]
                        and page["digest"] == first_reads[artifact][0]["digest"]
                        for page in second_reads[artifact]),
                    f"all {artifact} continuation pages must retain the first-page identity",
                )

            after = _terminal_db_snapshot(fixture, run_id)
            self.assertEqual(after, before, "paged reads must not rewrite terminal bytes or execution rows")

    def test_artifact_identity_is_independent_of_page_boundaries(self):
        with GraphFixture() as fixture:
            run_id = fixture.run({"payload": "pagination boundary fixture"})
            for artifact in ("receipt", "bundle"):
                small_pages = _read_pages(fixture, run_id, artifact, 128)
                large_pages = _read_pages(fixture, run_id, artifact, 4096)
                self.assertEqual(len(large_pages), 1)
                self.assertEqual(
                    (small_pages[0]["artifact_id"], small_pages[0]["digest"]),
                    (large_pages[0]["artifact_id"], large_pages[0]["digest"]),
                    f"{artifact} identity must not depend on page size",
                )
                self.assertEqual(
                    "".join(page["data"] for page in small_pages),
                    large_pages[0]["data"],
                    f"{artifact} pages must assemble to the same canonical bytes",
                )


if __name__ == "__main__":
    unittest.main()
