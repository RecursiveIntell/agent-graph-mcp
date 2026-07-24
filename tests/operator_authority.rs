use agent_graph_mcp::operator_auth::{validate_window, AuthenticatedOperator, OperatorAction};
use agent_graph_mcp::operator_ipc::{validate, OperatorFrame, PROTOCOL};
use chrono::{Duration, Utc};

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
