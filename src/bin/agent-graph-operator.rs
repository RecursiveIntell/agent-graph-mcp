//! Human-facing operator client. It never opens the Agent Graph database.
//!
//! Modes:
//! - Legacy/operator mode: `agent-graph-operator [--non-interactive] [socket] <action> <resource> <digest> <nonce> [state] [reason]`
//!   talks to operator.sock (peer-credentialed, uid-gated). Actions are
//!   serde snake_case: set_graph_retention, approve_graph_deletion,
//!   delete_graph, clear_execution_lineage, purge_graph, purge_run, ...
//! - MCP convenience subcommands (talk to mcp.sock, read-only + run tools):
//!   status | retention-review [limit] | run-triage [limit] |
//!   run-wait <run_id> [timeout_ms] | run-poll <run_id> | council-run <spec.json>
//!
//! Socket defaults resolve: AGENT_GRAPH_OPERATOR_SOCKET env, then
//! <data-dir>/run/status.json (daemon-published single source of truth), then
//! the legacy default path.
use agent_graph_mcp::operator_auth::OperatorAction;
use agent_graph_mcp::operator_ipc::{OperatorFrame, OperatorResponse, PROTOCOL};
use chrono::{Duration, Utc};
use std::{
    env,
    io::{self, BufRead, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

const DEFAULT_OPERATOR_SOCKET: &str = "~/.local/share/agent-graph/run/operator.sock";
const DEFAULT_MCP_SOCKET: &str = "~/.local/share/agent-graph/run/mcp.sock";

fn expand_home(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    p.to_string()
}

/// Read a daemon-published status.json (single source of truth for sockets).
fn read_status() -> Option<serde_json::Value> {
    let candidate = env::var("AGENT_GRAPH_STATUS_FILE")
        .ok()
        .map(|s| expand_home(&s))
        .unwrap_or_else(|| expand_home("~/.local/share/agent-graph/run/status.json"));
    let raw = std::fs::read_to_string(candidate).ok()?;
    serde_json::from_str(&raw).ok()
}

fn resolve_operator_socket() -> String {
    if let Ok(s) = env::var("AGENT_GRAPH_OPERATOR_SOCKET") {
        return s;
    }
    if let Some(status) = read_status() {
        if let Some(s) = status.get("operator_socket").and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    expand_home(DEFAULT_OPERATOR_SOCKET)
}

fn resolve_mcp_socket() -> String {
    if let Ok(s) = env::var("AGENT_GRAPH_MCP_SOCKET") {
        return s;
    }
    if let Some(status) = read_status() {
        if let Some(s) = status.get("mcp_socket").and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    expand_home(DEFAULT_MCP_SOCKET)
}

/// One JSON-RPC tools/call over the MCP socket (line-delimited framing).
fn mcp_call(
    tool: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let socket = resolve_mcp_socket();
    let mut stream = UnixStream::connect(&socket)
        .map_err(|e| format!("mcp socket {socket} unavailable: {e}"))?;
    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": tool, "arguments": arguments}
    });
    stream.write_all(request.to_string().as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    let mut reader = io::BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: serde_json::Value = serde_json::from_str(&line)?;
    if let Some(err) = resp.get("error") {
        return Err(format!("mcp error: {err}").into());
    }
    let data = resp["result"]["structuredContent"]["data"].clone();
    if data.is_null() {
        // Some tools return content[0].text JSON instead.
        let text = resp["result"]["content"][0]["text"].clone();
        if let Some(t) = text.as_str() {
            return Ok(serde_json::from_str(t).unwrap_or(text));
        }
        return Ok(resp["result"].clone());
    }
    Ok(data)
}

fn print_pretty(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn cmd_retention_review(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let limit = args.first().and_then(|s| s.parse().ok()).unwrap_or(100);
    let data = mcp_call(
        "graph_retention_review",
        serde_json::json!({"limit": limit}),
    )?;
    print_pretty(&data);
    Ok(())
}

fn cmd_run_triage(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let limit = args.first().and_then(|s| s.parse().ok()).unwrap_or(100);
    let data = mcp_call("graph_run_triage", serde_json::json!({"limit": limit}))?;
    print_pretty(&data);
    Ok(())
}

fn cmd_run_wait(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = args.first().ok_or("run-wait requires <run_id>")?;
    let timeout: Option<u64> = args.get(1).and_then(|s| s.parse().ok());
    let data = mcp_call(
        "graph_run_wait",
        serde_json::json!({"run_id": run_id, "timeout_ms": timeout}),
    )?;
    print_pretty(&data);
    Ok(())
}

fn cmd_run_poll(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = args.first().ok_or("run-poll requires <run_id>")?;
    let data = mcp_call("graph_run_get", serde_json::json!({"run_id": run_id}))?;
    print_pretty(&data);
    Ok(())
}

fn cmd_receipt(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = args.first().ok_or("receipt requires <run_id>")?;
    let data = mcp_call("graph_run_receipt", serde_json::json!({"run_id": run_id}))?;
    print_pretty(&data);
    Ok(())
}

fn cmd_status() -> Result<(), Box<dyn std::error::Error>> {
    let data = mcp_call("graph_status", serde_json::json!({"resource": "server"}))?;
    print_pretty(&data);
    Ok(())
}

/// G10: council-run — create + start + poll + print receipt summary. The spec
/// file may carry an optional "input" key passed to the run.
fn cmd_council_run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let spec_path = args.first().ok_or("council-run requires <spec.json>")?;
    let spec_text =
        std::fs::read_to_string(spec_path).map_err(|e| format!("read spec {spec_path}: {e}"))?;
    let mut spec: serde_json::Value = serde_json::from_str(&spec_text)?;
    let run_input = spec.get("input").cloned().unwrap_or(serde_json::json!({}));
    if let Some(obj) = spec.as_object_mut() {
        obj.remove("input");
    }
    let created = mcp_call(
        "graph_create",
        serde_json::json!({"action": "create", "spec": spec}),
    )?;
    let graph_id = created
        .get("graph_id")
        .or(created.get("name"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("graph_create returned no graph_id: {created}"))?
        .to_string();
    let started = mcp_call(
        "graph_run_start",
        serde_json::json!({"graph_id": graph_id, "input": run_input}),
    )?;
    let run_id = started
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("graph_run_start returned no run_id: {started}"))?
        .to_string();
    println!("graph: {graph_id} | run: {run_id}");
    // Poll until terminal (max ~10 min).
    for _ in 0..75 {
        std::thread::sleep(std::time::Duration::from_secs(8));
        let state = mcp_call("graph_run_get", serde_json::json!({"run_id": run_id}))?;
        let status = state.get("status").and_then(|v| v.as_str()).unwrap_or("");
        print!("\r  status: {status}");
        io::stdout().flush()?;
        if matches!(
            status,
            "completed" | "failed" | "cancelled" | "interrupted_non_resumable"
        ) {
            println!();
            let receipt = mcp_call("graph_run_receipt", serde_json::json!({"run_id": run_id}))?;
            print_pretty(&receipt);
            return Ok(());
        }
    }
    Err("council-run timed out waiting for terminal state".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let non_interactive = raw_args.iter().any(|a| a == "--non-interactive")
        || std::env::var("AGENT_GRAPH_OPERATOR_TOKEN").is_ok();
    let mut args: Vec<String> = raw_args
        .into_iter()
        .filter(|a| a != "--non-interactive")
        .collect();

    // MCP convenience subcommands (E1 dashboard / B9 triage / G10 run client).
    match args.first().map(String::as_str) {
        Some("status") => return cmd_status(),
        Some("retention-review") => {
            args.remove(0);
            return cmd_retention_review(&args);
        }
        Some("run-triage") => {
            args.remove(0);
            return cmd_run_triage(&args);
        }
        Some("run-wait") => {
            args.remove(0);
            return cmd_run_wait(&args);
        }
        Some("run-poll") => {
            args.remove(0);
            return cmd_run_poll(&args);
        }
        Some("receipt") => {
            args.remove(0);
            return cmd_receipt(&args);
        }
        Some("council-run") => {
            args.remove(0);
            return cmd_council_run(&args);
        }
        Some("help" | "--help" | "-h") => {
            println!(
                "agent-graph-operator — operator channel + MCP convenience client\n\
                 \n\
                 Operator mode:\n\
                 \x20 agent-graph-operator [--non-interactive] <action> <resource> <digest> <nonce> [state] [reason]\n\
                 \x20   actions: set_graph_retention approve_graph_deletion delete_graph\n\
                 \x20            clear_execution_lineage purge_graph purge_run\n\
                 \x20   socket resolves: AGENT_GRAPH_OPERATOR_SOCKET -> run/status.json -> default\n\
                 \n\
                 MCP convenience:\n\
                 \x20 status | retention-review [limit] | run-triage [limit]\n\
                 \x20 run-wait <run_id> [timeout_ms] | run-poll <run_id> | receipt <run_id>\n\
                 \x20 council-run <spec.json>"
            );
            return Ok(());
        }
        _ => {}
    }

    // ── Legacy operator frame mode ────────────────────────────────────────
    let socket = args
        .first()
        .cloned()
        .filter(|a| {
            !a.starts_with('-') && PathBuf::from(a).extension().is_none() && a.contains('/')
        })
        .unwrap_or_else(resolve_operator_socket);
    let rest: Vec<String> = if socket.contains('/') && !args.is_empty() && args[0] == socket {
        args.into_iter().skip(1).collect()
    } else {
        args
    };
    let action_text = rest
        .first()
        .cloned()
        .unwrap_or_else(|| "delete_graph".into());
    let action: OperatorAction = serde_json::from_str(&format!("\"{action_text}\""))?;
    let resource_id = rest.get(1).ok_or("missing resource ID")?.clone();
    let expected_state_digest = rest.get(2).ok_or("missing expected state digest")?.clone();
    let nonce = rest.get(3).ok_or("missing nonce")?.clone();
    let state = rest.get(4).cloned();
    let reason = rest.get(5).cloned();

    if !non_interactive {
        eprint!("Authorize {action_text} on {resource_id}? type 'yes': ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if answer.trim() != "yes" {
            return Err("operator decision cancelled".into());
        }
    }

    let decision_material = match (state, reason) {
        (Some(state), Some(reason)) => {
            Some(serde_json::json!({"state": state, "reason": reason}).to_string())
        }
        (Some(state), None) => Some(serde_json::json!({"state": state}).to_string()),
        _ => None,
    };
    let resource_kind = if matches!(action, OperatorAction::PurgeRun) {
        "run".to_string()
    } else {
        "graph".to_string()
    };
    let frame = OperatorFrame {
        protocol: PROTOCOL.into(),
        request_id: format!("cli-{nonce}"),
        action,
        resource_kind,
        resource_id,
        expected_state_digest,
        nonce,
        issued_at: (Utc::now() - Duration::seconds(1)).to_rfc3339(),
        expires_at: (Utc::now() + Duration::minutes(1)).to_rfc3339(),
        decision_material,
    };
    let body = serde_json::to_vec(&frame)?;
    let mut stream = UnixStream::connect(&socket)?;
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > 1024 * 1024 {
        return Err("invalid operator response frame".into());
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;
    let response: OperatorResponse = serde_json::from_slice(&payload)?;
    if !response.ok {
        return Err(response
            .error_code
            .unwrap_or_else(|| "operator request failed".into())
            .into());
    }
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}
