//! Minimal, provider-neutral handling for Codex app-server JSON-RPC notifications.
//!
//! The transport emits numerous lifecycle/status events. Only `turn/completed`
//! is terminal. Text streams in `item/agentMessage/delta`; older/newer servers
//! may instead expose final text in a completed `agentMessage` item.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Condvar, Mutex, OnceLock};
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
        .next_back()
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
        let message = match rx.recv_timeout(remaining(deadline)?) {
            Ok(message) => message?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err("codex app-server timed out waiting for response".to_owned());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("codex app-server stdout closed before response".to_owned());
            }
        };
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(format!("codex app-server request {id} failed: {error}"));
        }
        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
    }
}

const MAX_STDERR_BYTES: usize = 8_192;
const MAX_STDERR_CHARS: usize = 4_096;
const DEFAULT_MAX_PROCESSES: usize = 3;
const MAX_CONFIGURED_PROCESSES: usize = 32;
const MAX_DISABLED_MCP_SERVERS: usize = 128;
const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_JSON_LINE_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const ADMISSION_WAIT_TIMEOUT: Duration = Duration::from_secs(900);
const PERSISTENT_STARTUP_TIMEOUT: Duration = Duration::from_secs(60);
const THREAD_CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);
const WEBSOCKET_HANDSHAKE_MAX_BYTES: usize = 8 * 1024;
const DISABLED_MCP_SERVERS_ENV: &str = "AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON";

fn parse_disabled_mcp_server_names(raw: &str) -> Result<Vec<String>, String> {
    let mut names = serde_json::from_str::<Vec<String>>(raw)
        .map_err(|error| format!("invalid {DISABLED_MCP_SERVERS_ENV}: {error}"))?;
    if names.len() > MAX_DISABLED_MCP_SERVERS {
        return Err(format!(
            "{DISABLED_MCP_SERVERS_ENV} exceeds bounded server count ({MAX_DISABLED_MCP_SERVERS})"
        ));
    }
    for name in &names {
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(format!(
                "invalid MCP server name in {DISABLED_MCP_SERVERS_ENV}"
            ));
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn apply_codex_isolation_overrides(command: &mut Command) -> Result<(), String> {
    let raw = match std::env::var(DISABLED_MCP_SERVERS_ENV) {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(()),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{DISABLED_MCP_SERVERS_ENV} is not valid UTF-8"));
        }
    };
    let names = parse_disabled_mcp_server_names(&raw)?;
    command.args(["-c", "features.plugins=false"]);
    command.args(["-c", "features.apps=false"]);
    for name in names {
        command.args(["-c", &format!("mcp_servers.{name}.enabled=false")]);
    }
    Ok(())
}

struct ProcessAdmission {
    active: Mutex<usize>,
    changed: Condvar,
    limit: usize,
}

struct ProcessPermit<'a> {
    admission: &'a ProcessAdmission,
}

impl Drop for ProcessPermit<'_> {
    fn drop(&mut self) {
        let mut active = self
            .admission
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *active = active.saturating_sub(1);
        self.admission.changed.notify_one();
    }
}

static PROCESS_ADMISSION: OnceLock<ProcessAdmission> = OnceLock::new();

fn process_admission() -> &'static ProcessAdmission {
    PROCESS_ADMISSION.get_or_init(|| {
        let limit = std::env::var("AGENT_GRAPH_CODEX_MAX_PROCESSES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_PROCESSES)
            .min(MAX_CONFIGURED_PROCESSES);
        ProcessAdmission {
            active: Mutex::new(0),
            changed: Condvar::new(),
            limit,
        }
    })
}

/// A single configured process uses the reusable worker. Higher limits select
/// bounded one-shot workers so graph fan-out is real provider concurrency.
pub fn use_persistent_worker() -> bool {
    process_admission().limit == 1
}

