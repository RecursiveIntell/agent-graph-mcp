use agent_graph_mcp::transport::{bound_json_response, MAX_FRAME};
use serde_json::{json, Value};

#[test]
fn small_responses_are_byte_exact() {
    let payload = br#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#;
    assert_eq!(bound_json_response(payload).unwrap().as_ref(), payload);
}

#[test]
fn large_response_becomes_correlated_typed_error_not_a_large_frame() {
    for id in [json!(17), json!("request-utf8-λ")] {
        let payload = serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":id,"result":{"secret_output":"x".repeat(MAX_FRAME)}}),
        )
        .unwrap();
        let frame = bound_json_response(&payload).unwrap();
        assert!(frame.len() < 2048);
        let reply: Value = serde_json::from_slice(frame.as_ref()).unwrap();
        assert_eq!(reply["id"], id);
        assert_eq!(reply["error"]["data"]["code"], "RESPONSE_TOO_LARGE");
        assert!(reply.get("result").is_none());
        assert!(!String::from_utf8_lossy(frame.as_ref()).contains("secret_output"));
    }
}

#[test]
fn invalid_large_notifications_do_not_invent_a_correlation_id() {
    let payload = serde_json::to_vec(
        &json!({"jsonrpc":"2.0","method":"notification","params":"x".repeat(MAX_FRAME)}),
    )
    .unwrap();
    assert!(bound_json_response(&payload).is_err());
    assert!(bound_json_response(&vec![b'x'; MAX_FRAME + 1]).is_err());
}

#[test]
fn limit_is_unchanged_and_a_small_response_after_large_remains_valid() {
    assert_eq!(MAX_FRAME, 1024 * 1024);
    let large = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"result":"x".repeat(MAX_FRAME)}))
        .unwrap();
    let first = bound_json_response(&large).unwrap();
    let small = br#"{"jsonrpc":"2.0","id":2,"result":"next"}"#;
    let second = bound_json_response(small).unwrap();
    assert!(first.len() < MAX_FRAME);
    assert_eq!(second.as_ref(), small);
}
