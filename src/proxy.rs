//! Framing and stdio/Unix transport helpers for the thin proxy.
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Debug)]
#[allow(dead_code)]
pub enum ProxyError {
    DaemonUnavailable,
    FrameTooLarge,
    Io(io::Error),
}

impl From<io::Error> for ProxyError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn connect(path: &std::path::Path) -> Result<UnixStream, ProxyError> {
    connect_timeout(path, 2000)
}

pub fn connect_timeout(path: &std::path::Path, timeout_ms: u64) -> Result<UnixStream, ProxyError> {
    let stream = UnixStream::connect(path).map_err(|_| ProxyError::DaemonUnavailable)?;
    // This bounded read is only for MCP notifications, which deliberately do
    // not receive daemon responses. The proxy clears it per request before a
    // potentially long graph execution or wait response.
    stream.set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)))?;
    Ok(stream)
}

pub fn read_frame<R: Read>(r: &mut R) -> Result<Vec<u8>, ProxyError> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n)?;
    let len = u32::from_be_bytes(n) as usize;
    if len > MAX_FRAME {
        return Err(ProxyError::FrameTooLarge);
    }
    let mut b = vec![0; len];
    r.read_exact(&mut b)?;
    Ok(b)
}

pub fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> Result<(), ProxyError> {
    if payload.len() > MAX_FRAME {
        return Err(ProxyError::FrameTooLarge);
    }
    w.write_all(&(payload.len() as u32).to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()?;
    Ok(())
}
