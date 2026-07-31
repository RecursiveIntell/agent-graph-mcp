use agent_graph_mcp::codex_app_server::{collect_text, is_terminal_notification, run_turn};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[test]
fn collects_stream_deltas_and_ignores_non_terminal_events() {
    let events = vec![
        json!({"method":"thread/started","params":{}}),
        json!({"method":"item/agentMessage/delta","params":{"delta":"hello "}}),
        json!({"method":"item/agentMessage/delta","params":{"delta":"world"}}),
        json!({"method":"thread/tokenUsage/updated","params":{}}),
        json!({"method":"turn/completed","params":{"turn":{"status":"completed"}}}),
    ];
    assert_eq!(collect_text(&events), "hello world");
    assert!(!is_terminal_notification(&events[0]));
    assert!(is_terminal_notification(&events[4]));
}

#[test]
fn falls_back_to_completed_agent_message_when_delta_is_absent() {
    let events = vec![json!({
        "method":"item/completed",
        "params":{"item":{"type":"agentMessage","text":"final answer"}}
    })];
    assert_eq!(collect_text(&events), "final answer");
}

#[test]
fn runs_complete_turn_and_collects_text_from_a_stdio_app_server() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("fake-codex");
    fs::write(
        &fake,
        r##"#!/bin/sh
read line
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{}}'
read line
read line
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-1"}}}'
read line
printf '%s\n' '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-1"}}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"delta":"safe result"}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn-1"}}}'
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();

    let text = run_turn(
        fake.to_str().unwrap(),
        "gpt-5.6-luna",
        dir.path(),
        "hello",
        Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(text, "safe result");
}
