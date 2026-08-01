//! Daemon-owned Unix operator service. The client never opens SQLite.
use crate::{
    evidence::digest,
    operator_auth::{
        peer_credentials, validate_window, AuthenticatedOperator, OperatorAction, PeerCredentials,
    },
    operator_ipc::{validate, OperatorFrame, OperatorResponse, PROTOCOL},
    store::{
        OperatorRetentionError, OperatorRetentionRequest, OperatorRetentionResult, PersistentStore,
    },
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

const MAX_FRAME: usize = 1024 * 1024;

#[derive(Clone)]
pub struct OperatorService {
    store: PersistentStore,
    allowed_uids: Arc<BTreeSet<u32>>,
    daemon_instance_id: String,
}

impl OperatorService {
    pub fn new(
        store: PersistentStore,
        allowed_uids: BTreeSet<u32>,
        daemon_instance_id: String,
    ) -> Self {
        Self {
            store,
            allowed_uids: Arc::new(allowed_uids),
            daemon_instance_id,
        }
    }

    pub fn handle(&self, peer: PeerCredentials, frame: OperatorFrame) -> OperatorResponse {
        let error = |code: &str| OperatorResponse {
            protocol: PROTOCOL.into(),
            ok: false,
            error_code: Some(code.into()),
            receipt_id: None,
        };
        if let Err(code) = validate(&frame) {
            return error(code);
        }
        if frame.resource_kind != "graph" {
            return error("OPERATOR_RESOURCE_UNSUPPORTED");
        }
        if !self.allowed_uids.contains(&peer.uid) {
            return error("OPERATOR_PEER_UNAUTHORIZED");
        }
        let issued_at = match DateTime::parse_from_rfc3339(&frame.issued_at) {
            Ok(v) => v.with_timezone(&Utc),
            Err(_) => return error("OPERATOR_TIMESTAMP_INVALID"),
        };
        let expires_at = match DateTime::parse_from_rfc3339(&frame.expires_at) {
            Ok(v) => v.with_timezone(&Utc),
            Err(_) => return error("OPERATOR_TIMESTAMP_INVALID"),
        };
        let operator = AuthenticatedOperator {
            uid: peer.uid,
            gid: peer.gid,
            action: frame.action,
            resource_kind: frame.resource_kind.clone(),
            resource_id: frame.resource_id.clone(),
            expected_state_digest: frame.expected_state_digest.clone(),
            nonce: frame.nonce.clone(),
            issued_at,
            expires_at,
        };
        if let Err(code) = validate_window(&operator, Utc::now()) {
            return error(code);
        }
        if !matches!(
            frame.action,
            OperatorAction::SetGraphRetention
                | OperatorAction::ApproveGraphDeletion
                | OperatorAction::DeleteGraph
        ) {
            return error("OPERATOR_ACTION_UNSUPPORTED");
        }
        let material: Value = match frame.decision_material.as_deref() {
            Some(raw) => match serde_json::from_str(raw) {
                Ok(v) => v,
                Err(_) => return error("OPERATOR_DECISION_INVALID"),
            },
            None => json!({}),
        };
        let request_digest = digest(&json!({
            "protocol": frame.protocol, "request_id": frame.request_id, "action": frame.action,
            "resource_kind": frame.resource_kind, "resource_id": frame.resource_id,
            "expected_state_digest": frame.expected_state_digest, "nonce": frame.nonce,
            "issued_at": frame.issued_at, "expires_at": frame.expires_at,
            "decision_material": material, "peer_uid": peer.uid, "peer_gid": peer.gid,
        }));
        let request = OperatorRetentionRequest {
            request_digest,
            action: frame.action,
            graph_id: frame.resource_id,
            expected_state_digest: frame.expected_state_digest,
            nonce: frame.nonce,
            operator_uid: peer.uid,
            daemon_instance_id: self.daemon_instance_id.clone(),
            issued_at: operator.issued_at.to_rfc3339(),
            expires_at: operator.expires_at.to_rfc3339(),
            state: material
                .get("state")
                .and_then(Value::as_str)
                .map(str::to_owned),
            reason: material
                .get("reason")
                .and_then(Value::as_str)
                .map(str::to_owned),
            review_after: material
                .get("review_after")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        match self.store.apply_operator_retention(&request) {
            Ok(OperatorRetentionResult::Applied { receipt_id })
            | Ok(OperatorRetentionResult::Replayed { receipt_id }) => OperatorResponse {
                protocol: PROTOCOL.into(),
                ok: true,
                error_code: None,
                receipt_id: Some(receipt_id),
            },
            Err(err) => error(match err {
                OperatorRetentionError::StaleState => "AUTHORIZATION_STATE_STALE",
                OperatorRetentionError::NonceReplayed => "AUTHORIZATION_NONCE_REPLAYED",
                OperatorRetentionError::Tombstoned => "GRAPH_TOMBSTONED",
                OperatorRetentionError::Referenced => "GRAPH_REFERENCED",
                OperatorRetentionError::ReferencedBySubgraph => "GRAPH_REFERENCED_BY_SUBGRAPH",
                OperatorRetentionError::InvalidState => "RETENTION_STATE_INVALID",
                OperatorRetentionError::InvalidTransition => "RETENTION_TRANSITION_INVALID",
                OperatorRetentionError::NotFound => "GRAPH_NOT_FOUND",
                OperatorRetentionError::InvalidAction => "OPERATOR_ACTION_UNSUPPORTED",
                OperatorRetentionError::Persistence => "OPERATOR_PERSISTENCE_FAILED",
            }),
        }
    }
}

pub async fn serve_connection(stream: UnixStream, service: OperatorService) -> std::io::Result<()> {
    let peer = peer_credentials(&stream).await?;
    let (mut rx, mut tx) = stream.into_split();
    loop {
        let mut header = [0u8; 4];
        if rx.read_exact(&mut header).await.is_err() {
            return Ok(());
        }
        let len = u32::from_be_bytes(header) as usize;
        if len == 0 || len > MAX_FRAME {
            return Ok(());
        }
        let mut payload = vec![0u8; len];
        if rx.read_exact(&mut payload).await.is_err() {
            return Ok(());
        }
        let response = match serde_json::from_slice::<OperatorFrame>(&payload) {
            Ok(frame) => service.handle(peer.clone(), frame),
            Err(_) => OperatorResponse {
                protocol: PROTOCOL.into(),
                ok: false,
                error_code: Some("OPERATOR_FRAME_INVALID".into()),
                receipt_id: None,
            },
        };
        let body = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
        tx.write_all(&(body.len() as u32).to_be_bytes()).await?;
        tx.write_all(&body).await?;
        tx.flush().await?;
    }
}
