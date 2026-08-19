# Phase 000 — Preflight and contract freeze

## Entry gate

Current source is available and the requested goal is recorded.

## Actions

1. Review `PRECHECK.json` against current source.
2. Populate `OWNER_MAP.yaml`.
3. Define later phases and validation gates.
4. Preserve unrelated dirty state.

## Acceptance gate

- Repository identity is correct.
- Every material semantic surface has one canonical owner.
- Scope, invariants, required checks, rollback, and forbidden leftovers are explicit.

## Stop conditions

Stop if source identity, ownership, or authority is ambiguous.