fn acquire_process() -> Result<ProcessPermit<'static>, String> {
    let admission = process_admission();
    let admission_deadline = Instant::now() + ADMISSION_WAIT_TIMEOUT;
    let mut active = admission.active.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if *active < admission.limit {
            *active += 1;
            return Ok(ProcessPermit { admission });
        }
        let remaining = remaining(admission_deadline)?;
        let (next, result) = admission
            .changed
            .wait_timeout(active, remaining)
            .map_err(|_| "codex process admission lock poisoned".to_owned())?;
        active = next;
        if result.timed_out() {
            return Err(format!(
                "codex app-server process capacity exhausted (limit {})",
                admission.limit
            ));
        }
    }
}

/// Drain stderr without allowing a faulty child to consume unbounded memory.
///
/// Keeping only the trailing bytes makes compiler/parser failures visible while
/// ensuring diagnostics remain bounded before they enter an MCP error.
fn capture_stderr_tail(stderr: &mut impl Read) -> String {
    let mut tail = Vec::with_capacity(MAX_STDERR_BYTES);
    let mut chunk = [0_u8; 1_024];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                tail.extend_from_slice(&chunk[..read]);
                if tail.len() > MAX_STDERR_BYTES {
                    let excess = tail.len() - MAX_STDERR_BYTES;
                    tail.drain(..excess);
                }
            }
        }
    }
    String::from_utf8_lossy(&tail).into_owned()
}

fn redacted_stderr_tail(stderr: String) -> Option<String> {
    let redacted = stderr
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "authorization",
                "api key",
                "api_key",
                "token",
                "secret",
                "password",
            ]
            .iter()
            .any(|marker| lower.contains(marker))
            {
                "[redacted sensitive stderr line]".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = redacted.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tail = if trimmed.len() > MAX_STDERR_CHARS {
        // Find a valid UTF-8 boundary so diagnostic rendering cannot panic on
        // a multi-byte character near the truncation point.
        let mut start = trimmed.len() - MAX_STDERR_CHARS;
        while !trimmed.is_char_boundary(start) {
            start += 1;
        }
        &trimmed[start..]
    } else {
        trimmed
    };
    Some(tail.to_owned())
}

