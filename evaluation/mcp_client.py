from __future__ import annotations

import json
import os
import signal
import subprocess
from typing import Any


class ProxyClient:
    def __init__(self, binary: str, socket_path: str, timeout_ms: int = 300_000):
        self.proc = subprocess.Popen([binary, "--socket", socket_path, "--connect-timeout-ms", str(timeout_ms)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1, start_new_session=True)
        self._id = 0
        self.request("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "governed-proof", "version": "1"}})
        self.notify("notifications/initialized", {})

    def _write(self, value: dict[str, Any]) -> None:
        if self.proc.stdin is None:
            raise RuntimeError("proxy stdin closed")
        self.proc.stdin.write(json.dumps(value, separators=(",", ":")) + "\n")
        self.proc.stdin.flush()

    def notify(self, method: str, params: dict[str, Any]) -> None:
        self._write({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        self._id += 1
        request_id = self._id
        self._write({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        if self.proc.stdout is None:
            raise RuntimeError("proxy stdout closed")
        for line in self.proc.stdout:
            if not line.strip():
                continue
            response = json.loads(line)
            if response.get("id") == request_id:
                return response
        stderr = self.proc.stderr.read() if self.proc.stderr else ""
        raise RuntimeError(f"proxy closed before {method}: {stderr[-4000:]}")

    def call_tool(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        response = self.request("tools/call", {"name": name, "arguments": arguments})
        result = response.get("result", {})
        structured = result.get("structuredContent")
        if isinstance(structured, dict):
            return structured
        for item in result.get("content", []):
            if item.get("type") == "text":
                return json.loads(item["text"])
        if "error" in response:
            return {"ok": False, "error": response["error"], "error_code": "RPC_ERROR"}
        return {"ok": False, "error": "missing tool result", "error_code": "EMPTY_RESULT"}

    def close(self) -> None:
        if self.proc.stdin:
            self.proc.stdin.close()
        if self.proc.poll() is None:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(self.proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.proc.wait(timeout=5)
