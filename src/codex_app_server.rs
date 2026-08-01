//! Minimal, provider-neutral handling for Codex app-server JSON-RPC notifications.
//!
//! The transport emits numerous lifecycle/status events. Only `turn/completed`
//! is terminal. Text streams in `item/agentMessage/delta`; older/newer servers
//! may instead expose final text in a completed `agentMessage` item.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Returns true only for the app-server's terminal turn notification.
pub fn is_terminal_notification(event: &Value) -> bool {
    event
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|method| method == "turn/completed")
}

/// Collect assistant text without treating non-message events as output.
///
/// Delta text is authoritative when present. The completed-item form is a
/// compatibility fallback for app-server versions that omit deltas.
pub fn collect_text(events: &[Value]) -> String {
    let deltas: String = events
        .iter()
        .filter(|event| {
            event
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|method| method == "item/agentMessage/delta")
        })
        .filter_map(|event| event.pointer("/params/delta").and_then(Value::as_str))
        .collect();
    if !deltas.is_empty() {
        return deltas;
    }
    events
        .iter()
        .filter(|event| {
            event
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|method| method == "item/completed")
        })
        .filter(|event| {
            event
                .pointer("/params/item/type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == "agentMessage")
        })
        .filter_map(|event| event.pointer("/params/item/text").and_then(Value::as_str))
        .last()
        .unwrap_or_default()
        .to_owned()
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| "codex app-server timed out".to_owned())
}

fn send(stdin: &mut impl Write, request: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *stdin, &request).map_err(|e| e.to_string())?;
    stdin.write_all(b"\n").map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())
}

fn receive(
    rx: &mpsc::Receiver<Result<Value, String>>,
    id: u64,
    deadline: Instant,
) -> Result<Value, String> {
    loop {
        let message = rx
            .recv_timeout(remaining(deadline)?)
            .map_err(|_| "codex app-server timed out waiting for response".to_owned())??;
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(format!("codex app-server request {id} failed: {error}"));
        }
        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
}

/// Run one read-only Codex app-server turn without exposing OAuth credentials.
///
/// The child owns its own Codex login/refresh lifecycle. Agent Graph passes no
/// auth environment, token, or auth-store path. Server-initiated requests are
/// denied because graph LLM nodes are prompt-only execution, not tool agents.
pub fn run_turn(
    codex_bin: &str,
    model: &str,
    cwd: &Path,
    prompt: &str,
    timeout: Duration,
) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err("codex model must not be empty".to_owned());
    }
    let mut child = Command::new(codex_bin)
        .args([
            "app-server",
            "--stdio",
            "-c",
            &format!("model={:?}", model),
            "-c",
            "sandbox_mode=\"read-only\"",
        ])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start codex app-server: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("codex stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("codex stdout unavailable")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let parsed = line
                .map_err(|e| e.to_string())
                .and_then(|line| serde_json::from_str::<Value>(&line).map_err(|e| e.to_string()));
            if tx.send(parsed).is_err() {
                return;
            }
        }
    });
    let deadline = Instant::now() + timeout;
    let result = (|| {
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"agent-graph-mcp","title":"Agent Graph MCP","version":env!("CARGO_PKG_VERSION")},"capabilities":{}}}),
        )?;
        receive(&rx, 1, deadline)?;
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        )?;
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":2,"method":"thread/start","params":{"cwd":cwd}}),
        )?;
        let thread = receive(&rx, 2, deadline)?;
        let thread_id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .or_else(|| thread.pointer("/thread/sessionId").and_then(Value::as_str))
            .ok_or("codex thread/start returned no thread id")?;
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":3,"method":"turn/start","params":{"threadId":thread_id,"input":[{"type":"text","text":prompt}]}}),
        )?;
        receive(&rx, 3, deadline)?;
        let mut events = Vec::new();
        loop {
            let event = rx.recv_timeout(remaining(deadline)?).map_err(|_| {
                "codex app-server timed out waiting for turn completion".to_owned()
            })??;
            if let Some(request_id) = event.get("id").and_then(Value::as_u64) {
                send(
                    &mut stdin,
                    json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32000,"message":"Agent Graph forbids Codex tool/approval requests"}}),
                )?;
                continue;
            }
            let terminal = is_terminal_notification(&event);
            events.push(event);
            if terminal {
                let text = collect_text(&events);
                if text.trim().is_empty() {
                    return Err("codex app-server completed without assistant text".to_owned());
                }
                return Ok(text);
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}