fn attach_stderr_context(result: Result<String, String>, stderr: String) -> Result<String, String> {
    match (result, redacted_stderr_tail(stderr)) {
        (Err(error), Some(stderr)) => Err(format!("{error}; codex stderr tail: {stderr}")),
        (result, _) => result,
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
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!(
            "codex prompt exceeds bounded input limit ({} bytes)",
            MAX_PROMPT_BYTES
        ));
    }
    let _process_permit = acquire_process()?;
    let deadline = Instant::now() + timeout;
    let model_config = format!("model={model:?}");
    let reasoning_effort =
        std::env::var("AGENT_GRAPH_CODEX_REASONING_EFFORT").unwrap_or_else(|_| "high".to_owned());
    if !matches!(
        reasoning_effort.as_str(),
        "minimal" | "low" | "medium" | "high" | "xhigh"
    ) {
        return Err(format!(
            "unsupported Codex reasoning effort: {reasoning_effort}"
        ));
    }
    let reasoning_config = format!("model_reasoning_effort={reasoning_effort:?}");
    let mut command = Command::new(codex_bin);
    command.args([
        "app-server",
        "--stdio",
        "-c",
        model_config.as_str(),
        "-c",
        "sandbox_mode=\"read-only\"",
        "-c",
        reasoning_config.as_str(),
    ]);
    apply_codex_isolation_overrides(&mut command)?;
    let mut child = unsafe {
        command
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .pre_exec(|| {
                // Create a new process group so we can cleanly kill the entire
                // process tree on timeout/failure.  A negative PID in kill(2)
                // targets the group.  setpgid(0,0) returns 0 on success.
                if libc::setpgid(0, 0) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            })
            .spawn()
    }
    .map_err(|e| format!("failed to start codex app-server: {e}"))?;
    let pgid = child.id();
    let mut stdin = child.stdin.take().ok_or("codex stdin unavailable")?;
    let stdout = child.stdout.take().ok_or("codex stdout unavailable")?;
    let mut stderr = child.stderr.take().ok_or("codex stderr unavailable")?;
    let stderr_reader = thread::spawn(move || capture_stderr_tail(&mut stderr));
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let read = {
                let mut bounded = Read::by_ref(&mut reader).take((MAX_JSON_LINE_BYTES + 1) as u64);
                bounded.read_line(&mut line)
            };
            match read {
                Ok(0) => return,
                Ok(_) if line.len() > MAX_JSON_LINE_BYTES => {
                    let _ = tx.send(Err(format!(
                        "codex app-server response line exceeds bounded limit ({} bytes; observed at least {} bytes)",
                        MAX_JSON_LINE_BYTES,
                        line.len()
                    )));
                    return;
                }
                Ok(_) => {
                    let parsed =
                        serde_json::from_str::<Value>(line.trim_end()).map_err(|e| e.to_string());
                    if tx.send(parsed).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    return;
                }
            }
        }
    });
    // Startup gets a bounded sub-budget so a cold Node start doesn't
    // consume the entire generation window.
    let startup_deadline = deadline.min(Instant::now() + Duration::from_secs(60));
    let result = (|| {
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"agent-graph-mcp","title":"Agent Graph MCP","version":env!("CARGO_PKG_VERSION")},"capabilities":{}}}),
        )?;
        receive(&rx, 1, startup_deadline)?;
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        )?;
        send(
            &mut stdin,
            json!({"jsonrpc":"2.0","id":2,"method":"thread/start","params":{"cwd":cwd}}),
        )?;
        let thread = receive(&rx, 2, startup_deadline)?;
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
        let mut streamed_text = String::new();
        let mut fallback_text = None;
        loop {
            let event = match rx.recv_timeout(remaining(deadline)?) {
                Ok(event) => event?,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err("codex app-server timed out waiting for turn completion".to_owned());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("codex app-server stdout closed before turn completion".to_owned());
                }
            };
            if let Some(request_id) = event.get("id").and_then(Value::as_u64) {
                send(
                    &mut stdin,
                    json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32000,"message":"Agent Graph forbids Codex tool/approval requests"}}),
                )?;
                continue;
            }
            let terminal = is_terminal_notification(&event);
            if event
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|method| method == "item/agentMessage/delta")
            {
                if let Some(delta) = event.pointer("/params/delta").and_then(Value::as_str) {
                    if streamed_text.len() + delta.len() > MAX_OUTPUT_BYTES {
                        return Err(format!(
                            "codex app-server output exceeds bounded limit ({} bytes)",
                            MAX_OUTPUT_BYTES
                        ));
                    }
                    streamed_text.push_str(delta);
                }
            } else if event
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|method| method == "item/completed")
                && event
                    .pointer("/params/item/type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == "agentMessage")
            {
                if let Some(text) = event.pointer("/params/item/text").and_then(Value::as_str) {
                    if text.len() > MAX_OUTPUT_BYTES {
                        return Err(format!(
                            "codex app-server output exceeds bounded limit ({} bytes)",
                            MAX_OUTPUT_BYTES
                        ));
                    }
                    fallback_text = Some(text.to_owned());
                }
            }
            if terminal {
                let text = if streamed_text.is_empty() {
                    fallback_text.unwrap_or_default()
                } else {
                    streamed_text
                };
                if text.trim().is_empty() {
                    return Err("codex app-server completed without assistant text".to_owned());
                }
                return Ok(text);
            }
        }
    })();
    pgid_kill(pgid);
    let _ = child.wait();
    let stderr = stderr_reader.join().unwrap_or_default();
    attach_stderr_context(result, stderr)
}

/// Send SIGKILL to every process in the process group identified by `pgid`
/// (the codex app-server, its Node wrapper, and any subprocesses).
fn pgid_kill(pgid: u32) {
    // Graceful first: SIGTERM allows the Node wrapper to forward to children.
    unsafe { libc::kill(-(pgid as i32), libc::SIGTERM) };
    std::thread::sleep(std::time::Duration::from_millis(200));
    // Force: SIGKILL guarantees termination of any stragglers.
    unsafe { libc::kill(-(pgid as i32), libc::SIGKILL) };
}

/// Bounded shared stderr storage for a persistent app-server child.
struct SharedStderr {
    bytes: Mutex<Vec<u8>>,
}

