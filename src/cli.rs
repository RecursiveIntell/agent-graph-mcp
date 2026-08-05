//! Strict CLI argument parsing for agent-graph-mcp.
//!
//! Replaces the silent `_ => {}` pattern that allowed typos like `--dat-dir`
//! to launch with unsafe defaults. Every unknown flag, missing value, or
//! malformed URL must exit nonzero before MCP transport starts.

use std::path::PathBuf;

use crate::spec::{validate_max_graphs, DEFAULT_MAX_GRAPHS};

/// Parsed CLI configuration.
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub base_url: String,
    pub default_model: String,
    pub data_dir: Option<PathBuf>,
    pub integrity_key_path: Option<PathBuf>,
    pub require_integrity_key: bool,
    pub ephemeral: bool,
    pub max_graphs: usize,
    pub api_key: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_string(),
            default_model: "glm-5.2:cloud".to_string(),
            data_dir: None,
            integrity_key_path: None,
            require_integrity_key: false,
            ephemeral: false,
            max_graphs: DEFAULT_MAX_GRAPHS,
            api_key: None,
        }
    }
}

/// Typed CLI parse error. The caller must exit nonzero and must not start
/// MCP transport.
#[derive(Debug, Clone)]
pub struct CliError {
    pub message: String,
    pub exit_code: i32,
}

impl CliError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub socket: PathBuf,
    pub timeout_ms: u64,
}
pub fn parse_proxy_args(args: &[String]) -> Result<ProxyConfig, CliError> {
    let mut socket = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("agent-graph/mcp.sock");
    let mut timeout_ms = 2000;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" => {
                return Err(CliError {
                    message: "agent-graph-mcp [--socket PATH] [--connect-timeout-ms N]".into(),
                    exit_code: 0,
                });
            }
            "--version" => {
                return Err(CliError {
                    message: env!("CARGO_PKG_VERSION").into(),
                    exit_code: 0,
                });
            }
            "--socket" => {
                i += 1;
                socket = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| CliError::new("--socket requires a value"))?,
                );
            }
            "--connect-timeout-ms" => {
                i += 1;
                timeout_ms = args
                    .get(i)
                    .ok_or_else(|| CliError::new("--connect-timeout-ms requires a value"))?
                    .parse()
                    .map_err(|_| CliError::new("invalid timeout"))?;
            }
            "--data-dir" | "--integrity-key" | "--base-url" | "--model" | "--max-graphs" => {
                return Err(CliError::new("LEGACY_DIRECT_DURABLE_UNSUPPORTED"));
            }
            other => return Err(CliError::new(format!("unknown argument: '{other}'"))),
        }
        i += 1;
    }
    Ok(ProxyConfig { socket, timeout_ms })
}

///
/// Strict rules:
/// - Unknown flags are rejected.
/// - Flags requiring a value must have one.
/// - `--data-dir` implies durable mode; `--ephemeral` is explicit memory-only.
/// - `--require-integrity-key` without a readable key path is rejected at
///   parse time if the path is provided here; otherwise the caller validates
///   file existence/length after parsing.
/// - Provider URLs must be http or https.
pub fn parse_args(args: &[String]) -> Result<CliConfig, CliError> {
    let mut config = CliConfig::default();
    let mut iter = args.iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--base-url" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::new("--base-url requires a value"))?;
                validate_url(value)?;
                config.base_url = value.clone();
            }
            "--model" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::new("--model requires a value"))?;
                if value.is_empty() {
                    return Err(CliError::new("--model value must not be empty"));
                }
                config.default_model = value.clone();
            }
            "--data-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::new("--data-dir requires a value"))?;
                if value.is_empty() {
                    return Err(CliError::new("--data-dir value must not be empty"));
                }
                config.data_dir = Some(PathBuf::from(value));
            }
            "--integrity-key" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::new("--integrity-key requires a value"))?;
                if value.is_empty() {
                    return Err(CliError::new("--integrity-key value must not be empty"));
                }
                config.integrity_key_path = Some(PathBuf::from(value));
            }
            "--require-integrity-key" => {
                config.require_integrity_key = true;
            }
            "--ephemeral" => {
                config.ephemeral = true;
            }
            "--max-graphs" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::new("--max-graphs requires a value"))?;
                let max_graphs = value
                    .parse::<usize>()
                    .map_err(|_| CliError::new("--max-graphs must be an integer"))?;
                config.max_graphs = validate_max_graphs(max_graphs).map_err(CliError::new)?;
            }
            "--help" => {
                eprintln!("agent-graph-mcp [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!(
                    "  --base-url <url>         Provider URL (default: http://127.0.0.1:11434)"
                );
                eprintln!(
                    "  --model <name>           Default model for LLM nodes (default: glm-5.2:cloud)"
                );
                eprintln!("  --data-dir <path>        Persistent storage directory");
                eprintln!("  --integrity-key <path>   Integrity key file for durable mode");
                eprintln!(
                    "  --require-integrity-key  Fail startup if integrity key is missing/unreadable"
                );
                eprintln!("  --ephemeral              Explicit in-memory mode (no persistence)");
                eprintln!(
                    "  --max-graphs <n>         Registered graph capacity (1-1024; default: 64)"
                );
                eprintln!("  --help                   Show this help message");
                return Err(CliError {
                    message: String::new(),
                    exit_code: 0,
                });
            }
            _ => {
                return Err(CliError::new(format!(
                    "unknown argument: '{arg}' — use --help for usage"
                )));
            }
        }
    }

    // Cross-flag validation
    if config.ephemeral && config.data_dir.is_some() {
        return Err(CliError::new(
            "--ephemeral and --data-dir are mutually exclusive",
        ));
    }

    if config.require_integrity_key && config.data_dir.is_none() {
        return Err(CliError::new(
            "--require-integrity-key requires --data-dir (durable mode)",
        ));
    }

    Ok(config)
}

