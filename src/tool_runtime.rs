use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::evidence::{digest, hmac_sha256, redact};

pub const TOOL_LEASE_PROTOCOL: &str = "agent_graph.tool_lease.v1";
pub const MAX_RECEIPT_SUMMARY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    ReadOnly,
    LocalMutation,
    ExternalEffect,
    AuthorityChange,
    RecursiveOrchestration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCounters {
    pub tool_calls: u64,
    pub recursive_calls: u64,
    pub children: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolLease {
    pub protocol: String,
    pub lease_id: String,
    pub lineage_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub run_id: String,
    pub node_id: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub tool_allowlist: Vec<String>,
    pub effect_allowlist: Vec<ToolEffect>,
    pub max_tool_calls: u64,
    pub max_recursive_calls: u64,
    pub max_agent_depth: u64,
    pub max_graph_depth: u64,
    pub max_children: u64,
    pub agent_depth: u64,
    pub graph_depth: u64,
    pub active_stack: Vec<String>,
    pub counters: ToolCounters,
    pub parent_receipt_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedToolLease {
    pub lease: ToolLease,
    pub signature: String,
}

#[derive(Debug, Clone, Copy)]
pub struct LeaseBinding<'a> {
    pub graph_id: &'a str,
    pub graph_version: &'a str,
    pub run_id: &'a str,
    pub node_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInvocation {
    pub graph_id: String,
    pub graph_version: String,
    pub run_id: String,
    pub node_id: String,
    pub attempt: u64,
    pub tool_name: String,
    pub arguments: Value,
    pub effect: ToolEffect,
    pub recursion_identity: Option<String>,
    pub parent_receipt_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallIntent {
    pub protocol: String,
    pub call_id: String,
    pub lease_id: String,
    pub lease_digest: String,
    pub lineage_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub run_id: String,
    pub node_id: String,
    pub attempt: u64,
    pub tool_name: String,
    pub arguments_digest: String,
    pub effect: ToolEffect,
    pub parent_receipt_digest: Option<String>,
    pub reserved_at: DateTime<Utc>,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptOutcome {
    Succeeded,
    Failed,
    Blocked,
    Cancelled,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCallReceipt {
    pub protocol: String,
    pub call_id: String,
    pub intent_digest: String,
    pub lineage_id: String,
    pub outcome: ReceiptOutcome,
    pub result_digest: String,
    pub redacted_summary: String,
    pub parent_receipt_digest: Option<String>,
    pub completed_at: DateTime<Utc>,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallReservation {
    pub intent: ToolCallIntent,
    pub updated_lease: SignedToolLease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolPolicyError {
    LeaseProtocolUnsupported,
    LeaseIdentityInvalid,
    LeaseSignatureRequired,
    LeaseSignatureInvalid,
    LeaseExpired,
    LeaseNotYetValid,
    LeaseBindingMismatch,
    LeaseScopeInvalid,
    ToolNotGranted,
    EffectNotGranted,
    EffectClassificationMismatch,
    ToolBudgetExhausted,
    RecursiveBudgetExhausted,
    ChildBudgetExhausted,
    AgentDepthExceeded,
    GraphDepthExceeded,
    RecursionCycleDetected,
    InvocationInvalid,
    IntentSignatureInvalid,
    ReceiptSignatureInvalid,
    ReceiptIntentMismatch,
    ReceiptChainMismatch,
    ReceiptSummaryTooLarge,
    SerializationFailed,
}

impl fmt::Display for ToolPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ToolPolicyError {}

fn signed_value<T: Serialize>(value: &T) -> Result<Value, ToolPolicyError> {
    serde_json::to_value(value).map_err(|_| ToolPolicyError::SerializationFailed)
}

fn secure_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

pub fn issue_lease(lease: ToolLease, key: &[u8]) -> Result<SignedToolLease, ToolPolicyError> {
    validate_lease_shape(&lease)?;
    let signature = hmac_sha256(&signed_value(&lease)?, key);
    Ok(SignedToolLease { lease, signature })
}

fn validate_lease_shape(lease: &ToolLease) -> Result<(), ToolPolicyError> {
    if lease.protocol != TOOL_LEASE_PROTOCOL {
        return Err(ToolPolicyError::LeaseProtocolUnsupported);
    }
    if lease.lease_id.trim().is_empty()
        || lease.lineage_id.trim().is_empty()
        || lease.graph_id.trim().is_empty()
        || lease.graph_version.trim().is_empty()
        || lease.run_id.trim().is_empty()
        || lease.node_id.trim().is_empty()
    {
        return Err(ToolPolicyError::LeaseIdentityInvalid);
    }
    if lease.expires_at <= lease.issued_at
        || lease.tool_allowlist.is_empty()
        || lease.effect_allowlist.is_empty()
        || lease.max_tool_calls == 0
    {
        return Err(ToolPolicyError::LeaseScopeInvalid);
    }
    if lease.agent_depth > lease.max_agent_depth {
        return Err(ToolPolicyError::AgentDepthExceeded);
    }
    if lease.graph_depth > lease.max_graph_depth {
        return Err(ToolPolicyError::GraphDepthExceeded);
    }
    if lease.counters.tool_calls > lease.max_tool_calls
        || lease.counters.recursive_calls > lease.max_recursive_calls
        || lease.counters.children > lease.max_children
    {
        return Err(ToolPolicyError::LeaseScopeInvalid);
    }
    Ok(())
}

pub fn verify_lease(
    signed: &SignedToolLease,
    key: &[u8],
    now: DateTime<Utc>,
    binding: LeaseBinding<'_>,
) -> Result<(), ToolPolicyError> {
    validate_lease_shape(&signed.lease)?;
    if signed.signature.is_empty() {
        return Err(ToolPolicyError::LeaseSignatureRequired);
    }
    let expected = hmac_sha256(&signed_value(&signed.lease)?, key);
    if !secure_eq(&expected, &signed.signature) {
        return Err(ToolPolicyError::LeaseSignatureInvalid);
    }
    if signed.lease.expires_at <= now {
        return Err(ToolPolicyError::LeaseExpired);
    }
    if signed.lease.issued_at > now {
        return Err(ToolPolicyError::LeaseNotYetValid);
    }
    if signed.lease.graph_id != binding.graph_id
        || signed.lease.graph_version != binding.graph_version
        || signed.lease.run_id != binding.run_id
        || signed.lease.node_id != binding.node_id
    {
        return Err(ToolPolicyError::LeaseBindingMismatch);
    }
    Ok(())
}

fn tool_granted(allowlist: &[String], tool_name: &str) -> bool {
    allowlist
        .iter()
        .any(|candidate| candidate == "*" || candidate == tool_name)
}

fn is_recursive(effect: ToolEffect) -> bool {
    effect == ToolEffect::RecursiveOrchestration
}

pub fn reserve_call(
    signed: &SignedToolLease,
    key: &[u8],
    now: DateTime<Utc>,
    binding: LeaseBinding<'_>,
    invocation: ToolInvocation,
) -> Result<CallReservation, ToolPolicyError> {
    verify_lease(signed, key, now, binding)?;
    if invocation.graph_id != binding.graph_id
        || invocation.graph_version != binding.graph_version
        || invocation.run_id != binding.run_id
        || invocation.node_id != binding.node_id
        || invocation.attempt == 0
        || invocation.tool_name.trim().is_empty()
    {
        return Err(ToolPolicyError::InvocationInvalid);
    }
    if !tool_granted(&signed.lease.tool_allowlist, &invocation.tool_name) {
        return Err(ToolPolicyError::ToolNotGranted);
    }
    let classified = classify_tool(&invocation.tool_name, &invocation.arguments);
    if classified != invocation.effect {
        return Err(ToolPolicyError::EffectClassificationMismatch);
    }
    if !signed.lease.effect_allowlist.contains(&classified) {
        return Err(ToolPolicyError::EffectNotGranted);
    }
    if signed.lease.counters.tool_calls >= signed.lease.max_tool_calls {
        return Err(ToolPolicyError::ToolBudgetExhausted);
    }
    if is_recursive(classified)
        && signed.lease.counters.recursive_calls >= signed.lease.max_recursive_calls
    {
        return Err(ToolPolicyError::RecursiveBudgetExhausted);
    }
    if let Some(identity) = invocation.recursion_identity.as_deref() {
        if signed
            .lease
            .active_stack
            .iter()
            .any(|active| active == identity)
        {
            return Err(ToolPolicyError::RecursionCycleDetected);
        }
    }

    let lease_digest = digest(&signed_value(&signed.lease)?);
    let arguments_digest = digest(&redact(&invocation.arguments));
    let call_identity = serde_json::json!({
        "lease_digest": lease_digest,
        "graph_id": invocation.graph_id,
        "graph_version": invocation.graph_version,
        "run_id": invocation.run_id,
        "node_id": invocation.node_id,
        "attempt": invocation.attempt,
        "tool_name": invocation.tool_name,
        "arguments_digest": arguments_digest,
        "parent_receipt_digest": invocation.parent_receipt_digest,
    });
    let call_id = format!(
        "tool-call-{}",
        digest(&call_identity)
            .strip_prefix("sha256:")
            .unwrap_or("invalid")
    );
    let mut intent = ToolCallIntent {
        protocol: "agent_graph.tool_intent.v1".into(),
        call_id,
        lease_id: signed.lease.lease_id.clone(),
        lease_digest,
        lineage_id: signed.lease.lineage_id.clone(),
        graph_id: invocation.graph_id,
        graph_version: invocation.graph_version,
        run_id: invocation.run_id,
        node_id: invocation.node_id,
        attempt: invocation.attempt,
        tool_name: invocation.tool_name,
        arguments_digest,
        effect: classified,
        parent_receipt_digest: invocation.parent_receipt_digest,
        reserved_at: now,
        signature: String::new(),
    };
    intent.signature = sign_intent(&intent, key)?;

    let mut updated = signed.lease.clone();
    updated.counters.tool_calls = updated.counters.tool_calls.saturating_add(1);
    if is_recursive(classified) {
        updated.counters.recursive_calls = updated.counters.recursive_calls.saturating_add(1);
    }
    let updated_lease = issue_lease(updated, key)?;
    Ok(CallReservation {
        intent,
        updated_lease,
    })
}

fn intent_unsigned(intent: &ToolCallIntent) -> Result<Value, ToolPolicyError> {
    let mut value = signed_value(intent)?;
    let object = value
        .as_object_mut()
        .ok_or(ToolPolicyError::SerializationFailed)?;
    object.insert("signature".into(), Value::String(String::new()));
    Ok(value)
}

fn sign_intent(intent: &ToolCallIntent, key: &[u8]) -> Result<String, ToolPolicyError> {
    Ok(hmac_sha256(&intent_unsigned(intent)?, key))
}

fn verify_intent(intent: &ToolCallIntent, key: &[u8]) -> Result<(), ToolPolicyError> {
    let expected = sign_intent(intent, key)?;
    if !secure_eq(&expected, &intent.signature) {
        return Err(ToolPolicyError::IntentSignatureInvalid);
    }
    Ok(())
}

fn receipt_unsigned(receipt: &ToolCallReceipt) -> Result<Value, ToolPolicyError> {
    let mut value = signed_value(receipt)?;
    let object = value
        .as_object_mut()
        .ok_or(ToolPolicyError::SerializationFailed)?;
    object.insert("signature".into(), Value::String(String::new()));
    Ok(value)
}

impl ToolCallReceipt {
    pub fn complete(
        intent: &ToolCallIntent,
        outcome: ReceiptOutcome,
        result: &Value,
        redacted_summary: &str,
        completed_at: DateTime<Utc>,
        key: &[u8],
    ) -> Result<Self, ToolPolicyError> {
        if redacted_summary.len() > MAX_RECEIPT_SUMMARY_BYTES {
            return Err(ToolPolicyError::ReceiptSummaryTooLarge);
        }
        let mut receipt = Self {
            protocol: "agent_graph.tool_receipt.v1".into(),
            call_id: intent.call_id.clone(),
            intent_digest: digest(&signed_value(intent)?),
            lineage_id: intent.lineage_id.clone(),
            outcome,
            result_digest: digest(&redact(result)),
            redacted_summary: redacted_summary.to_owned(),
            parent_receipt_digest: intent.parent_receipt_digest.clone(),
            completed_at,
            signature: String::new(),
        };
        receipt.signature = hmac_sha256(&receipt_unsigned(&receipt)?, key);
        Ok(receipt)
    }
}

pub fn verify_receipt_chain(
    intent: &ToolCallIntent,
    receipt: &ToolCallReceipt,
    key: &[u8],
) -> Result<(), ToolPolicyError> {
    verify_intent(intent, key)?;
    if receipt.protocol != "agent_graph.tool_receipt.v1"
        || receipt.call_id != intent.call_id
        || receipt.lineage_id != intent.lineage_id
        || receipt.intent_digest != digest(&signed_value(intent)?)
    {
        return Err(ToolPolicyError::ReceiptIntentMismatch);
    }
    if receipt.parent_receipt_digest != intent.parent_receipt_digest {
        return Err(ToolPolicyError::ReceiptChainMismatch);
    }
    let expected = hmac_sha256(&receipt_unsigned(receipt)?, key);
    if !secure_eq(&expected, &receipt.signature) {
        return Err(ToolPolicyError::ReceiptSignatureInvalid);
    }
    Ok(())
}

pub fn classify_tool(tool_name: &str, arguments: &Value) -> ToolEffect {
    let lower = tool_name.to_ascii_lowercase();
    if lower == "delegate_task"
        || lower == "execute_code"
        || lower.starts_with("mcp__agent_graph__graph_execute")
        || lower.starts_with("mcp__agent_graph__graph_run_start")
        || lower.starts_with("mcp__agent_graph__graph_run_resume")
        || lower.starts_with("mcp__agent_graph__graph_execute")
    {
        return ToolEffect::RecursiveOrchestration;
    }
    if lower == "cronjob" {
        return match arguments.get("action").and_then(Value::as_str) {
            Some("list") => ToolEffect::ReadOnly,
            _ => ToolEffect::RecursiveOrchestration,
        };
    }
    if matches!(
        lower.as_str(),
        "read_file"
            | "search_files"
            | "session_search"
            | "web_search"
            | "web_extract"
            | "browser_snapshot"
            | "browser_get_images"
            | "browser_console"
            | "ha_get_state"
            | "ha_list_entities"
            | "ha_list_services"
            | "skills_list"
            | "skill_view"
            | "git_status"
    ) || lower.starts_with("mcp__semantic_memory__sm_get_")
        || lower.starts_with("mcp__semantic_memory__sm_list_")
        || lower.starts_with("mcp__semantic_memory__sm_search")
        || lower.starts_with("mcp__agent_graph__graph_list")
        || lower.starts_with("mcp__agent_graph__graph_inspect")
        || lower.starts_with("mcp__agent_graph__graph_render")
        || lower.starts_with("mcp__agent_graph__graph_run_get")
        || lower.starts_with("mcp__agent_graph__graph_run_events")
        || lower.starts_with("mcp__agent_graph__graph_run_receipt")
    {
        return ToolEffect::ReadOnly;
    }
    if matches!(
        lower.as_str(),
        "write_file" | "patch" | "skill_manage" | "project_create" | "project_switch"
    ) {
        return ToolEffect::LocalMutation;
    }
    if matches!(
        lower.as_str(),
        "ha_call_service" | "browser_click" | "browser_type" | "computer_use" | "text_to_speech"
    ) {
        return ToolEffect::ExternalEffect;
    }
    if lower.contains("approval")
        || lower.ends_with("delete_namespace")
        || lower.ends_with("delete_fact")
    {
        return ToolEffect::AuthorityChange;
    }
    ToolEffect::ExternalEffect
}
