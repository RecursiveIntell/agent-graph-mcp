//! Provider-path health tracking (B2).
//!
//! The daemon probes the provider endpoint (TCP connect + minimal HTTP GET)
//! on an interval and records consecutive failures. The shared state is
//! surfaced through `graph_status` so operators can distinguish "provider
//! path is down" (F1 class: silent `llm_calls:0` failures for days) from
//! healthy operation. This is detection/surfacing only — failover switching
//! is deliberately out of scope for v1.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ProviderHealth {
    healthy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    consecutive_failures: std::sync::Arc<AtomicU64>,
    last_check_ms: std::sync::Arc<AtomicU64>,
    last_error: std::sync::Arc<Mutex<Option<String>>>,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderHealth {
    pub fn new() -> Self {
        Self {
            healthy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            consecutive_failures: std::sync::Arc::new(AtomicU64::new(0)),
            last_check_ms: std::sync::Arc::new(AtomicU64::new(0)),
            last_error: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    pub fn record_success(&self) {
        self.healthy.store(true, Ordering::SeqCst);
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.last_check_ms.store(now_ms(), Ordering::SeqCst);
        if let Ok(mut err) = self.last_error.lock() {
            *err = None;
        }
    }

    pub fn record_failure(&self, error: impl Into<String>) {
        self.healthy.store(false, Ordering::SeqCst);
        self.consecutive_failures.fetch_add(1, Ordering::SeqCst);
        self.last_check_ms.store(now_ms(), Ordering::SeqCst);
        if let Ok(mut err) = self.last_error.lock() {
            *err = Some(error.into());
        }
    }

    /// JSON surface for `graph_status`. `total_failures` is optional context
    /// from callers that observe run-level provider failures.
    pub fn as_json(&self) -> serde_json::Value {
        let last_error = self.last_error.lock().ok().and_then(|e| e.clone());
        serde_json::json!({
            "healthy": self.healthy.load(Ordering::SeqCst),
            "consecutive_failures": self.consecutive_failures.load(Ordering::SeqCst),
            "last_check_ms": self.last_check_ms.load(Ordering::SeqCst),
            "last_error": last_error,
            "probe": "tcp_connect_plus_http_get",
            "failover": "not_configured_v1",
        })
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Probe the provider endpoint with a bounded TCP connect plus a minimal HTTP
/// GET. Any HTTP response (even 4xx) means the path is alive; connect failure
/// means the F1 class of "proxy process is dead". Returns Ok(()) when alive.
pub fn probe_base_url(base_url: &str, timeout_ms: u64) -> Result<(), String> {
    let (host, port) = parse_host_port(base_url)?;
    let addr = format!("{host}:{port}");
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let socket_addr: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("invalid address {addr}: {e}"))?;
    let mut stream = std::net::TcpStream::connect_timeout(&socket_addr, timeout)
        .map_err(|e| format!("tcp connect {addr} failed: {e}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    let request = format!("GET /v1/models HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    use std::io::Write;
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    use std::io::Read;
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("http probe read failed: {e}"))?;
    if n == 0 {
        return Err("http probe returned empty response".into());
    }
    Ok(())
}

fn parse_host_port(base_url: &str) -> Result<(String, u16), String> {
    let stripped = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let authority = stripped.split('/').next().unwrap_or(stripped);
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| format!("invalid port in {authority}"))?,
        ),
        None => (authority.to_string(), 80),
    };
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_port_variants() {
        assert_eq!(
            parse_host_port("http://127.0.0.1:1780/v1").unwrap(),
            ("127.0.0.1".into(), 1780)
        );
        assert_eq!(
            parse_host_port("http://localhost:11434").unwrap(),
            ("localhost".into(), 11434)
        );
        assert_eq!(
            parse_host_port("http://example.com").unwrap(),
            ("example.com".into(), 80)
        );
    }

    #[test]
    fn health_transitions() {
        let h = ProviderHealth::new();
        assert!(h.as_json()["healthy"].as_bool().unwrap());
        h.record_failure("tcp connect failed");
        h.record_failure("tcp connect failed");
        h.record_failure("tcp connect failed");
        let j = h.as_json();
        assert!(!j["healthy"].as_bool().unwrap());
        assert_eq!(j["consecutive_failures"].as_u64().unwrap(), 3);
        assert_eq!(j["last_error"].as_str(), Some("tcp connect failed"));
        h.record_success();
        let j2 = h.as_json();
        assert!(j2["healthy"].as_bool().unwrap());
        assert_eq!(j2["consecutive_failures"].as_u64().unwrap(), 0);
        assert!(j2["last_error"].is_null());
    }
}