fn capture_stderr_shared(stderr: &mut impl Read, shared: &SharedStderr) {
    let mut chunk = [0_u8; 1_024];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let mut bytes = shared.bytes.lock().unwrap_or_else(|e| e.into_inner());
                bytes.extend_from_slice(&chunk[..read]);
                if bytes.len() > MAX_STDERR_BYTES {
                    let excess = bytes.len() - MAX_STDERR_BYTES;
                    bytes.drain(..excess);
                }
            }
        }
    }
}

fn shared_stderr_text(shared: &SharedStderr) -> String {
    let bytes = shared.bytes.lock().unwrap_or_else(|e| e.into_inner());
    String::from_utf8_lossy(&bytes).into_owned()
}

fn send_websocket_frame(
    writer: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    mask_seed: u32,
) -> Result<(), String> {
    if payload.len() > MAX_JSON_LINE_BYTES {
        return Err(format!(
            "codex app-server websocket frame exceeds bounded limit ({} bytes)",
            MAX_JSON_LINE_BYTES
        ));
    }
    let mask = mask_seed.to_be_bytes();
    let mut header = Vec::with_capacity(14);
    header.push(0x80 | (opcode & 0x0f));
    let masked_length = 0x80_u8;
    match payload.len() {
        length @ 0..=125 => header.push(masked_length | length as u8),
        length @ 126..=65_535 => {
            header.push(masked_length | 126);
            header.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            header.push(masked_length | 127);
            header.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    writer.write_all(&header).map_err(|e| e.to_string())?;
    writer.write_all(&mask).map_err(|e| e.to_string())?;
    let mut masked = payload.to_vec();
    for (index, byte) in masked.iter_mut().enumerate() {
        *byte ^= mask[index % mask.len()];
    }
    writer.write_all(&masked).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

fn send_websocket_json(
    writer: &mut TcpStream,
    value: &Value,
    mask_seed: u32,
) -> Result<(), String> {
    let payload = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    send_websocket_frame(writer, 0x1, &payload, mask_seed)
}

fn read_websocket_frame(reader: &mut TcpStream) -> Result<Option<(bool, u8, Vec<u8>)>, String> {
    let mut header = [0_u8; 2];
    match reader.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.to_string()),
    }
    let fin = header[0] & 0x80 != 0;
    if header[0] & 0x70 != 0 {
        return Err("codex app-server websocket reserved bits were set".to_owned());
    }
    let opcode = header[0] & 0x0f;
    let masked = header[1] & 0x80 != 0;
    let mut length = (header[1] & 0x7f) as u64;
    if length == 126 {
        let mut extended = [0_u8; 2];
        reader
            .read_exact(&mut extended)
            .map_err(|e| e.to_string())?;
        length = u16::from_be_bytes(extended) as u64;
    } else if length == 127 {
        let mut extended = [0_u8; 8];
        reader
            .read_exact(&mut extended)
            .map_err(|e| e.to_string())?;
        length = u64::from_be_bytes(extended);
    }
    if length > MAX_JSON_LINE_BYTES as u64 {
        return Err(format!(
            "codex app-server websocket frame exceeds bounded limit ({} bytes)",
            MAX_JSON_LINE_BYTES
        ));
    }
    let mut mask = [0_u8; 4];
    if masked {
        reader.read_exact(&mut mask).map_err(|e| e.to_string())?;
    }
    let mut payload = vec![0_u8; length as usize];
    reader.read_exact(&mut payload).map_err(|e| e.to_string())?;
    if masked {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % mask.len()];
        }
    }
    Ok(Some((fin, opcode, payload)))
}

