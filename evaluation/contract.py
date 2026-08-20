"""Strict, versioned contracts for corpus and run envelopes."""
from __future__ import annotations

from typing import Any

CORPUS_SCHEMA = "recursiveintell.held-out-corpus.v1"
ENVELOPE_SCHEMA = "recursiveintell.runner-envelope.v1"
RECEIPT_SCHEMAS = {"agent-graph-mcp-receipt-v2", "agent-graph-mcp-receipt-v1", "offline-fixture-receipt-v1"}
OFFLINE_ENVELOPE_SCHEMA = "recursiveintell.offline-envelope.v1"
REQUIRED_TASK_FIELDS = {"id", "task", "task_profile", "acceptance_criteria", "denial_criteria"}


def _errors(*items: str) -> list[str]:
    return [item for item in items if item]


def validate_corpus(corpus: Any) -> dict[str, Any]:
    errors: list[str] = []
    if not isinstance(corpus, dict):
        return {"valid": False, "tasks": 0, "errors": ["corpus must be an object"]}
    if corpus.get("schema") != CORPUS_SCHEMA:
        errors.append(f"schema must be {CORPUS_SCHEMA}")
    tasks = corpus.get("tasks")
    if not isinstance(tasks, list) or len(tasks) != 6:
        errors.append("corpus must contain exactly six held-out tasks")
        tasks = tasks if isinstance(tasks, list) else []
    ids: set[str] = set()
    for index, task in enumerate(tasks):
        if not isinstance(task, dict):
            errors.append(f"task {index} must be an object")
            continue
        missing = REQUIRED_TASK_FIELDS - task.keys()
        if missing:
            errors.append(f"task {index} missing {sorted(missing)}")
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            errors.append(f"task {index} has invalid id")
        elif task_id in ids:
            errors.append(f"duplicate task id {task_id}")
        else:
            ids.add(task_id)
        if not isinstance(task.get("acceptance_criteria"), list) or not task["acceptance_criteria"]:
            errors.append(f"task {task_id} has no acceptance criteria")
        if not isinstance(task.get("denial_criteria"), list) or not task["denial_criteria"]:
            errors.append(f"task {task_id} has no denial criteria")
    return {"valid": not errors, "tasks": len(tasks), "errors": errors}


def validate_envelope(envelope: Any) -> dict[str, Any]:
    errors: list[str] = []
    if not isinstance(envelope, dict):
        return {"valid": False, "errors": ["envelope must be an object"]}
    if envelope.get("schema") not in {ENVELOPE_SCHEMA, OFFLINE_ENVELOPE_SCHEMA}:
        errors.append(f"schema must be {ENVELOPE_SCHEMA} or {OFFLINE_ENVELOPE_SCHEMA}")
    if envelope.get("schema") == OFFLINE_ENVELOPE_SCHEMA and envelope.get("evidence_class") != "synthetic_fixture_only":
        errors.append("offline envelopes must declare synthetic_fixture_only evidence")
    for field in ("task_id", "configuration", "trial", "raw_response", "receipt", "outcome"):
        if field not in envelope:
            errors.append(f"missing {field}")
    if envelope.get("configuration") not in {"baseline", "candidate"}:
        errors.append("configuration must be baseline or candidate")
    receipt = envelope.get("receipt")
    if not isinstance(receipt, dict):
        errors.append("receipt must be an object")
    elif receipt.get("schema") not in RECEIPT_SCHEMAS:
        errors.append("receipt schema is missing or unsupported")
    outcome = envelope.get("outcome")
    if not isinstance(outcome, dict):
        errors.append("outcome must be an object")
    else:
        if not isinstance(outcome.get("text"), str):
            errors.append("outcome.text must be a string")
        if not isinstance(outcome.get("score"), (int, float)):
            errors.append("outcome.score must be numeric")
        if not isinstance(outcome.get("denial_failures"), list):
            errors.append("outcome.denial_failures must be a list")
    return {"valid": not errors, "errors": errors}


def validate_batch(envelopes: list[dict[str, Any]], task_ids: set[str]) -> dict[str, Any]:
    errors: list[str] = []
    seen: set[tuple[str, str, int]] = set()
    for envelope in envelopes:
        report = validate_envelope(envelope)
        errors.extend(report["errors"])
        key = (envelope.get("task_id", ""), envelope.get("configuration", ""), envelope.get("trial", -1))
        if key in seen:
            errors.append(f"duplicate envelope {key}")
        seen.add(key)
        if envelope.get("task_id") not in task_ids:
            errors.append(f"unknown task {envelope.get('task_id')}")
    return {"valid": not errors, "envelopes": len(envelopes), "errors": errors}
