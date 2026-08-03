//! Human-facing operator client. It never opens the Agent Graph database.
use agent_graph_mcp::operator_auth::OperatorAction;
use agent_graph_mcp::operator_ipc::{OperatorFrame, OperatorResponse, PROTOCOL};
use chrono::{Duration, Utc};
use std::{
    env,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let socket = args
        .next()
        .unwrap_or_else(|| "/run/user/1000/agent-graph/operator.sock".into());
    let action_text = args.next().unwrap_or_else(|| "delete_graph".into());
    let action: OperatorAction = serde_json::from_str(&format!("\"{action_text}\""))?;
    let resource_id = args.next().ok_or("missing graph ID")?;
    let expected_state_digest = args.next().ok_or("missing expected state digest")?;
    let nonce = args.next().ok_or("missing nonce")?;
    let state = args.next();
    let reason = args.next();

    eprint!("Authorize {action_text} on {resource_id}? type 'yes': ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if answer.trim() != "yes" {
        return Err("operator decision cancelled".into());
    }

    let decision_material = match (state, reason) {
        (Some(state), Some(reason)) => {
            Some(serde_json::json!({"state": state, "reason": reason}).to_string())
        }
        (Some(state), None) => Some(serde_json::json!({"state": state}).to_string()),
        _ => None,
    };
    let frame = OperatorFrame {
        protocol: PROTOCOL.into(),
        request_id: format!("cli-{}", nonce),
        action,
        resource_kind: "graph".into(),
        resource_id,
        expected_state_digest,
        nonce,
        issued_at: (Utc::now() - Duration::seconds(1)).to_rfc3339(),
        expires_at: (Utc::now() + Duration::minutes(1)).to_rfc3339(),
        decision_material,
    };
    let body = serde_json::to_vec(&frame)?;
    let mut stream = UnixStream::connect(socket)?;
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