/// Validate that a URL has http or https scheme and a non-empty host.
fn validate_url(url: &str) -> Result<(), CliError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| CliError::new(format!("invalid URL (no scheme): {url}")))?;

    if scheme != "http" && scheme != "https" {
        return Err(CliError::new(format!(
            "unsupported URL scheme '{scheme}': only http and https are allowed"
        )));
    }

    let authority = rest.split_once('/').map(|(auth, _)| auth).unwrap_or(rest);
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);

    if host.is_empty() {
        return Err(CliError::new(format!("invalid URL (empty host): {url}")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(flags: &[&str]) -> Vec<String> {
        flags.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_unknown_flag_rejected() {
        let result = parse_args(&args(&["--dat-dir", "/secure/path"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unknown argument"));
    }

    #[test]
    fn test_missing_value_for_base_url() {
        let result = parse_args(&args(&["--base-url"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("requires a value"));
    }

    #[test]
    fn test_missing_value_for_model() {
        let result = parse_args(&args(&["--model"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("requires a value"));
    }

    #[test]
    fn test_missing_value_for_data_dir() {
        let result = parse_args(&args(&["--data-dir"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("requires a value"));
    }

    #[test]
    fn test_empty_model_value_rejected() {
        let result = parse_args(&args(&["--model", ""]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must not be empty"));
    }

    #[test]
    fn test_empty_data_dir_value_rejected() {
        let result = parse_args(&args(&["--data-dir", ""]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("must not be empty"));
    }

    #[test]
    fn test_non_http_url_rejected() {
        let result = parse_args(&args(&["--base-url", "ftp://example.com"]));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("unsupported URL scheme"));
    }

    #[test]
    fn test_no_scheme_url_rejected() {
        let result = parse_args(&args(&["--base-url", "example.com"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no scheme"));
    }

    #[test]
    fn test_empty_host_url_rejected() {
        let result = parse_args(&args(&["--base-url", "http://"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("empty host"));
    }

    #[test]
    fn test_valid_http_url_accepted() {
        let result = parse_args(&args(&["--base-url", "http://127.0.0.1:11434"]));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().base_url, "http://127.0.0.1:11434");
    }

    #[test]
    fn test_valid_https_url_accepted() {
        let result = parse_args(&args(&["--base-url", "https://api.openai.com"]));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().base_url, "https://api.openai.com");
    }

    #[test]
    fn test_url_with_credentials_stripped_in_validation() {
        let result = parse_args(&args(&["--base-url", "https://user:pass@host.com"]));
        assert!(result.is_ok()); // validation passes; runtime redaction is separate
    }

    #[test]
    fn test_ephemeral_and_data_dir_mutually_exclusive() {
        let result = parse_args(&args(&["--ephemeral", "--data-dir", "/tmp/test"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("mutually exclusive"));
    }

    #[test]
    fn test_require_integrity_key_without_data_dir_rejected() {
        let result = parse_args(&args(&["--require-integrity-key"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("requires --data-dir"));
    }

    #[test]
    fn test_require_integrity_key_with_data_dir_accepted() {
        let result = parse_args(&args(&[
            "--require-integrity-key",
            "--data-dir",
            "/tmp/test",
            "--integrity-key",
            "/tmp/key",
        ]));
        assert!(result.is_ok());
        assert!(result.unwrap().require_integrity_key);
    }

    #[test]
    fn test_help_returns_exit_zero() {
        let result = parse_args(&args(&["--help"]));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().exit_code, 0);
    }

    #[test]
    fn test_default_config_when_no_args() {
        let result = parse_args(&[]);
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.base_url, "http://127.0.0.1:11434");
        assert_eq!(config.default_model, "glm-5.2:cloud");
        assert!(config.data_dir.is_none());
        assert!(!config.ephemeral);
    }

    #[test]
    fn test_ephemeral_alone_accepted() {
        let result = parse_args(&args(&["--ephemeral"]));
        assert!(result.is_ok());
        assert!(result.unwrap().ephemeral);
    }

    #[test]
    fn test_integrity_key_path_recorded() {
        let result = parse_args(&args(&[
            "--data-dir",
            "/tmp/test",
            "--integrity-key",
            "/tmp/my-key",
        ]));
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(
            config.integrity_key_path,
            Some(PathBuf::from("/tmp/my-key"))
        );
    }

    #[test]
    fn test_max_graphs_records_configured_value() {
        let config = parse_args(&args(&["--max-graphs", "256"])).expect("valid limit");
        assert_eq!(config.max_graphs, 256);
    }

    #[test]
    fn test_zero_max_graphs_is_rejected() {
        let error = parse_args(&args(&["--max-graphs", "0"])).expect_err("zero is invalid");
        assert!(error.message.contains("between 1 and"));
    }

    #[test]
    fn test_excessive_max_graphs_is_rejected() {
        let error = parse_args(&args(&["--max-graphs", "1025"])).expect_err("hard cap");
        assert!(error.message.contains("between 1 and"));
    }
}
