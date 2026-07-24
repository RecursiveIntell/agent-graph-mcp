use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::store::PersistentStore;

pub const MAX_WITNESS_CONTENT_BYTES: usize = 256 * 1024;
pub const MAX_WITNESS_LOCATOR_BYTES: usize = 4096;
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EvidenceDisposition {
    pub shape_valid: bool,
    pub integrity_verified: bool,
    pub source_witness_bound: bool,
    pub source_authority: &'static str,
    pub factual_support: &'static str,
}

impl EvidenceDisposition {
    pub fn model_output() -> Self {
        Self {
            shape_valid: false,
            integrity_verified: false,
            source_witness_bound: false,
            source_authority: "unverified",
            factual_support: "unjudged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessError {
    pub code: String,
    pub message: String,
}

impl WitnessError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WitnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessCapture {
    pub locator: String,
    pub content: String,
    pub media_type: String,
    pub authority_class: String,
    pub retrieved_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessRecord {
    pub witness_id: String,
    pub locator: String,
    pub content: String,
    pub media_type: String,
    pub authority_class: String,
    pub retrieved_at: String,
    pub digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitnessSpan {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessBinding {
    pub witness_id: String,
    pub span: WitnessSpan,
}

pub fn witness_envelope_digest(capture: &WitnessCapture) -> String {
    // A struct gives the envelope a fixed field order independent of JSON map
    // ordering. The captured content is hashed as supplied; it is never fetched
    // or normalized by this crate.
    #[derive(serde::Serialize)]
    struct Envelope<'a> {
        locator: &'a str,
        content: &'a str,
        media_type: &'a str,
        authority_class: &'a str,
        retrieved_at: &'a str,
    }
    let envelope = Envelope {
        locator: &capture.locator,
        content: &capture.content,
        media_type: &capture.media_type,
        authority_class: &capture.authority_class,
        retrieved_at: &capture.retrieved_at,
    };
    let bytes = serde_json::to_vec(&envelope).expect("witness envelope serialization");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// Authenticate an integrity-sensitive envelope with a secret that remains
/// outside SQLite, receipts, and bundles.
pub fn hmac_sha256(value: &Value, key: &[u8]) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = [0u8; 64];
    let mut outer = [0u8; 64];
    for (index, byte) in block.iter().enumerate() {
        inner[index] = byte ^ 0x36;
        outer[index] = byte ^ 0x5c;
    }
    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(bytes);
    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_hash.finalize());
    format!("hmac-sha256:{:x}", outer_hash.finalize())
}

pub fn witness_envelope_hmac(capture: &WitnessCapture, key: &[u8]) -> String {
    #[derive(serde::Serialize)]
    struct Envelope<'a> {
        locator: &'a str,
        content: &'a str,
        media_type: &'a str,
        authority_class: &'a str,
        retrieved_at: &'a str,
    }
    let envelope = Envelope {
        locator: &capture.locator,
        content: &capture.content,
        media_type: &capture.media_type,
        authority_class: &capture.authority_class,
        retrieved_at: &capture.retrieved_at,
    };
    hmac_sha256(
        &serde_json::to_value(envelope).expect("witness envelope serialization"),
        key,
    )
}

pub fn witness_id_for_digest(digest: &str) -> String {
    format!(
        "witness-{}",
        digest.strip_prefix("sha256:").unwrap_or(digest)
    )
}

pub fn validate_witness_capture(capture: WitnessCapture) -> Result<WitnessRecord, WitnessError> {
    validate_witness_capture_with_key(capture, None)
}

pub fn validate_witness_capture_with_key(
    capture: WitnessCapture,
    key: Option<&[u8]>,
) -> Result<WitnessRecord, WitnessError> {
    if capture.locator.trim().is_empty() {
        return Err(WitnessError::new(
            "WITNESS_INVALID_LOCATOR",
            "locator must be non-empty",
        ));
    }
    if capture.locator.len() > MAX_WITNESS_LOCATOR_BYTES {
        return Err(WitnessError::new(
            "WITNESS_LOCATOR_TOO_LARGE",
            "locator exceeds 4096 UTF-8 bytes",
        ));
    }
    if capture.locator.chars().any(char::is_control) {
        return Err(WitnessError::new(
            "WITNESS_INVALID_LOCATOR",
            "locator must not contain control characters",
        ));
    }
    if capture.content.is_empty() {
        return Err(WitnessError::new(
            "WITNESS_INVALID_CONTENT",
            "content must be non-empty",
        ));
    }
    if capture.content.len() > MAX_WITNESS_CONTENT_BYTES {
        return Err(WitnessError::new(
            "WITNESS_CONTENT_TOO_LARGE",
            "content exceeds 256 KiB UTF-8 bytes",
        ));
    }
    if !matches!(
        capture.media_type.as_str(),
        "text/plain" | "text/markdown" | "application/json"
    ) {
        return Err(WitnessError::new(
            "WITNESS_INVALID_MEDIA_TYPE",
            "media_type is not in the witness v1 allowlist",
        ));
    }
    if !matches!(
        capture.authority_class.as_str(),
        "caller_supplied_unverified" | "local_primary_capture"
    ) {
        return Err(WitnessError::new(
            "WITNESS_INVALID_AUTHORITY_CLASS",
            "authority_class is not in the witness v1 allowlist",
        ));
    }
    if chrono::DateTime::parse_from_rfc3339(&capture.retrieved_at).is_err() {
        return Err(WitnessError::new(
            "WITNESS_INVALID_TIMESTAMP",
            "retrieved_at must be an RFC3339 timestamp",
        ));
    }
    let digest = key.map_or_else(
        || witness_envelope_digest(&capture),
        |key| witness_envelope_hmac(&capture, key),
    );
    Ok(WitnessRecord {
        witness_id: witness_id_for_digest(&digest),
        locator: capture.locator,
        content: capture.content,
        media_type: capture.media_type,
        authority_class: capture.authority_class,
        retrieved_at: capture.retrieved_at,
        digest,
    })
}

pub fn verify_witness_record(record: &WitnessRecord) -> Result<(), WitnessError> {
    verify_witness_record_with_key(record, None)
}

pub fn verify_witness_record_with_key(
    record: &WitnessRecord,
    key: Option<&[u8]>,
) -> Result<(), WitnessError> {
    let capture = WitnessCapture {
        locator: record.locator.clone(),
        content: record.content.clone(),
        media_type: record.media_type.clone(),
        authority_class: record.authority_class.clone(),
        retrieved_at: record.retrieved_at.clone(),
    };
    let expected = validate_witness_capture_with_key(capture, key).map_err(|_| {
        WitnessError::new(
            "WITNESS_INTEGRITY_FAILURE",
            "stored witness integrity validation failed",
        )
    })?;
    if expected.digest != record.digest
        || expected.witness_id != record.witness_id
        || expected.locator != record.locator
    {
        return Err(WitnessError::new(
            "WITNESS_INTEGRITY_FAILURE",
            "stored witness integrity validation failed",
        ));
    }
    Ok(())
}

pub fn digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let hash = Sha256::digest(bytes);
    format!("sha256:{hash:x}")
}

pub fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    let lower = k.to_ascii_lowercase();
                    let redacted = [
                        "secret",
                        "token",
                        "password",
                        "authorization",
                        "api_key",
                        "credential",
                    ]
                    .iter()
                    .any(|needle| lower.contains(needle));
                    (
                        k.clone(),
                        if redacted {
                            Value::String("[REDACTED]".into())
                        } else {
                            redact(v)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        Value::String(text)
            if text.starts_with("sk-")
                || text.starts_with("Bearer ")
                || text.contains("BEGIN PRIVATE KEY") =>
        {
            Value::String("[REDACTED]".into())
        }
        value => value.clone(),
    }
}

pub fn bundle(
    run_id: &str,
    graph_version: &str,
    input: &Value,
    output: &Value,
    receipt: &Value,
) -> Value {
    let dependency_envelopes_complete = receipt
        .get("dependency_envelopes_complete")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let payload = serde_json::json!({"schema":"agent-graph-mcp-bundle-v1","run_id":run_id,"graph_version":graph_version,
        "input":redact(input),"output":redact(output),"receipt":redact(receipt),"replay_capability":"integrity_only",
        "dependency_envelopes_complete":dependency_envelopes_complete,"environment":{}});
    let integrity = digest(&payload);
    serde_json::json!({"payload":payload,"integrity":integrity})
}

pub fn verify(bundle: &Value) -> Value {
    let Some(payload) = bundle.get("payload") else {
        return serde_json::json!({"verified":false,"code":"INVALID_BUNDLE"});
    };
    let expected = bundle
        .get("integrity")
        .and_then(Value::as_str)
        .unwrap_or("");
    let actual = digest(payload);
    serde_json::json!({"verified":expected==actual,"level":"integrity_verified","expected":expected,"actual":actual,"models_or_tools_invoked":false})
}

/// Validate the small, deterministic evidence object contract used by research
/// workflows. This checks shape and references only; it does not fetch or
/// verify sources.
pub fn validate_research_evidence(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "research evidence must be an object".to_owned())?;
    let claims = object
        .get("claims")
        .and_then(Value::as_array)
        .ok_or_else(|| "research evidence requires a claims array".to_owned())?;
    if claims.is_empty() {
        return Err("research evidence claims array must not be empty".into());
    }
    let sources = object
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| "research evidence requires a sources array".to_owned())?;
    if sources.is_empty() {
        return Err("research evidence sources array must not be empty".into());
    }
    for (index, source) in sources.iter().enumerate() {
        let locator = source.get("locator").and_then(Value::as_str).unwrap_or("");
        if locator.trim().is_empty() {
            return Err(format!(
                "research evidence source {index} requires a non-empty locator"
            ));
        }
        if source
            .get("source_type")
            .and_then(Value::as_str)
            .is_none_or(|source_type| source_type.trim().is_empty())
        {
            return Err(format!(
                "research evidence source {index} requires a non-empty source_type"
            ));
        }
        if let Some(witness_id) = source.get("witness_id") {
            if witness_id.as_str().is_none_or(str::is_empty) {
                return Err(format!(
                    "research evidence source {index} witness_id must be a non-empty string"
                ));
            }
        }
    }
    for (index, claim) in claims.iter().enumerate() {
        if claim
            .get("text")
            .and_then(Value::as_str)
            .is_none_or(|text| text.trim().is_empty())
        {
            return Err(format!(
                "research evidence claim {index} requires non-empty text"
            ));
        }
        let locator_refs = claim
            .get("source_locator")
            .and_then(Value::as_str)
            .into_iter()
            .chain(claim.get("source").and_then(Value::as_str))
            .chain(
                claim
                    .get("source_locators")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            )
            .any(|locator| {
                sources
                    .iter()
                    .any(|source| source.get("locator").and_then(Value::as_str) == Some(locator))
            });
        let index_refs = claim
            .get("source_index")
            .and_then(Value::as_u64)
            .into_iter()
            .chain(
                claim
                    .get("source_indices")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_u64),
            )
            .any(|source_index| source_index < sources.len() as u64);
        let witness_ids = claim_witness_ids(claim).map_err(|error| error.to_string())?;
        if witness_ids.is_empty() {
            return Err(format!(
                "WITNESS_BINDING_REQUIRED: research evidence claim {index} requires one or more witness IDs"
            ));
        }
        let _ = (locator_refs, index_refs);
        parse_span(claim).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn claim_witness_ids(claim: &Value) -> Result<Vec<String>, WitnessError> {
    let mut ids = Vec::new();
    if let Some(id) = claim.get("witness_id") {
        let id = id.as_str().ok_or_else(|| {
            WitnessError::new(
                "WITNESS_BINDING_REQUIRED",
                "claim witness_id must be a non-empty string",
            )
        })?;
        if id.is_empty() {
            return Err(WitnessError::new(
                "WITNESS_BINDING_REQUIRED",
                "claim witness_id must be a non-empty string",
            ));
        }
        ids.push(id.to_owned());
    }
    if let Some(raw_ids) = claim.get("witness_ids") {
        let raw_ids = raw_ids.as_array().ok_or_else(|| {
            WitnessError::new(
                "WITNESS_BINDING_REQUIRED",
                "claim witness_ids must be an array of strings",
            )
        })?;
        if raw_ids.is_empty() {
            return Err(WitnessError::new(
                "WITNESS_BINDING_REQUIRED",
                "claim witness_ids must not be empty",
            ));
        }
        for raw_id in raw_ids {
            let id = raw_id.as_str().ok_or_else(|| {
                WitnessError::new(
                    "WITNESS_BINDING_REQUIRED",
                    "claim witness_ids must be an array of strings",
                )
            })?;
            if id.is_empty() {
                return Err(WitnessError::new(
                    "WITNESS_BINDING_REQUIRED",
                    "claim witness_ids must not contain empty strings",
                ));
            }
            ids.push(id.to_owned());
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn parse_span(claim: &Value) -> Result<WitnessSpan, WitnessError> {
    let span = claim
        .get("span")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            WitnessError::new(
                "WITNESS_SPAN_REQUIRED",
                "each witness-bound claim requires a span object",
            )
        })?;
    let start = span.get("start").and_then(Value::as_u64).ok_or_else(|| {
        WitnessError::new(
            "WITNESS_SPAN_INVALID",
            "witness span requires an unsigned start",
        )
    })?;
    let end = span.get("end").and_then(Value::as_u64).ok_or_else(|| {
        WitnessError::new(
            "WITNESS_SPAN_INVALID",
            "witness span requires an unsigned end",
        )
    })?;
    if start >= end {
        return Err(WitnessError::new(
            "WITNESS_SPAN_INVALID",
            "witness span must be non-empty with start < end",
        ));
    }
    Ok(WitnessSpan { start, end })
}

pub fn witness_bindings(value: &Value) -> Result<Vec<WitnessBinding>, WitnessError> {
    validate_research_evidence(value).map_err(|message| {
        let (code, message) = message
            .split_once(": ")
            .map(|(code, message)| (code.to_owned(), message.to_owned()))
            .unwrap_or_else(|| ("WITNESS_EVIDENCE_INVALID".to_owned(), message));
        WitnessError::new(code, message)
    })?;
    let claims = value
        .get("claims")
        .and_then(Value::as_array)
        .expect("validate_research_evidence checked claims");
    let mut bindings = Vec::new();
    for claim in claims {
        let span = parse_span(claim)?;
        for witness_id in claim_witness_ids(claim)? {
            bindings.push(WitnessBinding { witness_id, span });
        }
    }
    Ok(bindings)
}

pub fn validate_witness_dependencies(
    value: &Value,
    store: &PersistentStore,
) -> Result<Value, WitnessError> {
    let bindings = witness_bindings(value)?;
    let mut dependencies = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for binding in bindings {
        let Some(record) = store.get_witness(&binding.witness_id)? else {
            return Err(WitnessError::new(
                "WITNESS_NOT_FOUND",
                "referenced witness was not found in SQLite",
            ));
        };
        let bytes = record.content.as_bytes();
        let start = usize::try_from(binding.span.start).unwrap_or(usize::MAX);
        let end = usize::try_from(binding.span.end).unwrap_or(usize::MAX);
        if start >= end
            || end > bytes.len()
            || !record.content.is_char_boundary(start)
            || !record.content.is_char_boundary(end)
        {
            return Err(WitnessError::new(
                "WITNESS_SPAN_OUT_OF_RANGE",
                "witness span is outside captured UTF-8 content",
            ));
        }
        if seen.insert(binding.witness_id.clone()) {
            dependencies.push(serde_json::json!({
                "witness_id": record.witness_id,
                "digest": record.digest,
                "locator_digest": digest(&Value::String(record.locator)),
            }));
        }
    }
    Ok(Value::Array(dependencies))
}

#[cfg(test)]
mod tests {
    use super::{validate_research_evidence, validate_witness_dependencies, WitnessCapture};
    use crate::store::PersistentStore;
    use serde_json::json;

    fn configure_test_integrity_key() {
        let path = std::env::temp_dir().join("agent-graph-mcp-unit-integrity.key");
        std::fs::write(&path, [0x5au8; 32]).expect("test integrity key");
        std::env::set_var("AGENT_GRAPH_INTEGRITY_KEY_PATH", path);
    }

    #[test]
    fn research_evidence_requires_claims_and_source_locators() {
        assert!(validate_research_evidence(&json!({
            "claims": [{"text": "claim", "witness_id": "witness-test", "span":{"start":0,"end":5}}],
            "sources": [{"locator": "local://source", "source_type": "local", "witness_id":"witness-test"}]
        }))
        .is_ok());
        assert!(validate_research_evidence(&json!({
            "claims": [], "sources": [{"locator": "x"}]
        }))
        .is_err());
        assert!(validate_research_evidence(&json!({
            "claims": [{"text": "claim", "source_index": 0}],
            "sources": [{"locator": "x"}]
        }))
        .is_err());
        assert!(validate_research_evidence(&json!({
            "claims": [{"text": "claim"}],
            "sources": [{"locator": "x", "source_type": "web"}]
        }))
        .is_err());
        assert!(validate_research_evidence(&json!({
            "claims": [{"text": "claim", "source_index": 0}], "sources": [{"locator": "  ", "source_type": "web"}]
        }))
        .is_err());
    }

    #[test]
    fn witness_binding_requires_ids_and_bounded_spans() {
        assert!(validate_research_evidence(&json!({
            "claims": [{"text":"claim","source_index":0}],
            "sources": [{"locator":"local://source","source_type":"local"}]
        }))
        .is_err());
        assert!(validate_research_evidence(&json!({
            "claims": [{"text":"claim","witness_id":"witness-test","span":{"start":3,"end":3}}],
            "sources": [{"locator":"local://source","source_type":"local","witness_id":"witness-test"}]
        }))
        .is_err());
    }

    #[test]
    fn witness_dependencies_verify_sqlite_content_and_span() {
        configure_test_integrity_key();
        let temp = tempfile::tempdir().expect("witness database");
        let store = PersistentStore::open(temp.path()).expect("store");
        let record = store
            .capture_witness(WitnessCapture {
                locator: "local://source".into(),
                content: "bounded text".into(),
                media_type: "text/plain".into(),
                authority_class: "caller_supplied_unverified".into(),
                retrieved_at: "2026-07-21T12:00:00Z".into(),
            })
            .expect("capture");
        let value = json!({
            "claims": [{"text":"claim","witness_id":record.witness_id,"span":{"start":0,"end":7}}],
            "sources": [{"locator":"local://source","source_type":"local"}]
        });
        let dependencies = validate_witness_dependencies(&value, &store).expect("valid span");
        assert_eq!(dependencies[0]["digest"], record.digest);
        assert_eq!(
            dependencies[0]["locator_digest"].as_str().unwrap().len(),
            71
        );

        let out_of_range = json!({
            "claims": [{"text":"claim","witness_id":record.witness_id,"span":{"start":0,"end":99}}],
            "sources": [{"locator":"local://source","source_type":"local"}]
        });
        assert_eq!(
            validate_witness_dependencies(&out_of_range, &store)
                .expect_err("range must fail")
                .code,
            "WITNESS_SPAN_OUT_OF_RANGE"
        );
    }

    #[test]
    fn evidence_requires_durable_witness_store() {
        configure_test_integrity_key();
        let temp = tempfile::tempdir().expect("witness database");
        let store = PersistentStore::open(temp.path()).expect("store");
        let value = json!({
            "claims": [{"text":"claim","witness_id":"witness-absent","span":{"start":0,"end":5}}],
            "sources": [{"locator":"local://missing","source_type":"local"}]
        });
        let error = validate_witness_dependencies(&value, &store)
            .expect_err("missing durable witness must fail closed");
        assert_eq!(error.code, "WITNESS_NOT_FOUND");
    }
}
