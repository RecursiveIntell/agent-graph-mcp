//! OS-authenticated operator authority primitives. JSON caller labels are never authority.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorAction {
    Approve,
    Reject,
    DeleteGraph,
    SetGraphRetention,
    ApproveGraphDeletion,
    ClearExecutionLineage,
    PurgeGraph,
    PromoteTemplate,
    Migrate,
    Install,
    DecideApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedOperator {
    pub uid: u32,
    pub gid: u32,
    pub action: OperatorAction,
    pub resource_kind: String,
    pub resource_id: String,
    pub expected_state_digest: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationReceipt {
    pub receipt_id: String,
    pub request_digest: String,
    pub action: OperatorAction,
    pub resource: String,
    pub state_digest: String,
    pub operator_uid: u32,
    pub daemon_instance_id: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCredentials {
    pub uid: u32,
    pub gid: u32,
}

/// Obtain credentials from the kernel-owned Unix socket peer context.
pub async fn peer_credentials(stream: &UnixStream) -> std::io::Result<PeerCredentials> {
    let c = stream.peer_cred()?;
    Ok(PeerCredentials {
        uid: c.uid(),
        gid: c.gid(),
    })
}

pub fn validate_window(op: &AuthenticatedOperator, now: DateTime<Utc>) -> Result<(), &'static str> {
    if op.nonce.is_empty() {
        return Err("AUTHORIZATION_NONCE_REQUIRED");
    }
    if op.expires_at <= op.issued_at || op.expires_at <= now {
        return Err("AUTHORIZATION_EXPIRED");
    }
    if op.issued_at > now {
        return Err("AUTHORIZATION_NOT_YET_VALID");
    }
    Ok(())
}
