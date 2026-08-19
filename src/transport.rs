//! Bounded length-prefixed transport shared by the daemon and proxy.
use std::io::{self, Read, Write};
use tokio::io::{AsyncWrite, AsyncWriteExt};
pub const MAX_FRAME: usize = 1024 * 1024;
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

/// Build the proxy's hello frame (sent as the first frame of a connection).
pub fn hello_frame() -> Vec<u8> {
    serde_json::json!({
        "hello": {
            "protocol_version": PROTOCOL_VERSION,
            "crate_version": env!("CARGO_PKG_VERSION"),
        }
    })
    .to_string()
    .into_bytes()
}

/// Proxy side: interpret the daemon's hello reply. Ok(()) means versions agree;
/// Err(reason) means the daemon rejected the handshake (or replied oddly).
pub fn parse_hello_response(bytes: &[u8]) -> Result<(), String> {
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("not a hello response: {e}"))?;
    if v.get("hello").is_some() {
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
/// - `Some(Err(reason))`: version mismatch — caller sends a hello_error frame
///   and drops the connection (fail fast instead of silent protocol failure).
pub fn interpret_hello(frame: &[u8]) -> Option<Result<Vec<u8>, String>> {
    let v: serde_json::Value = serde_json::from_slice(frame).ok()?;
    let hello = v.get("hello")?;
    let proto = hello
        .get("protocol_version")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    if proto != PROTOCOL_VERSION {
        let err = serde_json::json!({
            "hello_error": {
                "protocol_version": PROTOCOL_VERSION,
                "crate_version": env!("CARGO_PKG_VERSION"),
                "reason": "PROTOCOL_VERSION_MISMATCH",
            }
        });
        return Some(Err(err.to_string()));
    }
    let reply = serde_json::json!({
        "hello": {
            "protocol_version": PROTOCOL_VERSION,
            "crate_version": env!("CARGO_PKG_VERSION"),
        }
    });
    Some(Ok(reply.to_string().into_bytes()))
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
    fn legacy_frame_not_hello() {
        // An MCP JSON-RPC message is not a hello — daemon must forward it.
        let mcp = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        assert!(interpret_hello(mcp).is_none());
        // Garbage bytes are also not a hello.
        assert!(interpret_hello(b"\x00\xffgarbage").is_none());
    }
}
