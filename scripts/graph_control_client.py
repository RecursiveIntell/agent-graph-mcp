#!/usr/bin/env python3
"""Explicit controller submission for bounded advisory DAGs, not a run owner.

The caller supplies operator-admitted scope. Server validation and immutable
receipts remain authoritative. Local files record submission intent/ACKs only;
uncertain writes are quarantined and never automatically resubmitted.
"""
import argparse
import asyncio
import hashlib
import json
import os
import pathlib
import re


class AdmissionError(RuntimeError):
    """The observed server/input contract does not admit this advisory run."""


class SubmissionUnknown(RuntimeError):
    """A write may have happened; reconcile its recorded intent, never retry it."""


def encoded(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode('utf-8')


def _positive(value):
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def save_new(path, value):
    """Durably publish a new local evidence file without overwriting a receipt."""
    path = pathlib.Path(path)
    data = encoded(value)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def validate_advisory(spec, input_value, budgets, preflight, expected_build):
    """Consumer restrictions, using runtime-owned limits; never authorization."""
    if (preflight.get('authorization_granted') is not False or
            preflight.get('provider_generation_verified') is not False or
            not isinstance(preflight.get('limits'), dict)):
        raise AdmissionError('UNQUALIFIED_PREFLIGHT')
    if preflight.get('passed') is not True:
        raise AdmissionError('PREFLIGHT_REJECTED')
    if not expected_build or preflight.get('build', {}).get('source_content_sha256') != expected_build:
        raise AdmissionError('BUILD_IDENTITY_MISMATCH')
    limits = preflight['limits']
    for key in ('graph_bytes', 'input_bytes', 'node_timeout_multiplier', 'node_timeout_ceiling_ms'):
        if not _positive(limits.get(key)):
            raise AdmissionError('INVALID_RUNTIME_LIMITS')
    if len(encoded(spec)) > limits['graph_bytes'] or len(encoded(input_value)) > limits['input_bytes']:
        raise AdmissionError('REQUEST_SIZE_LIMIT')
    required = {'max_nodes', 'max_llm_calls', 'max_wall_clock_ms'}
    if set(budgets) != required or not all(_positive(v) for v in budgets.values()):
        raise AdmissionError('INVALID_BUDGETS')
    nodes = spec.get('nodes', [])
    ids = [n['id'] for n in nodes]
    if not ids or len(set(ids)) != len(ids):
        raise AdmissionError('INVALID_NODE_IDENTITIES')
    incoming = {n: 0 for n in ids}
    outgoing = {n: [] for n in ids}
    for edge in spec.get('edges', []):
        if edge['from'] not in incoming or (edge['to'] != 'END' and edge['to'] not in incoming):
            raise AdmissionError('INVALID_EDGE')
        if edge['to'] != 'END':
            outgoing[edge['from']].append(edge['to'])
            incoming[edge['to']] += 1
    ready = [n for n in ids if not incoming[n]]
    visited = 0
    while ready:
        node = ready.pop()
        visited += 1
        for target in outgoing[node]:
            incoming[target] -= 1
            if not incoming[target]:
                ready.append(target)
    if visited != len(ids):
        raise AdmissionError('DAG_REQUIRED')
    calls = 0
    worst_ms = 0
    for node in nodes:
        if node['type'] not in {'llm', 'join', 'state_transform', 'passthrough'}:
            raise AdmissionError('ADVISORY_NODE_REQUIRED')
        if node.get('config', {}).get('retry'):
            raise AdmissionError('ADVISORY_RETRY_NOT_ADMITTED')
        if node['type'] == 'llm':
            calls += 1
            timeout = node.get('config', {}).get('timeout_ms')
            if not _positive(timeout) or timeout > limits['node_timeout_ceiling_ms']:
                raise AdmissionError('EXPLICIT_NODE_TIMEOUT_REQUIRED')
            worst_ms += timeout * limits['node_timeout_multiplier']
    if budgets['max_nodes'] < len(nodes) or budgets['max_llm_calls'] < calls:
        raise AdmissionError('INSUFFICIENT_ATTEMPT_BUDGET')
    if budgets['max_wall_clock_ms'] <= worst_ms:
        raise AdmissionError('EFFECTIVE_DEADLINE_REQUIRES_HEADROOM')


async def submit_once(call, spec, input_value, budgets, out, idempotency_key, *, expected_build):
    """Submit once under caller-owned scope; no retry or resume behavior."""
    if not isinstance(idempotency_key, str) or not idempotency_key:
        raise AdmissionError('EXPLICIT_IDEMPOTENCY_KEY_REQUIRED')
    if not isinstance(expected_build, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", expected_build):
        raise AdmissionError("INVALID_BUILD_DIGEST")
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=False)
    intent = {'spec': spec, 'input': input_value, 'budgets': budgets, 'idempotency_key': idempotency_key, 'expected_build': expected_build}
    save_new(out / 'INTENT.json', intent)
    status = await call('graph_status', {'resource': 'server'})
    save_new(out / 'SERVER_STATUS.json', status)
    if status.get('ok') is not True or status.get('data', {}).get('build', {}).get('source_content_sha256') != expected_build:
        raise AdmissionError('BUILD_IDENTITY_MISMATCH')
    validated = await call('graph_create', {'action': 'validate', 'spec': spec})
    save_new(out / 'VALIDATE.json', validated)
    if validated.get('ok') is not True:
        raise AdmissionError('SERVER_SPEC_REJECTED')
    request = {'action': 'create', 'spec': spec, 'overwrite': False, 'idempotency_key': idempotency_key + ':definition'}
    save_new(out / 'CREATE_REQUEST.json', request)
    try:
        created = await call('graph_create', request)
    except asyncio.CancelledError:
        raise
    except Exception as error:
        raise SubmissionUnknown('CREATE_OUTCOME_UNKNOWN: reconcile CREATE_REQUEST.json') from error
    save_new(out / 'CREATE_RESPONSE.json', created)
    if created.get('ok') is not True:
        raise AdmissionError('SERVER_CREATE_REJECTED')
    version = created.get('graph_version')
    if not isinstance(version, str) or not version:
        raise SubmissionUnknown('CREATE_IDENTITY_UNKNOWN')
    report = await call('graph_policy_check', {'graph_id': spec['name'], 'input': input_value})
    save_new(out / 'PREFLIGHT.json', report)
    if report.get('ok') is not True:
        raise AdmissionError('SERVER_PREFLIGHT_REJECTED')
    validate_advisory(spec, input_value, budgets, report['data'], expected_build)
    start = {'graph_id': spec['name'], 'graph_version': version, 'input': input_value, 'budgets': budgets, 'idempotency_key': idempotency_key, 'checkpoint': False}
    save_new(out / 'START_REQUEST.json', start)
    try:
        result = await call('graph_run_start', start)
    except asyncio.CancelledError:
        raise
    except Exception as error:
        raise SubmissionUnknown('START_OUTCOME_UNKNOWN: reconcile START_REQUEST.json') from error
    if result.get('ok') is not True:
        save_new(out / 'START_REJECTED.json', result)
        raise AdmissionError('SERVER_START_REJECTED')
    if (not isinstance(result.get('run_id'), str) or not result['run_id'] or
            result.get('graph_id') != spec['name'] or result.get('graph_version') != version):
        save_new(out / 'START_UNCERTAIN_RESPONSE.json', result)
        raise SubmissionUnknown('START_IDENTITY_UNKNOWN')
    save_new(out / 'START_RESPONSE.json', result)
    return result


async def main():
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client
    from jsonschema import Draft202012Validator
    from graph_run_client import RunReader, durably_terminal, ReadError
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--socket', required=True)
    parser.add_argument('--proxy', required=True)
    parser.add_argument('--expected-build', required=True)
    parser.add_argument('--idempotency-key', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--submit', action='store_true', help='explicitly request submission under existing operator scope')
    args = parser.parse_args()
    if not args.submit:
        parser.error('--submit is required; this helper is not the read-only client')
    manifest = json.loads(pathlib.Path(args.manifest).read_text())
    params = StdioServerParameters(command=args.proxy, args=['--socket', args.socket, '--connect-timeout-ms', '5000'])
    async with asyncio.timeout(60):
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write, read_timeout_seconds=15.0) as session:
                await session.initialize()
                schema = {t.name: t.input_schema for t in (await session.list_tools()).tools}
                async def call(tool, arguments):
                    if tool not in {'graph_status', 'graph_create', 'graph_policy_check', 'graph_run_start'}:
                        raise AdmissionError('CONTROLLER_METHOD_NOT_ADMITTED')
                    Draft202012Validator(schema[tool]).validate(arguments)
                    result = await session.call_tool(tool, arguments)
                    raw = result.model_dump(mode='json', exclude_none=True)
                    payload = raw.get('structured_content') or raw.get('structuredContent')
                    if payload is None:
                        payload = json.loads(next(x['text'] for x in raw['content'] if x.get('type') == 'text'))
                    return payload
                started = await submit_once(call, manifest['spec'], manifest['input'], manifest['budgets'], args.out, args.idempotency_key, expected_build=args.expected_build)
    out = pathlib.Path(args.out)
    rid = started['run_id']
    async with asyncio.timeout(manifest['budgets']['max_wall_clock_ms'] / 1000 + 30):
        async with RunReader(args.socket, proxy=args.proxy) as reader:
            while True:
                status = await reader.status(rid)
                if durably_terminal(status):
                    break
                if status.get('persistence_status') == 'volatile_persistence_failed':
                    raise ReadError('PERSISTENCE_FAILED')
                await asyncio.sleep(2)
            save_new(out / 'TERMINAL_STATUS.json', status)
            identities = {}
            for kind in ['receipt', 'bundle'] + (['output'] if status.get('success') is True else []):
                data, identity = await reader.artifact(rid, kind)
                # Preserve canonical serialized bytes, not a locally reserialized projection.
                path = out / (kind + '.json')
                with path.open('xb') as stream:
                    stream.write(data); stream.flush(); os.fsync(stream.fileno())
                identities[kind] = identity
            save_new(out / 'READBACK.json', {'run_id': rid, 'artifacts': identities, 'reconnections': reader.reconnections, 'cleanup_verified': False, 'acceptance': 'controller_review_required'})
    print(json.dumps(status, indent=2))
    if status.get('success') is not True:
        raise SystemExit(1)


if __name__ == '__main__':
    asyncio.run(main())
