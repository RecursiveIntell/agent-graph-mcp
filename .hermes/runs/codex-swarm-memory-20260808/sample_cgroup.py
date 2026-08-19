#!/usr/bin/env python3
import csv
import os
import subprocess
import sys
import time
from pathlib import Path

out = Path(sys.argv[1])
control_group = subprocess.check_output(
    ["systemctl", "--user", "show", "agent-graph-mcpd.service", "-p", "ControlGroup", "--value"],
    text=True,
).strip()
cgroup = Path("/sys/fs/cgroup") / control_group.lstrip("/")
fields = ["anon", "file", "kernel", "kernel_stack", "pagetables", "sock", "slab", "slab_reclaimable", "slab_unreclaimable"]
header = [
    "unix_s",
    "memory_current",
    "memory_peak",
    "process_count",
    "rss_total_kib",
    "codex_process_count",
    "codex_rss_kib",
    "mcp_process_count",
    "mcp_rss_kib",
    *fields,
]

def integer(path):
    try:
        return int(path.read_text().strip())
    except (OSError, ValueError):
        return -1

def stat_values():
    values = {}
    try:
        for line in (cgroup / "memory.stat").read_text().splitlines():
            key, value = line.split()
            values[key] = int(value)
    except (OSError, ValueError):
        pass
    return values

def rss_kib(pid):
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    except (OSError, ValueError):
        pass
    return 0

def cmdline(pid):
    try:
        return Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode("utf-8", "replace")
    except OSError:
        return ""

out.parent.mkdir(parents=True, exist_ok=True)
with out.open("w", newline="") as handle:
    writer = csv.writer(handle)
    writer.writerow(header)
    handle.flush()
    while True:
        try:
            pids = [int(value) for value in (cgroup / "cgroup.procs").read_text().split()]
        except (OSError, ValueError):
            pids = []
        rss = {pid: rss_kib(pid) for pid in pids}
        commands = {pid: cmdline(pid) for pid in pids}
        codex_pids = [pid for pid, command in commands.items() if "codex app-server" in command]
        mcp_pids = []
        for pid, command in commands.items():
            argv = command.split()
            executable = Path(argv[0]).name if argv else ""
            script = Path(argv[1]).name if len(argv) > 1 else ""
            if "semantic-memory-mcp" in executable or (
                executable.startswith("python") and script.endswith("-mcp.py")
            ):
                mcp_pids.append(pid)
        stats = stat_values()
        writer.writerow([
            f"{time.time():.6f}",
            integer(cgroup / "memory.current"),
            integer(cgroup / "memory.peak"),
            len(pids),
            sum(rss.values()),
            len(codex_pids),
            sum(rss.get(pid, 0) for pid in codex_pids),
            len(mcp_pids),
            sum(rss.get(pid, 0) for pid in mcp_pids),
            *[stats.get(field, -1) for field in fields],
        ])
        handle.flush()
        time.sleep(0.25)
