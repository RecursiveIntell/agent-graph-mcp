# Offline governed-agent reliability proof harness

This directory contains the **no-graphs** portion of the governed-agent reliability proof. It validates the corpus contract, deterministic adjudication, paired comparison, raw-envelope preservation, and offline replay without invoking Agent Graph MCP, creating graphs, starting runs, or contacting a model provider.

## Evidence boundary

`offline-fixture-receipt-v1` and `synthetic_fixture_only` are deliberately not live execution evidence. The offline `PROMOTE` result only proves that the harness mechanics and fixture comparator are internally consistent. It does not prove agent quality, provider safety, graph reliability, or production readiness.

The live adapter is preserved in `runner.py`, `mcp_client.py`, and `graph_specs.json`, but it is not invoked by the offline command. A live run requires an explicit bounded execution decision.

## Local verification

From the repository root:

```bash
uv run --no-project --with pytest pytest -q evaluation/tests
python3 -m compileall -q evaluation
rm -rf evaluation/artifacts/offline
python3 -m evaluation.offline_runner \
  --output evaluation/artifacts/offline \
  --trials 3
python3 -m evaluation.replay \
  evaluation/artifacts/offline/offline-envelopes.jsonl \
  --output evaluation/artifacts/offline/replay.json
```

Expected offline invariants:

- 36 envelopes: six tasks × two configurations × three trials;
- six paired tasks;
- replay valid;
- `live_graph_calls: 0`;
- `provider_calls: 0`;
- synthetic-only evidence classification.

## Artifact ownership

- `contract.py`: versioned corpus/envelope validation;
- `adjudicate.py`: deterministic acceptance/denial adjudication;
- `compare.py`: paired scores, bootstrap interval, hard denial gate, and verdict;
- `offline_runner.py`: deterministic synthetic fixture producer;
- `replay.py`: raw JSONL replay and digest receipt;
- `corpus/held_out_audit_v1.json`: vendored held-out corpus;
- `vendor/deterministic_comparator.py`: source comparator preserved for audit comparison;
- `artifacts/offline/`: rebuildable local evidence projection, not canonical source truth.

## Remaining proof gate

A real baseline-versus-governed execution still requires a separately authorized live adapter, a real provider, durable run receipts, independent adjudication, replay from those receipts, and a second reproduction. None of those claims are made by this no-graphs pass.
