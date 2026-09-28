//! Bounded length-prefixed transport shared by the daemon and proxy.
use std::io::{self, Read, Write};
use tokio::io::{AsyncWrite, AsyncWriteExt};
pub const MAX_FRAME: usize = 1024 * 1024;

/// Preserve small replies exactly and replace oversized correlated JSON-RPC
/// responses with a bounded error. This does not truncate canonical artifacts
/// or enlarge the wire contract. Uncorrelated/invalid messages fail closed.
pub fn bound_json_response(payload: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, FrameError> {
    if payload.len() <= MAX_FRAME {
        return Ok(std::borrow::Cow::Borrowed(payload));
    }
    let value: serde_json::Value =
        serde_json::from_slice(payload).map_err(|_| FrameError::TooLarge)?;
    let id = value
        .get("id")
        .filter(|id| id.is_string() || id.is_number());
    if value.get("jsonrpc").and_then(serde_json::Value::as_str) != Some("2.0")
        || value.get("method").is_some()
        || (value.get("result").is_none() && value.get("error").is_none())
    {
        return Err(FrameError::TooLarge);
    }
    let id = id.ok_or(FrameError::TooLarge)?;
    let reply = serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0", "id": id,
        "error": {
            "code": -32001,
            "message": "RESPONSE_TOO_LARGE: use compact status or paged graph_run_artifact reads",
            "data": {"code": "RESPONSE_TOO_LARGE", "max_response_bytes": MAX_FRAME}
        }
    }))
    .map_err(|_| FrameError::TooLarge)?;
    if reply.len() > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    Ok(std::borrow::Cow::Owned(reply))
}
#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    TooLarge,
}
impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::Io(e) => write!(f, "transport io error: {e}"),
            FrameError::TooLarge => write!(f, "frame exceeds maximum size"),
        }
    }
}

impl std::error::Error for FrameError {}
pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut h = [0; 4];
    r.read_exact(&mut h)?;
    let n = u32::from_be_bytes(h) as usize;
    if n > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b)?;
    Ok(b)
}
pub fn write_frame<W: Write>(w: &mut W, b: &[u8]) -> Result<(), FrameError> {
    if b.len() > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    w.write_all(&(b.len() as u32).to_be_bytes())?;
    w.write_all(b)?;
    w.flush()?;
    Ok(())
}

pub async fn write_frame_async<W: AsyncWrite + Unpin>(
    w: &mut W,
    b: &[u8],
) -> Result<(), io::Error> {
    if b.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame exceeds maximum size",
        ));
    }
    w.write_all(&(b.len() as u32).to_be_bytes()).await?;
    w.write_all(b).await?;
    w.flush().await
}

// ── B5: proxy↔daemon protocol handshake ─────────────────────────────────────

/// Current wire protocol version. Both binaries must agree; a mismatch is a
/// deployment hazard (silent protocol failure / relay restart loop).
pub const PROTOCOL_VERSION: u64 = 1;

/// Return the build identity shared by the proxy and daemon binaries.
pub fn build_identity() -> serde_json::Value {
    serde_json::json!({
        "crate_version": env!("CARGO_PKG_VERSION"),
        "source_revision": env!("AGENT_GRAPH_BUILD_GIT_SHA"),
        "git_dirty": env!("AGENT_GRAPH_BUILD_GIT_DIRTY"),
        "source_content_sha256": env!("AGENT_GRAPH_BUILD_SOURCE_SHA256"),
        "source_scope": "build.rs,Cargo.toml,Cargo.lock(if present),src/**",
        "identity_policy": "observable_build_identity; protocol_version_controls_compatibility; deployment_manifest_controls_artifact_admission",
        "cargo_lock_sha256": env!("AGENT_GRAPH_BUILD_LOCK_SHA256"),
    })
}

/// Build the proxy's hello frame (sent as the first frame of a connection).
pub fn hello_frame() -> Vec<u8> {
    let mut build = build_identity();
    if let Some(object) = build.as_object_mut() {
        object.insert("protocol_version".into(), PROTOCOL_VERSION.into());
    }
    serde_json::json!({"hello": build}).to_string().into_bytes()
}

/// Proxy side: interpret the daemon's hello reply. Ok(()) means versions agree;
/// Err(reason) means the daemon rejected the handshake (or replied oddly).
pub fn parse_hello_response(bytes: &[u8]) -> Result<(), String> {
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("not a hello response: {e}"))?;
    if let Some(hello) = v.get("hello") {
        if hello
            .get("protocol_version")
            .and_then(|value| value.as_u64())
            != Some(PROTOCOL_VERSION)
        {
            return Err("daemon protocol version does not match proxy".into());
        }
        Ok(())
    } else if let Some(err) = v.get("hello_error") {
        Err(format!("daemon rejected handshake: {err}"))
    } else {
        Err("unexpected hello response shape".into())
    }
}

