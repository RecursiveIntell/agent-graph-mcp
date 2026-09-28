"""No native transport for unit tests that already stub read responses.

The production reader now scopes an initialized session explicitly. Stub that
boundary too, without weakening any pagination/retry/authority assertions.
Native SDK tests intentionally do not use this helper.
"""
from contextlib import asynccontextmanager
from unittest.mock import patch


@asynccontextmanager
async def _unit_transport(_reader):
    yield None


def stub_session_context(test_case, reader_type):
    replacement = patch.object(reader_type, '_transport_context', _unit_transport)
    replacement.start()
    test_case.addCleanup(replacement.stop)
