#!/usr/bin/env python3
"""Read/reconcile an existing Graph run without any execution authority.

Uses compact status and immutable UTF-8 artifact pages. Retrying a READ never
starts a run. Files are written only after byte length, identity and SHA256 agree.
Requires the existing MCP Python SDK for live calls; pure validators use stdlib.
"""
import argparse, asyncio, contextlib, hashlib, json, pathlib
from contextlib import AsyncExitStack

READ_TOOLS = frozenset({'graph_run_get', 'graph_run_artifact'})
TERMINAL = frozenset({'completed', 'failed', 'cancelled', 'interrupted_non_resumable'})

class ReadError(RuntimeError):
    """A bounded read could not establish its declared contract."""

def _integer(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0

def validate_page(page, run_id, artifact, offset, identity, limit, max_bytes):
    """Validate one page without accepting a changed source or losing bytes."""
    if not _integer(offset) or not _integer(page.get('offset')):
        raise ReadError('INVALID_BYTE_OFFSET_TYPE')
    if page.get('next_offset') is not None and not _integer(page['next_offset']):
        raise ReadError('INVALID_NEXT_OFFSET_TYPE')
    if not _integer(limit) or not 1 <= limit <= 16384 or not _integer(max_bytes):
        raise ReadError('INVALID_READ_BOUNDS')
    if page.get('schema') != 'agent-graph-artifact-page-v1' or page.get('encoding') != 'json-utf8':
        raise ReadError('UNSUPPORTED_PAGE_SCHEMA')
    if page.get('run_id') != run_id or page.get('artifact') != artifact or page.get('offset') != offset:
        raise ReadError('PAGE_TARGET_MISMATCH')
    fields = ('run_id','graph_version','artifact','encoding','digest','artifact_id','total_bytes')
    current = {key:page.get(key) for key in fields}
    if not all(isinstance(current[k], str) and current[k] for k in fields[:-1]):
        raise ReadError('INVALID_PAGE_IDENTITY')
    if not _integer(current['total_bytes']) or current['total_bytes'] > max_bytes:
        raise ReadError('ARTIFACT_SIZE_LIMIT')
    if identity is not None and current != identity:
        raise ReadError('ARTIFACT_CHANGED')
    data = page.get('data')
    if not isinstance(data, str):raise ReadError('INVALID_PAGE_DATA')
    size = len(data.encode('utf-8'));end = offset + size
    if not _integer(offset) or size > limit or end > current['total_bytes']:
        raise ReadError('PAGE_BOUNDS')
    if page.get('done') is True:
        if end != current['total_bytes'] or page.get('next_offset') is not None:
            raise ReadError('INVALID_FINAL_PAGE')
    elif page.get('done') is False:
        if size == 0 or end >= current['total_bytes'] or page.get('next_offset') != end:
            raise ReadError('PAGE_NO_PROGRESS')
    else:raise ReadError('INVALID_COMPLETION_FLAG')
    return current

def verify_assembled(chunks, identity):
    """Check exact serialized bytes before publishing an artifact locally."""
    data = ''.join(chunks).encode('utf-8')
    if len(data) != identity['total_bytes'] or 'sha256:'+hashlib.sha256(data).hexdigest() != identity['digest']:
        raise ReadError('ASSEMBLED_ARTIFACT_MISMATCH')
    return data

def durably_terminal(status):
    """Terminal execution, durable persistence and process cleanup are distinct."""
    return status.get('status') in TERMINAL and status.get('persistence_status') == 'durable_terminal'

class RunReader:
    """Task-owned, reconnection-safe read client for one configured MCP endpoint."""
    def __init__(self, socket, proxy='agent-graph-mcp', max_bytes=8*1024*1024):
        self.socket, self.proxy, self.max_bytes = socket, proxy, max_bytes
        self.reconnections = 0
        self._owner = None
        self._stack = None
        self._session = None
        self._reading = False

    @contextlib.asynccontextmanager
    async def _transport_context(self):
        """Open and initialize one native proxy plus MCP client session."""
        from mcp import ClientSession, StdioServerParameters
        from mcp.client.stdio import stdio_client
        params=StdioServerParameters(command=self.proxy,args=['--socket',self.socket,'--connect-timeout-ms','5000'])
        async with stdio_client(params) as (read,write):
            async with ClientSession(read,write,read_timeout_seconds=15) as session:
                await session.initialize()
                yield session

    async def _connect(self):
        stack=AsyncExitStack()
        try:
            self._session=await stack.enter_async_context(self._transport_context())
        except BaseException:
            await stack.aclose()
            raise
        self._stack=stack

    async def _disconnect(self):
        stack,self._stack=self._stack,None
        self._session=None
        if stack is not None:
            await stack.aclose()

    def _check_owner(self):
        if asyncio.current_task() is not self._owner:
            raise ReadError('SESSION_TASK_MISMATCH')

    async def __aenter__(self):
        if self._owner is not None:
            raise ReadError('SESSION_ALREADY_OPEN')
        self._owner=asyncio.current_task()
        try:
            await self._connect()
        except BaseException:
            self._owner=None
            raise
        return self

    async def __aexit__(self, exc_type, exc, tb):
        if self._owner is None:
            return False
        self._check_owner()
        try:
            await self._disconnect()
        finally:
            self._owner=None
        return False

    async def close(self):
        """Close an active persistent session in its owning task."""
        if self._owner is None:
            return
        self._check_owner()
        try:
            await self._disconnect()
        finally:
            self._owner=None

    async def _call(self, tool, args):
        result=await self._session.call_tool(tool,args)
        raw=result.model_dump(mode='json',exclude_none=True)
        payload=raw.get('structured_content') or raw.get('structuredContent')
        if payload is None:
            payload=json.loads(next(x['text'] for x in raw['content'] if x.get('type')=='text'))
        if result.is_error or payload.get('ok') is False:
            raise ReadError(payload.get('error_code') or 'MCP_READ_FAILED')
        return payload['data']

    async def _read_once(self, tool, args):
        return await self._call(tool,args)

    @staticmethod
    def _is_transport_error(error):
        leaves=[]
        def flatten(exc):
            if isinstance(exc,BaseExceptionGroup):
                for child in exc.exceptions:flatten(child)
            else:leaves.append(exc)
        flatten(error)
        return bool(leaves) and all(
            isinstance(item,(OSError,TimeoutError)) or str(item)=='Connection closed'
            for item in leaves
        )

    async def _read_in_scope(self, tool, args):
        self._check_owner()
        if self._reading:
            raise ReadError('CONCURRENT_READ_UNSUPPORTED')
        self._reading=True
        try:
            for attempt in range(2):
                try:
                    return await self._read_once(tool,args)
                except BaseException as error:
                    if not self._is_transport_error(error) or attempt:
                        raise
                    self.reconnections+=1
                    await self._disconnect()
                    await asyncio.sleep(.1)
                    await self._connect()
            raise ReadError('READ_RETRIES_EXHAUSTED')
        finally:
            self._reading=False

    async def read(self, tool, args):
        """Retry only a transport-failed read, once, using the same parameters."""
        if tool not in READ_TOOLS:
            raise ReadError('READ_ONLY_METHOD_REQUIRED')
        if self._owner is not None:
            return await self._read_in_scope(tool,args)
        async with self:
            return await self._read_in_scope(tool,args)

    async def status(self, run_id):
        data=await self.read('graph_run_get',{'run_id':run_id,'compact':True})
        if data.get('projection')!='compact':raise ReadError('COMPACT_STATUS_UNSUPPORTED')
        if data.get('run_id')!=run_id:raise ReadError('STATUS_TARGET_MISMATCH')
        return data

    async def artifact(self, run_id, artifact, limit=16384):
        if not _integer(limit) or not 1<=limit<=16384:raise ReadError('INVALID_PAGE_LIMIT')
        if self._owner is not None:
            return await self._artifact_in_scope(run_id,artifact,limit)
        async with self:
            return await self._artifact_in_scope(run_id,artifact,limit)

    async def _artifact_in_scope(self, run_id, artifact, limit):
        self._check_owner()
        offset=0;identity=None;chunks=[]
        for _ in range(4096):
            args={'run_id':run_id,'artifact':artifact,'offset':offset,'limit':limit}
            if identity is not None:args.update(expected_digest=identity['digest'],expected_artifact_id=identity['artifact_id'])
            page=await self.read('graph_run_artifact',args)
            identity=validate_page(page,run_id,artifact,offset,identity,limit,self.max_bytes)
            chunks.append(page['data'])
            if page['done']:return verify_assembled(chunks,identity),identity
            offset=page['next_offset']
        raise ReadError('PAGE_COUNT_LIMIT')

async def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--socket',required=True);ap.add_argument('--run-id',required=True);ap.add_argument('--out',required=True);ap.add_argument('--proxy',default='agent-graph-mcp');ap.add_argument('--wait-seconds',type=float,default=60)
    a=ap.parse_args();out=pathlib.Path(a.out);out.mkdir(parents=True,exist_ok=False)
    reader=RunReader(a.socket,a.proxy);record={'run_id':a.run_id,'effects':'read_only_no_dispatch','complete':False}
    try:
        async with asyncio.timeout(a.wait_seconds):
            async with reader:
                while True:
                    status=await reader.status(a.run_id)
                    if durably_terminal(status):break
                    if status.get('persistence_status')=='volatile_persistence_failed':raise ReadError('PERSISTENCE_FAILED')
                    await asyncio.sleep(.5)
                record['status']=status;record['artifacts']={}
                for kind in ['receipt','bundle']+(['output'] if status.get('status')=='completed' and status.get('success') is True else []):
                    data,identity=await reader.artifact(a.run_id,kind)
                    json.loads(data)
                    (out/(kind+'.json')).write_bytes(data);record['artifacts'][kind]=identity
                record['complete']=True
    finally:
        record['reconnections']=reader.reconnections
        (out/'READ_RECEIPT.json').write_text(json.dumps(record,indent=2))
        print(json.dumps(record,indent=2))
if __name__=='__main__':asyncio.run(main())
