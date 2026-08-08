use agent_graph_mcp::codex_app_server::{
    collect_text, is_terminal_notification, run_turn, run_turn_pooled,
};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn collects_stream_deltas_and_ignores_non_terminal_events() {
    let events = vec![
        json!({"method":"thread/started","params":{}}),
        json!({"method":"item/agentMessage/delta","params":{"delta":"hello "}}),
        json!({"method":"item/agentMessage/delta","params":{"delta":"world"}}),
        json!({"method":"thread/tokenUsage/updated","params":{}}),
        json!({"method":"turn/completed","params":{"turn":{"status":"completed"}}}),
    ];
    assert_eq!(collect_text(&events), "hello world");
    assert!(!is_terminal_notification(&events[0]));
    assert!(is_terminal_notification(&events[4]));
}

#[test]
fn falls_back_to_completed_agent_message_when_delta_is_absent() {
    let events = vec![json!({
        "method":"item/completed",
        "params":{"item":{"type":"agentMessage","text":"final answer"}}
    })];
    assert_eq!(collect_text(&events), "final answer");
}

#[test]
fn runs_complete_turn_and_collects_text_from_a_stdio_app_server() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
read line
read line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-1"}}}'
read line
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-1"}}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"safe result"}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn-1"}}}'
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let text = run_turn(
        fake.to_str().unwrap(),
        "gpt-5.6-luna",
        dir.path(),
        "hello",
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(text, "safe result");
}

#[test]
fn reports_stdout_disconnect_and_bounded_stderr_context() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
printf '%09000d\n' 0 >&2
echo 'simulated startup failure' >&2
exit 2
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let error = run_turn(
        fake.to_str().unwrap(),
        "gpt-5.3-codex-spark",
        dir.path(),
        "hello",
        Duration::from_secs(2),
    )
    .unwrap_err();

    assert!(error.contains("stdout closed before response"), "{error}");
    assert!(error.contains("simulated startup failure"), "{error}");
    assert!(error.len() < 4_400, "stderr diagnostics were not bounded");
}

#[test]
fn redacts_sensitive_stderr_lines() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
echo 'Authorization: Bearer TEST_SECRET_123' >&2
exit 2
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let error = run_turn(
        fake.to_str().unwrap(),
        "gpt-5.3-codex-spark",
        dir.path(),
        "hello",
        Duration::from_secs(2),
    )
    .unwrap_err();

    assert!(
        error.contains("[redacted sensitive stderr line]"),
        "{error}"
    );
    assert!(!error.contains("should-not-leak"), "{error}");
}

#[test]
fn rejects_oversized_prompt_before_starting_child() {
    let dir = tempfile::tempdir().unwrap();
    let missing_binary = dir.path().join("must-not-be-started");
    let prompt = "x".repeat(256 * 1024 + 1);

    let error = run_turn(
        missing_binary.to_str().unwrap(),
        "gpt-5.6-luna",
        dir.path(),
        &prompt,
        Duration::from_secs(2),
    )
    .unwrap_err();

    assert!(
        error.contains("prompt exceeds bounded input limit"),
        "{error}"
    );
}

#[test]
fn rejects_oversized_streamed_output_without_retaining_event_history() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
read line
read line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-1"}}}'
read line
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-1"}}}'
printf '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"'
printf '%*s' 262145 '' | tr ' ' 'x'
printf '%s\n' '"}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn-1"}}}'
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let error = run_turn(
        fake.to_str().unwrap(),
        "gpt-5.6-luna",
        dir.path(),
        "hello",
        Duration::from_secs(2),
    )
    .unwrap_err();

    assert!(error.contains("output exceeds bounded limit"), "{error}");
}

#[test]
fn admission_wait_does_not_consume_provider_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("slow-fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
read line
read line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-1"}}}'
read line
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-1"}}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"queued result"}}'
sleep 0.5
printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn-1"}}}'
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let fake = Arc::new(fake);
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let fake = Arc::clone(&fake);
            thread::spawn(move || {
                run_turn(
                    fake.to_str().unwrap(),
                    "gpt-5.6-luna",
                    fake.parent().unwrap(),
                    "hello",
                    Duration::from_millis(700),
                )
            })
        })
        .collect();

    for handle in handles {
        assert_eq!(handle.join().unwrap().unwrap(), "queued result");
    }
}

#[test]
fn persistent_turn_deletes_completed_thread_before_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-websocket-codex");
    let log = dir.path().join("methods.log");
    let script = r##"#!/usr/bin/env python3
import json
import socket
import struct
import sys
from urllib.parse import urlparse

log_path = r"__LOG_PATH__"
listen = next(arg for arg in sys.argv if arg.startswith("ws://"))
port = urlparse(listen).port
server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind(("127.0.0.1", port))
server.listen(1)
conn, _ = server.accept()
request = b""
while b"\r\n\r\n" not in request:
    request += conn.recv(4096)
conn.sendall(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")

def recv_json():
    header = conn.recv(2)
    if len(header) != 2:
        raise EOFError()
    length = header[1] & 0x7f
    if length == 126:
        length = struct.unpack("!H", conn.recv(2))[0]
    elif length == 127:
        length = struct.unpack("!Q", conn.recv(8))[0]
    mask = conn.recv(4) if header[1] & 0x80 else b""
    payload = b""
    while len(payload) < length:
        payload += conn.recv(length - len(payload))
    if mask:
        payload = bytes(value ^ mask[index % 4] for index, value in enumerate(payload))
    return json.loads(payload)

def send_json(value):
    payload = json.dumps(value, separators=(",", ":")).encode()
    header = bytearray([0x81])
    if len(payload) <= 125:
        header.append(len(payload))
    elif len(payload) <= 65535:
        header.append(126)
        header.extend(struct.pack("!H", len(payload)))
    else:
        header.append(127)
        header.extend(struct.pack("!Q", len(payload)))
    conn.sendall(header + payload)

def record(method):
    with open(log_path, "a", encoding="utf-8") as handle:
        handle.write(method + "\n")

while True:
    message = recv_json()
    method = message.get("method", "")
    record(method)
    request_id = message.get("id")
    if method == "initialize":
        send_json({"jsonrpc":"2.0","id":request_id,"result":{}})
    elif method == "initialized":
        continue
    elif method == "thread/start":
        send_json({"jsonrpc":"2.0","id":request_id,"result":{"thread":{"id":"thread-1"}}})
    elif method == "turn/start":
        send_json({"jsonrpc":"2.0","id":request_id,"result":{"turn":{"id":"turn-1"}}})
        send_json({"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"memory-safe result"}})
        send_json({"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn-1","status":"completed"}}})
        conn.settimeout(2)
    elif method == "thread/delete":
        send_json({"jsonrpc":"2.0","id":request_id,"result":{}})
        break
conn.close()
server.close()
"##
    .replace("__LOG_PATH__", log.to_str().unwrap());
    fs::write(&fake, script).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let text = run_turn_pooled(
        fake.to_str().unwrap(),
        "gpt-5.6-luna-cleanup-regression",
        dir.path(),
        "hello",
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(text, "memory-safe result");

    let methods = fs::read_to_string(&log).unwrap();
    assert!(
        methods.lines().any(|method| method == "thread/delete"),
        "completed thread was not deleted; observed methods: {methods}"
    );
}
