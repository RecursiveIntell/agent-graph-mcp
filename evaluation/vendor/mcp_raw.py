#!/usr/bin/env python3
"""Minimal MCP JSON-RPC client for the agent-graph daemon Unix socket.

Framing: 4-byte big-endian length prefix + JSON payload (same as operator socket).
Usage:
  mcp_raw.py --socket PATH --call tools/call --args '{"name":"graph_create","arguments":{...}}'
"""
import argparse
import json
import socket
import sys


def read_frame(s):
    header = s.recv(4)
    if len(header) != 4:
        raise RuntimeError("socket closed before frame header")
    length = int.from_bytes(header, "big")
    payload = b""
    while len(payload) < length:
        chunk = s.recv(length - len(payload))
        if not chunk:
            break
        payload += chunk
    return json.loads(payload.decode())


def send_frame(s, obj):
    body = json.dumps(obj).encode()
    s.sendall(len(body).to_bytes(4, "big") + body)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--socket", required=True)
    ap.add_argument("--call", default="tools/call")
    ap.add_argument("--args", required=True, help="JSON object for the params field")
    ap.add_argument("--method", default=None, help="override method (default: --call value)")
    args = ap.parse_args()

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(120)
    try:
        s.connect(args.socket)
    except OSError as e:
        print(f"socket connect failed: {e}", file=sys.stderr)
        return 2

    params = json.loads(args.args)
    method = args.method or args.call

    # MCP initialize handshake
    send_frame(s, {
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "mcp_raw", "version": "0.1"},
        },
    })
    init = read_frame(s)
    send_frame(s, {"jsonrpc": "2.0", "method": "notifications/initialized"})

    send_frame(s, {"jsonrpc": "2.0", "id": 2, "method": method, "params": params})
    resp = read_frame(s)
    s.close()
    print(json.dumps(resp, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
