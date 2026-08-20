from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from .contract import validate_batch


def digest_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def replay_envelopes(path: str | Path) -> dict[str, Any]:
    source = Path(path)
    raw = source.read_bytes()
    envelopes = [json.loads(line) for line in raw.splitlines() if line.strip()]
    task_ids = {e["task_id"] for e in envelopes if isinstance(e, dict) and "task_id" in e}
    validation = validate_batch(envelopes, task_ids)
    normalized = [
        {"task_id": e.get("task_id"), "configuration": e.get("configuration"), "trial": e.get("trial"), "score": e.get("outcome", {}).get("score"), "denial_failures": e.get("outcome", {}).get("denial_failures", [])}
        for e in sorted(envelopes, key=lambda x: (x.get("task_id", ""), x.get("configuration", ""), x.get("trial", 0)))
    ]
    return {
        "schema": "recursiveintell.replay-receipt.v1",
        "valid": validation["valid"],
        "validation": validation,
        "source_sha256": digest_bytes(raw),
        "normalized_sha256": digest_bytes(canonical_json(normalized)),
        "envelopes": normalized,
    }


def main() -> int:
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("path")
    parser.add_argument("--output")
    args = parser.parse_args()
    result = replay_envelopes(args.path)
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        Path(args.output).write_text(text)
    else:
        print(text, end="")
    return 0 if result["valid"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
