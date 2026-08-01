use agent_graph_mcp::operator::OperatorService;
use agent_graph_mcp::operator_auth::{
    validate_window, AuthenticatedOperator, OperatorAction, PeerCredentials,
};
use agent_graph_mcp::operator_ipc::{validate, OperatorFrame, PROTOCOL};
use agent_graph_mcp::store::PersistentStore;
use chrono::{Duration, Utc};
use serde_json::{json, Value};

#[test]
fn missing_or_expired_nonce_fails_closed() {
    let now = Utc::now();
    let op = AuthenticatedOperator {
        uid: 1,
        gid: 1,
        action: OperatorAction::Approve,
        resource_kind: "approval".into(),
        resource_id: "a".into(),
        expected_state_digest: "d".into(),
        nonce: String::new(),
        issued_at: now - Duration::seconds(1),
        expires_at: now + Duration::seconds(1),
    };
    assert_eq!(
        validate_window(&op, now),
        Err("AUTHORIZATION_NONCE_REQUIRED")
    );
}

#[test]
fn operator_protocol_rejects_wrong_version() {
    let frame = OperatorFrame {
        protocol: "agent_graph.operator.v0".into(),
        request_id: "r".into(),
        action: OperatorAction::Reject,
        resource_kind: "approval".into(),
        resource_id: "a".into(),
        expected_state_digest: "d".into(),
        nonce: "n".into(),
        issued_at: "".into(),
        expires_at: "".into(),
        decision_material: None,
    };
    assert_eq!(validate(&frame), Err("OPERATOR_PROTOCOL_UNSUPPORTED"));
    assert_eq!(PROTOCOL, "agent_graph.operator.v1");
}

fn frame(
    action: OperatorAction,
    graph: &str,
    digest: String,
    nonce: &str,
    material: Option<Value>,
) -> OperatorFrame {
    OperatorFrame {
        protocol: PROTOCOL.into(),
        request_id: format!("request-{nonce}"),
        action,
        resource_kind: "graph".into(),
        resource_id: graph.into(),
        expected_state_digest: digest,
        nonce: nonce.into(),
        issued_at: (Utc::now() - Duration::seconds(1)).to_rfc3339(),
        expires_at: (Utc::now() + Duration::minutes(1)).to_rfc3339(),
        decision_material: material.map(|v| v.to_string()),
    }
}

#[test]
fn operator_retention_is_authorized_idempotent_and_history_preserving() {
    let dir = tempfile::tempdir().expect("temp store");
    let store = PersistentStore::open(dir.path()).expect("open store");
    store
        .save_graph("operator-graph", r#"{"nodes":[],"edges":[]}"#, "v1", false)
        .expect("save graph");
    store
        .save_graph(
            "operator-graph",
            r#"{"nodes":[],"edges":[{"from":"a","to":"b"}]}"#,
            "v2",
            true,
        )
        .expect("save second graph version");
    let service = OperatorService::new(
        store.clone(),
        [4242_u32].into_iter().collect(),
        "daemon-test".into(),
    );
    let peer = PeerCredentials {
        uid: 4242,
        gid: 4242,
    };

    let digest = store
        .graph_retention_review(Some("operator-graph"), None, 1)
        .unwrap()[0]
        .state_digest
        .clone();
    let candidate = frame(
        OperatorAction::SetGraphRetention,
        "operator-graph",
        digest,
        "nonce-1",
        Some(json!({"state":"delete_candidate","reason":"test review"})),
    );
    let first = service.handle(peer.clone(), candidate.clone());
    assert!(first.ok, "{first:?}");
    let replay = service.handle(peer.clone(), candidate);
    assert_eq!(replay.receipt_id, first.receipt_id);

    let digest = store
        .graph_retention_review(Some("operator-graph"), None, 1)
        .unwrap()[0]
        .state_digest
        .clone();
    let approval = service.handle(
        peer.clone(),
        frame(
            OperatorAction::ApproveGraphDeletion,
            "operator-graph",
            digest,
            "nonce-2",
            None,
        ),
    );
    assert!(approval.ok, "{approval:?}");

    let digest = store
        .graph_retention_review(Some("operator-graph"), None, 1)
        .unwrap()[0]
        .state_digest
        .clone();
    let deletion = service.handle(
        peer,
        frame(
            OperatorAction::DeleteGraph,
            "operator-graph",
            digest,
            "nonce-3",
            None,
        ),
    );
    assert!(deletion.ok, "{deletion:?}");
    assert!(store.graph_is_tombstoned("operator-graph").unwrap());
    assert_eq!(
        store.list_graph_versions("operator-graph").unwrap(),
        vec!["v1", "v2"]
    );
    assert!(!store.graph_execution_allowed("operator-graph").unwrap());
}

#[test]
fn operator_rejects_unauthorized_peer_and_stale_digest_without_mutation() {
    let dir = tempfile::tempdir().expect("temp store");
    let store = PersistentStore::open(dir.path()).expect("open store");
    store
        .save_graph("guarded", r#"{"nodes":[],"edges":[]}"#, "v1", false)
        .expect("save graph");
    let service = OperatorService::new(
        store.clone(),
        [4242_u32].into_iter().collect(),
        "daemon-test".into(),
    );
    let stale = frame(
        OperatorAction::SetGraphRetention,
        "guarded",
        "stale".into(),
        "nonce-x",
        Some(json!({"state":"archived"})),
    );
    assert_eq!(
        service
            .handle(PeerCredentials { uid: 7, gid: 7 }, stale.clone())
            .error_code
            .as_deref(),
        Some("OPERATOR_PEER_UNAUTHORIZED")
    );
    assert_eq!(
        service
            .handle(
                PeerCredentials {
                    uid: 4242,
                    gid: 4242
                },
                stale
            )
            .error_code
            .as_deref(),
        Some("AUTHORIZATION_STATE_STALE")
    );
    assert_eq!(
        store
            .graph_retention_review(Some("guarded"), None, 1)
            .unwrap()[0]
            .state,
        "active"
    );
}
