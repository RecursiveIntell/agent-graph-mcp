//! Process-boundary test: proxy rejects legacy durable arguments.
use agent_graph_mcp::cli;

#[test]
fn proxy_rejects_data_dir() {
    let args: Vec<String> = vec!["--data-dir".into(), "/tmp/test".into()];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().message,
        "LEGACY_DIRECT_DURABLE_UNSUPPORTED"
    );
}

#[test]
fn proxy_rejects_integrity_key() {
    let args: Vec<String> = vec!["--integrity-key".into(), "/tmp/key".into()];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_err());
}

#[test]
fn proxy_rejects_base_url() {
    let args: Vec<String> = vec!["--base-url".into(), "http://localhost".into()];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_err());
}

#[test]
fn proxy_rejects_model() {
    let args: Vec<String> = vec!["--model".into(), "test-model".into()];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_err());
}

#[test]
fn proxy_accepts_socket() {
    let args: Vec<String> = vec!["--socket".into(), "/tmp/agent-graph/mcp.sock".into()];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_ok(), "socket arg should be accepted");
}

#[test]
fn proxy_accepts_connect_timeout() {
    let args: Vec<String> = vec![
        "--socket".into(),
        "/tmp/agent-graph/mcp.sock".into(),
        "--connect-timeout-ms".into(),
        "5000".into(),
    ];
    let result = cli::parse_proxy_args(&args);
    assert!(result.is_ok(), "connect-timeout-ms should be accepted");
}
