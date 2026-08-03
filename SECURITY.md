# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.2.x   | ✅ Yes    |
| < 0.2   | ❌ No     |

Security fixes are backported to the latest minor release only. Upgrade to the current version before reporting.

## Reporting a vulnerability

**Do not open a public issue.** Report security vulnerabilities privately through GitHub Security Advisories:

👉 [Report a vulnerability](https://github.com/RecursiveIntell/agent-graph-mcp/security/advisories/new)

Alternatively, email `security@recursiveintell.com`. Expect an initial response within 72 hours.

### What to include

- Description of the vulnerability and potential impact
- Steps to reproduce (minimal, self-contained)
- Affected versions
- Any proposed mitigation (optional)

### What happens next

1. **Acknowledgment** — you'll receive confirmation within 72 hours.
2. **Assessment** — we'll triage severity and scope within 5 business days.
3. **Fix development** — a fix is prepared in a private fork.
4. **Coordinated disclosure** — we'll agree on a disclosure date. A CVE will be requested if warranted. The fix is released and the advisory is published simultaneously.

## Scope

This policy covers vulnerabilities in:

- The `agent-graph-mcp` binary and `agent-graph-mcpd` daemon
- The npm package `@recursiveintell/agent-graph-mcp`
- The crates.io package `agent-graph-mcp`
- The `ri-agent-graph` runtime engine (when triggered through this server)

Out of scope:

- Vulnerabilities in LLM providers or model endpoints the server connects to
- Vulnerabilities in MCP clients (Hermes Agent, Claude Desktop, etc.)
- Denial-of-service through resource exhaustion (addressed via `--max-graphs` and execution budgets)
- Social engineering or phishing

## Security model

`agent-graph-mcp` operates with the authority of its caller. Key boundaries:

- **Daemon authentication** uses Unix socket peer credentials. Only the user who started the daemon can connect.
- **HMAC receipts** (SHA-256) authenticate source witness content and execution receipts.
- **Operator IPC** requires explicit authorization tokens.
- **No network listeners** in default configuration. The daemon binds to a local Unix socket only.
- **LLM calls** are made over the network to a configured endpoint. The server does not validate TLS certificates by default — configure your endpoint with HTTPS if transport security is required.

## Disclosure policy

We practice coordinated disclosure. We ask that you:

- Give us a reasonable window to fix the vulnerability before public disclosure (typically 90 days, negotiable for critical issues).
- Do not access, modify, or delete data that does not belong to you.
- Do not degrade service for other users while testing.

We will acknowledge your contribution in the advisory unless you request anonymity.

## Past advisories

None to date.

---

*This policy was last updated 2026-08-03.*