/// Daemon side: interpret the first frame of a connection.
/// - `None`: not a hello (legacy client) — the caller must forward the frame
///   into the MCP bridge unchanged.
/// - `Some(Ok(reply))`: valid hello — caller sends `reply` and proceeds.
/// - `Some(Err(reason))`: protocol mismatch — caller sends a hello_error
///   frame and drops the connection (fail fast instead of silent protocol failure).
pub fn interpret_hello(frame: &[u8]) -> Option<Result<Vec<u8>, String>> {
    let v: serde_json::Value = serde_json::from_slice(frame).ok()?;
    let hello = v.get("hello")?;
    let proto = hello
        .get("protocol_version")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let mismatch_reason = if proto != PROTOCOL_VERSION {
        Some("PROTOCOL_VERSION_MISMATCH")
    } else {
        None
    };
    if let Some(reason) = mismatch_reason {
        let err = serde_json::json!({
            "hello_error": {
                "protocol_version": PROTOCOL_VERSION,
                "build": build_identity(),
                "reason": reason,
            }
        });
        return Some(Err(err.to_string()));
    }
    let mut reply = build_identity();
    if let Some(object) = reply.as_object_mut() {
        object.insert("protocol_version".into(), PROTOCOL_VERSION.into());
    }
    Some(Ok(serde_json::json!({"hello": reply})
        .to_string()
        .into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_roundtrip() {
        let frame = hello_frame();
        let parsed: serde_json::Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(
            parsed["hello"]["protocol_version"].as_u64(),
            Some(PROTOCOL_VERSION)
        );
        // Daemon accepts it.
        match interpret_hello(&frame) {
            Some(Ok(reply)) => assert!(parse_hello_response(&reply).is_ok()),
            other => panic!("expected Some(Ok), got {other:?}"),
        }
    }

    #[test]
    fn version_mismatch_rejected() {
        let bad = serde_json::json!({
            "hello": {"protocol_version": 999, "crate_version": "0.0.0"}
        })
        .to_string()
        .into_bytes();
        match interpret_hello(&bad) {
            Some(Err(reason)) => {
                assert!(reason.contains("PROTOCOL_VERSION_MISMATCH"));
                // Proxy side surfaces the same mismatch.
                let reply = serde_json::json!({
                    "hello_error": {"protocol_version": 1, "crate_version": "x", "reason": "PROTOCOL_VERSION_MISMATCH"}
                })
                .to_string();
                assert!(parse_hello_response(reply.as_bytes()).is_err());
            }
            other => panic!("expected Some(Err), got {other:?}"),
        }
    }

    #[test]
    fn build_identity_is_observable_without_changing_protocol_compatibility() {
        let bad = serde_json::json!({
            "hello": {
                "protocol_version": PROTOCOL_VERSION,
                "crate_version": env!("CARGO_PKG_VERSION"),
                "source_revision": "different-source",
                "cargo_lock_sha256": env!("AGENT_GRAPH_BUILD_LOCK_SHA256"),
            }
        })
        .to_string()
        .into_bytes();
        match interpret_hello(&bad) {
            Some(Ok(reply)) => {
                let response: serde_json::Value = serde_json::from_slice(&reply).unwrap();
                assert_ne!(response["hello"]["source_revision"], "different-source");
                assert!(parse_hello_response(&reply).is_ok());
                assert!(parse_hello_response(&bad).is_ok());
            }
            other => panic!("expected compatible protocol with observable identity, got {other:?}"),
        }
    }

    #[test]
    fn source_content_digest_is_present() {
        let build = build_identity();
        let digest = build["source_content_sha256"].as_str().unwrap_or("");
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 71);
    }

    #[test]
    fn proxy_rejects_wrong_protocol_even_when_build_matches() {
        let mut response: serde_json::Value = serde_json::from_slice(&hello_frame()).unwrap();
        response["hello"]["protocol_version"] = 999.into();
        assert!(parse_hello_response(response.to_string().as_bytes()).is_err());
        response["hello"]
            .as_object_mut()
            .unwrap()
            .remove("protocol_version");
        assert!(parse_hello_response(response.to_string().as_bytes()).is_err());
    }

    #[test]
    fn legacy_frame_not_hello() {
        // An MCP JSON-RPC message is not a hello — daemon must forward it.
        let mcp = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        assert!(interpret_hello(mcp).is_none());
        // Garbage bytes are also not a hello.
        assert!(interpret_hello(b"\x00\xffgarbage").is_none());
    }
}
