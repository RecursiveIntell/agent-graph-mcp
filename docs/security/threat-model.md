# Agent Graph MCP threat model

## Principals

The MCP model client is an untrusted requester. The stdio proxy transports requests but is not an authority. The daemon owns durable state and enforces policy. A local operator is the only principal eligible for privileged approval and administration. Untrusted local users, provider endpoints, hooks/scripts, and installers are hostile or potentially compromised dependencies.

## Capability matrix

| Capability | Model client | Proxy | Daemon | Operator |
|---|---:|---:|---:|---:|
| graph read/create/run/cancel | request | forward | enforce | yes |
| witness capture/read | request | forward | validate | yes |
| checkpoint request/read | request | forward | validate | yes |
| approval decide | no | no | only operator route | yes |
| graph delete | no | no | operator route | yes |
| database migration | no | no | controlled startup | yes |
| config/install/release | no | no | no | yes |

Provider responses and model output are candidate data, never authority or source verification. Hooks and installers require separate review and least privilege.

## Boundary rule

The model-facing MCP capability set excludes approval decisions, permanent deletion, database migration, and release/install authority. A claimed actor label is metadata and cannot elevate a connection. Policy preflight is substantive but non-authoritative; authorization is derived from the connection channel and operator authentication.

## Primary threats

- confused deputy through self-asserted actor strings;
- forged or replayed approvals and checkpoints;
- malicious graph node types causing unbounded or effectful execution;
- tampered witness content, receipts, or provider output;
- provider exfiltration and hostile hook/script execution;
- destructive or supply-chain-compromised installers.

Controls must fail closed, preserve bounded receipts, and distinguish shape validation, integrity verification, witness binding, source authority, and factual support.
