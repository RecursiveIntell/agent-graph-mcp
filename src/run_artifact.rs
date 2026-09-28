//! Bounded, read-only pages over canonical verified terminal run artifacts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The canonical artifact selected for a bounded read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// The verified terminal receipt JSON value.
    Receipt,
    /// The verified source bundle JSON value.
    Bundle,
    /// The output value named by the receipt's declared terminal state key.
    Output,
}

impl ArtifactKind {
    /// Returns the stable lowercase wire name for this artifact.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Receipt => "receipt",
            Self::Bundle => "bundle",
            Self::Output => "output",
        }
    }
}

/// Typed failures while selecting or paging a verified artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPageError {
    /// The requested page size is outside the supported range.
    InvalidLimit,
    /// The offset is past the artifact end or inside a UTF-8 code point.
    InvalidOffset,
    /// The artifact does not contain the declared output value.
    Unavailable,
    /// The supplied digest does not match the complete artifact bytes.
    DigestMismatch,
    /// The supplied identity does not match this run and artifact.
    ArtifactIdentityMismatch,
    /// Continuation pages require both prior-page identity values.
    ContinuationIdentityRequired,
    /// The page size cannot fit the next UTF-8 code point.
    LimitTooSmall,
    /// Canonical JSON serialization failed.
    Serialization,
}

/// A bounded UTF-8 page over one canonical terminal artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactPage {
    /// Page schema identifier.
    pub schema: &'static str,
    /// Run this page is bound to.
    pub run_id: String,
    /// Graph version recorded by the canonical receipt.
    pub graph_version: String,
    /// Selected artifact kind.
    pub artifact: String,
    /// Content encoding.
    pub encoding: &'static str,
    /// SHA-256 of the complete serialized artifact bytes.
    pub digest: String,
    /// Identity binding the run, version, artifact kind, and digest.
    pub artifact_id: String,
    /// Complete serialized artifact byte length.
    pub total_bytes: u64,
    /// Byte offset of this page.
    pub offset: u64,
    /// Byte offset for the next page, or null when complete.
    pub next_offset: Option<u64>,
    /// Whether this page reaches end of artifact.
    pub done: bool,
    /// UTF-8 page contents.
    pub data: String,
}

#[derive(Serialize)]
struct ArtifactIdentity<'a> {
    run_id: &'a str,
    graph_version: &'a str,
    artifact: &'a str,
    digest: &'a str,
}

/// Builds one bounded page from a canonical wrapper returned by the durable
/// terminal receipt verifier. This function does not perform storage access.
pub fn page_verified_artifact(
    wrapper: &Value,
    run_id: &str,
    artifact: ArtifactKind,
    offset: u64,
    limit: u64,
    expected_digest: Option<&str>,
    expected_artifact_id: Option<&str>,
) -> Result<ArtifactPage, ArtifactPageError> {
    if !(1..=16_384).contains(&limit) {
        return Err(ArtifactPageError::InvalidLimit);
    }
    if offset > 0 && (expected_digest.is_none() || expected_artifact_id.is_none()) {
        return Err(ArtifactPageError::ContinuationIdentityRequired);
    }
    let receipt = wrapper
        .get("receipt")
        .ok_or(ArtifactPageError::Unavailable)?;
    let bundle = wrapper
        .get("bundle")
        .ok_or(ArtifactPageError::Unavailable)?;
    let graph_version = receipt
        .get("graph_version")
        .and_then(Value::as_str)
        .or_else(|| {
            bundle
                .pointer("/payload/graph_version")
                .and_then(Value::as_str)
        })
        .ok_or(ArtifactPageError::Unavailable)?;
    let value = match artifact {
        ArtifactKind::Receipt => receipt,
        ArtifactKind::Bundle => bundle,
        ArtifactKind::Output => {
            let key = receipt
                .pointer("/terminal_output/state_key")
                .and_then(Value::as_str)
                .ok_or(ArtifactPageError::Unavailable)?;
            bundle
                .pointer("/payload/output")
                .and_then(Value::as_object)
                .and_then(|output| output.get(key))
                .ok_or(ArtifactPageError::Unavailable)?
        }
    };
    let bytes = serde_json::to_vec(value).map_err(|_| ArtifactPageError::Serialization)?;
    let digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    if expected_digest.is_some_and(|expected| expected != digest) {
        return Err(ArtifactPageError::DigestMismatch);
    }
    let artifact_name = artifact.as_str();
    let identity = ArtifactIdentity {
        run_id,
        graph_version,
        artifact: artifact_name,
        digest: &digest,
    };
    let identity_bytes =
        serde_json::to_vec(&identity).map_err(|_| ArtifactPageError::Serialization)?;
    let artifact_id = format!("sha256:{:x}", Sha256::digest(identity_bytes));
    if expected_artifact_id.is_some_and(|expected| expected != artifact_id) {
        return Err(ArtifactPageError::ArtifactIdentityMismatch);
    }
    let start = usize::try_from(offset).map_err(|_| ArtifactPageError::InvalidOffset)?;
    if start > bytes.len() || !std::str::from_utf8(&bytes[..start]).is_ok() {
        return Err(ArtifactPageError::InvalidOffset);
    }
    let total_bytes = u64::try_from(bytes.len()).map_err(|_| ArtifactPageError::InvalidOffset)?;
    let requested_end = start.saturating_add(usize::try_from(limit).unwrap_or(usize::MAX));
    let mut end = requested_end.min(bytes.len());
    while end > start && std::str::from_utf8(&bytes[start..end]).is_err() {
        end -= 1;
    }
    if end == start && start < bytes.len() {
        return Err(ArtifactPageError::LimitTooSmall);
    }
    let data = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| ArtifactPageError::InvalidOffset)?
        .to_owned();
    let done = end == bytes.len();
    let next_offset = if done {
        None
    } else {
        Some(u64::try_from(end).map_err(|_| ArtifactPageError::InvalidOffset)?)
    };
    Ok(ArtifactPage {
        schema: "agent-graph-artifact-page-v1",
        run_id: run_id.to_owned(),
        graph_version: graph_version.to_owned(),
        artifact: artifact_name.to_owned(),
        encoding: "json-utf8",
        digest,
        artifact_id,
        total_bytes,
        offset,
        next_offset,
        done,
        data,
    })
}
