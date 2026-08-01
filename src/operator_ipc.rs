//! Separate, versioned operator-only IPC framing.
use crate::operator_auth::OperatorAction;
use serde::{Deserialize, Serialize};

pub const PROTOCOL: &str = "agent_graph.operator.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorFrame {
    pub protocol: String,
    pub request_id: String,
    pub action: OperatorAction,
    pub resource_kind: String,
    pub resource_id: String,
    pub expected_state_digest: String,
    pub nonce: String,
    pub issued_at: String,
    pub expires_at: String,
    pub decision_material: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorResponse {
    pub protocol: String,
    pub ok: bool,
    pub error_code: Option<String>,
    pub receipt_id: Option<String>,
}

pub fn validate(frame: &OperatorFrame) -> Result<(), &'static str> {
    if frame.protocol != PROTOCOL {
        return Err("OPERATOR_PROTOCOL_UNSUPPORTED");
    }
    if frame.request_id.is_empty() || frame.resource_id.is_empty() {
        return Err("OPERATOR_INVALID_REQUEST");
    }
    if frame.nonce.is_empty() {
        return Err("AUTHORIZATION_NONCE_REQUIRED");
    }
    Ok(())
}
