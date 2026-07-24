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
