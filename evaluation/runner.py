from __future__ import annotations

import argparse
import hashlib
import json
import time
from pathlib import Path
from typing import Any

from .adjudicate import adjudicate_text
from .contract import validate_corpus
from .mcp_client import ProxyClient


def sha256(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def unwrap(value: dict[str, Any]) -> dict[str, Any]:
    if isinstance(value.get("data"), dict):
        return value["data"]
    return value


def text_from_run(data: dict[str, Any], key: str) -> str:
    state = data.get("state") or {}
    value = data.get("final_state")
    if isinstance(value, str):
        return value
    value = state.get(key, value)
    if isinstance(value, str):
        return value
    return json.dumps(value, sort_keys=True)


def build_graph(client: ProxyClient, spec: dict[str, Any], experiment_id: str) -> tuple[str, dict[str, Any]]:
    graph = json.loads(json.dumps(spec))
    graph["name"] = f"{spec['name']}-{experiment_id}"
    raw = client.call_tool("graph_create", {"spec": graph, "idempotency_key": f"{experiment_id}-{spec['name']}-create"})
    data = unwrap(raw)
    if raw.get("ok") is False or data.get("status") not in {"created", "valid"}:
        raise RuntimeError(f"graph_create failed: {json.dumps(raw, sort_keys=True)}")
    return data["graph_id"], raw


def run_one(client: ProxyClient, graph_id: str, task: dict[str, Any], configuration: str, trial: int, graph_raw: dict[str, Any], run_prefix: str) -> tuple[dict[str, Any], dict[str, Any]]:
    start = time.time()
    started = client.call_tool("graph_run_start", {"graph_id": graph_id, "input": task, "idempotency_key": f"{run_prefix}-{task['id']}-{configuration}-{trial}-start"})
    start_data = unwrap(started)
    run_id = start_data.get("run_id")
    if not run_id:
        raise RuntimeError(f"graph_run_start failed: {json.dumps(started, sort_keys=True)}")
    waited = client.call_tool("graph_run_wait", {"run_id": run_id, "timeout_ms": 360000})
    wait_data = unwrap(waited)
    receipt_response = client.call_tool("graph_run_receipt", {"run_id": run_id})
    receipt_data = unwrap(receipt_response)
    receipt = receipt_data.get("receipt") or {}
    text = text_from_run(wait_data, "answer" if configuration == "baseline" else "final")
    outcome = adjudicate_text(task, text)
    outcome.update({"text": text, "text_sha256": "sha256:" + hashlib.sha256(text.encode()).hexdigest(), "status": wait_data.get("status"), "run_id": run_id, "budget_counters": wait_data.get("budget_counters", {})})
    envelope = {
        "schema": "recursiveintell.runner-envelope.v1", "task_id": task["id"], "configuration": configuration, "trial": trial,
        "started_at_unix": start, "finished_at_unix": time.time(), "raw_response": {"graph_create": graph_raw, "run_start": started, "run_wait": waited, "run_receipt": receipt_response},
        "receipt": receipt, "outcome": outcome,
    }
    raw_record = {"task_id": task["id"], "configuration": configuration, "trial": trial, "run_id": run_id, "responses": envelope["raw_response"]}
    return envelope, raw_record


def run(args: argparse.Namespace) -> dict[str, Any]:
    corpus_path = Path(args.corpus)
    corpus = json.loads(corpus_path.read_text())
    corpus_report = validate_corpus(corpus)
    if not corpus_report["valid"]:
        raise RuntimeError(f"invalid corpus: {corpus_report}")
    tasks = corpus["tasks"]
    if args.task_id:
        tasks = [task for task in tasks if task["id"] == args.task_id]
    specs = json.loads(Path(args.specs).read_text())
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=True)
    client = ProxyClient(args.binary, args.socket, args.proxy_timeout_ms)
    run_id = args.run_id or time.strftime("proof-%Y%m%dT%H%M%SZ", time.gmtime())
    experiment_id = run_id.replace("/", "-")
    graphs = {}
    envelopes: list[dict[str, Any]] = []
    try:
        for configuration in ("baseline", "candidate"):
            graph_id, graph_raw = build_graph(client, specs[configuration], experiment_id)
            graphs[configuration] = {"graph_id": graph_id, "create_response": graph_raw}
            for task in tasks:
                for trial in range(1, args.trials + 1):
                    envelope, raw_record = run_one(client, graph_id, task, configuration, trial, graph_raw, experiment_id)
                    envelopes.append(envelope)
                    with (out / "envelopes.jsonl").open("a") as handle:
                        handle.write(json.dumps(envelope, sort_keys=True) + "\n")
                    with (out / "raw-mcp.jsonl").open("a") as handle:
                        handle.write(json.dumps(raw_record, sort_keys=True) + "\n")
    finally:
        client.close()
    manifest = {
        "schema": "recursiveintell.governed-proof-run.v1", "run_id": run_id, "corpus": str(corpus_path), "corpus_sha256": sha256(corpus_path), "specs": str(args.specs), "binary": str(args.binary), "socket": str(args.socket), "tasks": [task["id"] for task in tasks], "trials": args.trials, "provider_attempts_expected": len(tasks) * args.trials * 5, "envelopes": len(envelopes), "graphs": graphs, "created_at_unix": time.time(), "runner": "evaluation.runner.v1",
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", default="evaluation/corpus/held_out_audit_v1.json")
    parser.add_argument("--specs", default="evaluation/graph_specs.json")
    parser.add_argument("--binary", required=True)
    parser.add_argument("--socket", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--run-id")
    parser.add_argument("--task-id")
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument("--proxy-timeout-ms", type=int, default=300000)
    args = parser.parse_args()
    print(json.dumps(run(args), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