fn websocket_reader(
    mut reader: TcpStream,
    writer: Arc<Mutex<TcpStream>>,
    tx: mpsc::Sender<Result<Value, String>>,
) {
    let mut fragmented = None::<Vec<u8>>;
    loop {
        let frame = match read_websocket_frame(&mut reader) {
            Ok(Some(frame)) => frame,
            Ok(None) => return,
            Err(error) => {
                let _ = tx.send(Err(error));
                return;
            }
        };
        let (fin, opcode, payload) = frame;
        match opcode {
            0x8 => return,
            0x9 => {
                let mut writer = writer.lock().unwrap_or_else(|e| e.into_inner());
                if send_websocket_frame(&mut writer, 0xA, &payload, 0x504F4E47).is_err() {
                    return;
                }
            }
            0xA => {}
            0x1 if fin => {
                let parsed = serde_json::from_slice::<Value>(&payload).map_err(|e| e.to_string());
                if tx.send(parsed).is_err() {
                    return;
                }
            }
            0x1 => fragmented = Some(payload),
            0x0 => {
                let Some(buffer) = fragmented.as_mut() else {
                    let _ = tx.send(Err(
                        "codex app-server websocket continuation without text frame".to_owned(),
                    ));
                    return;
                };
                if buffer.len() + payload.len() > MAX_JSON_LINE_BYTES {
                    let _ = tx.send(Err(format!(
                        "codex app-server websocket message exceeds bounded limit ({} bytes)",
                        MAX_JSON_LINE_BYTES
                    )));
                    return;
                }
                buffer.extend_from_slice(&payload);
                if fin {
                    let complete = fragmented.take().unwrap_or_default();
                    let parsed =
                        serde_json::from_slice::<Value>(&complete).map_err(|e| e.to_string());
                    if tx.send(parsed).is_err() {
                        return;
                    }
                }
            }
            _ => {
                let _ = tx.send(Err(format!(
                    "unsupported codex app-server websocket opcode {opcode}"
                )));
                return;
            }
        }
    }
}

