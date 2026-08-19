use ri_agent_graph::error::AgentGraphError;

#[test]
fn other_error_variant_is_string_bounded() {
    let err = AgentGraphError::Other("wrapped diagnostics for local debugging".to_string());
    assert_eq!(err.kind(), "other");
    assert_eq!(
        format!("{}", err),
        "wrapped diagnostics for local debugging"
    );
}

#[test]
fn error_kind_matches_other_variant_without_anyhow_bridge() {
    let err = AgentGraphError::Other("no external bridge".to_string());
    assert_eq!(err.kind(), "other");
}
