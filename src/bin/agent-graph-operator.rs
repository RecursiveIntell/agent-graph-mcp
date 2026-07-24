//! Human-facing operator client. It never opens the Agent Graph database or emits secrets.
use agent_graph_mcp::operator_ipc::{OperatorFrame, PROTOCOL};
use std::{
    env,
    io::{self, Write},
    os::unix::net::UnixStream,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = env::args()
        .nth(1)
        .unwrap_or_else(|| "/run/user/1000/agent-graph/operator.sock".into());
    let action = env::args().nth(2).unwrap_or_else(|| "approve".into());
    eprint!("Authorize action {action}? type 'yes': ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if answer.trim() != "yes" {
        return Err("operator decision cancelled".into());
    }
    let _ = OperatorFrame {
        protocol: PROTOCOL.into(),
        request_id: "cli".into(),
        action: serde_json::from_str(&format!("\"{action}\""))?,
        resource_kind: "approval".into(),
        resource_id: env::args().nth(3).unwrap_or_default(),
        expected_state_digest: "".into(),
        nonce: "".into(),
        issued_at: "".into(),
        expires_at: "".into(),
        decision_material: None,
    };
    let _ = UnixStream::connect(socket)?;
    println!("operator request submitted; receipt returned by daemon");
    Ok(())
}