fn websocket_handshake(mut stream: TcpStream) -> Result<TcpStream, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(
            b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
        )
        .map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut response = Vec::with_capacity(512);
    let mut byte = [0_u8; 1];
    loop {
        stream.read_exact(&mut byte).map_err(|e| e.to_string())?;
        response.push(byte[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
        if response.len() > WEBSOCKET_HANDSHAKE_MAX_BYTES {
            return Err("codex app-server websocket handshake exceeded bounded limit".to_owned());
        }
    }
    let response = String::from_utf8_lossy(&response);
    if !response.starts_with("HTTP/1.1 101") && !response.starts_with("HTTP/1.0 101") {
        return Err(format!(
            "codex app-server websocket handshake failed: {response}"
        ));
    }
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    stream.set_write_timeout(None).map_err(|e| e.to_string())?;
    Ok(stream)
}

struct PersistentAppServer {
    child: Child,
    pgid: u32,
    writer: Arc<Mutex<TcpStream>>,
    rx: mpsc::Receiver<Result<Value, String>>,
    next_request_id: u64,
    stderr: Arc<SharedStderr>,
    codex_bin: String,
    model: String,
    cwd: PathBuf,
}

impl PersistentAppServer {
    fn start(codex_bin: &str, model: &str, cwd: &Path) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("reserve codex app-server listener: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("read codex app-server listener: {e}"))?
            .port();
        drop(listener);
        let listen_url = format!("ws://127.0.0.1:{port}");
        let model_config = format!("model={model:?}");
        let reasoning_effort = std::env::var("AGENT_GRAPH_CODEX_REASONING_EFFORT")
            .unwrap_or_else(|_| "high".to_owned());
        if !matches!(
            reasoning_effort.as_str(),
            "minimal" | "low" | "medium" | "high" | "xhigh"
        ) {
            return Err(format!(
                "unsupported Codex reasoning effort: {reasoning_effort}"
            ));
        }
        let reasoning_config = format!("model_reasoning_effort={reasoning_effort:?}");
        let mut command = Command::new(codex_bin);
        command.args([
            "app-server",
            "--listen",
            listen_url.as_str(),
            "-c",
            model_config.as_str(),
            "-c",
            "sandbox_mode=\"read-only\"",
            "-c",
            reasoning_config.as_str(),
        ]);
        apply_codex_isolation_overrides(&mut command)?;
        let mut child = unsafe {
            command
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .pre_exec(|| {
                    if libc::setpgid(0, 0) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                })
                .spawn()
        }
        .map_err(|e| format!("failed to start persistent codex app-server: {e}"))?;
        let pgid = child.id();
        let stderr = Arc::new(SharedStderr {
            bytes: Mutex::new(Vec::with_capacity(MAX_STDERR_BYTES)),
        });
        let mut child_stderr = child
            .stderr
            .take()
            .ok_or("persistent codex stderr unavailable")?;
        let stderr_for_reader = Arc::clone(&stderr);
        thread::spawn(move || capture_stderr_shared(&mut child_stderr, &stderr_for_reader));

        let startup_deadline = Instant::now() + PERSISTENT_STARTUP_TIMEOUT;
        let stream = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|e| format!("persistent codex app-server status failed: {e}"))?
            {
                pgid_kill(pgid);
                return Err(format!(
                    "persistent codex app-server exited during startup ({status}); stderr: {}",
                    shared_stderr_text(&stderr)
                ));
            }
            if Instant::now() >= startup_deadline {
                pgid_kill(pgid);
                let _ = child.wait();
                return Err(format!(
                    "persistent codex app-server startup timed out; stderr: {}",
                    shared_stderr_text(&stderr)
                ));
            }
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => {
                    if let Ok(stream) = websocket_handshake(stream) {
                        break stream;
                    }
                }
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        };
        let reader = stream
            .try_clone()
            .map_err(|e| format!("clone persistent codex stream: {e}"))?;
        let writer = Arc::new(Mutex::new(stream));
        let (tx, rx) = mpsc::channel();
        thread::spawn({
            let writer = Arc::clone(&writer);
            move || websocket_reader(reader, writer, tx)
        });
        let mut server = Self {
            child,
            pgid,
            writer,
            rx,
            next_request_id: 1,
            stderr,
            codex_bin: codex_bin.to_owned(),
            model: model.to_owned(),
            cwd: cwd.to_owned(),
        };
        server.initialize(startup_deadline)?;
        Ok(server)
    }

    fn initialize(&mut self, deadline: Instant) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "agent-graph-mcp",
                    "title": "Agent Graph MCP",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {}
            }),
            deadline,
        )?;
        self.notification("initialized", json!({}))
    }

    fn notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        let request = json!({"jsonrpc":"2.0","method":method,"params":params});
        let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        send_websocket_json(&mut writer, &request, self.next_request_id as u32)
    }

    fn request(&mut self, method: &str, params: Value, deadline: Instant) -> Result<Value, String> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        {
            let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
            send_websocket_json(&mut writer, &request, id as u32)?;
        }
        receive(&self.rx, id, deadline)
    }

    fn run_turn(&mut self, cwd: &Path, prompt: &str, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        let thread = self.request("thread/start", json!({"cwd": cwd}), deadline)?;
        let thread_id = thread
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .or_else(|| thread.pointer("/thread/sessionId").and_then(Value::as_str))
            .ok_or("codex thread/start returned no thread id")?;
        self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{"type":"text","text":prompt}]
            }),
            deadline,
        )?;
        let mut streamed_text = String::new();
        let mut fallback_text = None;
        loop {
            let event = match self.rx.recv_timeout(remaining(deadline)?) {
                Ok(event) => event?,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err("codex app-server timed out waiting for turn completion".to_owned());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(
                        "codex app-server websocket closed before turn completion".to_owned()
                    );
                }
            };
            if let Some(request_id) = event.get("id").and_then(Value::as_u64) {
                let denial = json!({
                    "jsonrpc":"2.0",
                    "id":request_id,
                    "error":{"code":-32000,"message":"Agent Graph forbids Codex tool/approval requests"}
                });
                let mut writer = self.writer.lock().unwrap_or_else(|e| e.into_inner());
                send_websocket_json(&mut writer, &denial, request_id as u32)?;
                continue;
            }
            let method = event
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if method == "item/agentMessage/delta" {
                if let Some(delta) = event.pointer("/params/delta").and_then(Value::as_str) {
                    if streamed_text.len() + delta.len() > MAX_OUTPUT_BYTES {
                        return Err(format!(
                            "codex app-server output exceeds bounded limit ({} bytes)",
                            MAX_OUTPUT_BYTES
                        ));
                    }
                    streamed_text.push_str(delta);
                }
            } else if method == "item/completed"
                && event
                    .pointer("/params/item/type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind == "agentMessage")
            {
                if let Some(text) = event.pointer("/params/item/text").and_then(Value::as_str) {
                    if text.len() > MAX_OUTPUT_BYTES {
                        return Err(format!(
                            "codex app-server output exceeds bounded limit ({} bytes)",
                            MAX_OUTPUT_BYTES
                        ));
                    }
                    fallback_text = Some(text.to_owned());
                }
            }
            if is_terminal_notification(&event) {
                let text = if streamed_text.is_empty() {
                    fallback_text.unwrap_or_default()
                } else {
                    streamed_text
                };
                if text.trim().is_empty() {
                    return Err("codex app-server completed without assistant text".to_owned());
                }
                // `thread/start` auto-subscribes this long-lived connection. Without
                // explicit deletion, every successful graph node remains loaded in
                // the persistent App Server indefinitely and swarm memory grows with
                // completed turns. Use a cleanup budget independent of the provider
                // deadline because Codex runs bounded SessionEnd hooks during delete.
                self.request(
                    "thread/delete",
                    json!({"threadId": thread_id}),
                    Instant::now() + THREAD_CLEANUP_TIMEOUT,
                )?;
                return Ok(text);
            }
        }
    }

    fn matches(&self, codex_bin: &str, model: &str, cwd: &Path) -> bool {
        self.codex_bin == codex_bin && self.model == model && self.cwd == cwd
    }
}

impl Drop for PersistentAppServer {
    fn drop(&mut self) {
        pgid_kill(self.pgid);
        let _ = self.child.wait();
    }
}

struct PersistentPool {
    server: Option<PersistentAppServer>,
}

static PERSISTENT_POOL: OnceLock<Mutex<PersistentPool>> = OnceLock::new();

fn persistent_pool() -> &'static Mutex<PersistentPool> {
    PERSISTENT_POOL.get_or_init(|| Mutex::new(PersistentPool { server: None }))
}

/// Run a turn through one long-lived Codex app-server worker.
///
/// The worker is deliberately single-lane: the Rust mutex serializes turns,
/// while the heavy Codex process is reused across graph nodes. This removes the
/// fixed memory multiplier from spawning one app-server per node. A failed or
/// timed-out turn tears down the worker so the next call gets a clean session.
pub fn run_turn_pooled(
    codex_bin: &str,
    model: &str,
    cwd: &Path,
    prompt: &str,
    timeout: Duration,
) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err("codex model must not be empty".to_owned());
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(format!(
            "codex prompt exceeds bounded input limit ({} bytes)",
            MAX_PROMPT_BYTES
        ));
    }
    let mut pool = persistent_pool()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let replace = pool
        .server
        .as_ref()
        .map_or(true, |server| !server.matches(codex_bin, model, cwd));
    if replace {
        pool.server.take();
        match PersistentAppServer::start(codex_bin, model, cwd) {
            Ok(server) => pool.server = Some(server),
            Err(error) if error.contains("exited during startup (exit status: 0)") => {
                // Keep stdio-only Codex-compatible executables usable for deterministic
                // harnesses. The production Codex CLI supports the listener path and
                // remains on the persistent worker path.
                drop(pool);
                return run_turn(codex_bin, model, cwd, prompt, timeout);
            }
            Err(error) => return Err(error),
        }
    }
    let server = pool
        .server
        .as_mut()
        .ok_or("persistent codex app-server worker unavailable")?;
    let result = server.run_turn(cwd, prompt, timeout);
    if let Err(error) = result {
        let stderr = pool
            .server
            .as_ref()
            .map(|server| shared_stderr_text(&server.stderr))
            .unwrap_or_default();
        pool.server.take();
        return attach_stderr_context(Err(error), stderr);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::parse_disabled_mcp_server_names;

    #[test]
    fn parses_and_deduplicates_bounded_mcp_server_names() {
        let names = parse_disabled_mcp_server_names(
            r#"["semantic-memory","claim-ledger","semantic-memory"]"#,
        )
        .unwrap();
        assert_eq!(names, vec!["claim-ledger", "semantic-memory"]);
    }

    #[test]
    fn rejects_mcp_server_names_that_could_escape_config_path() {
        let error = parse_disabled_mcp_server_names(r#"["safe","bad.name"]"#).unwrap_err();
        assert!(error.contains("invalid MCP server name"), "{error}");
    }
}
