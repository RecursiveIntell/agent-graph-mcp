//! Checkpoint-to-state binding for model-state reuse across graph retries.
//!
//! When a graph node uses a local model executor (e.g. proveKV + Qwen),
//! each checkpoint can be bound to a proveKV state ID. On retry, the
//! pre-call state is forked so the retry starts from the same model state
//! without replaying non-idempotent side effects.
//!
//! # Ownership boundaries
//!
//! - Agent Graph owns graph checkpoints and retry policy.
//! - proveKV owns state IDs, leases, and fork semantics.
//! - llm-pipeline owns model-call parsing and semantic retry.
//! - llm-tool-runtime owns tool retry, idempotency, and side-effect metadata.
//! - Hermes owns transcript and tool receipts.
//!
//! # Safety invariants
//!
//! - A parser/model retry may fork from a pre-call state but must never
//!   replay a completed non-idempotent tool.
//! - Model-state rollback must never rewind transcript/effect truth.
//! - Retry ownership must be unambiguous — no duplicate truth.
//! - Attempt leases must not survive cancellation indefinitely.
//! - Only lease digests may appear in receipts; never bearer secrets.

use serde::{Deserialize, Serialize};

/// Links a graph checkpoint to a proveKV state for replay.
///
/// After a model call with attached executor, the checkpoint carries
/// the state ID so a retry can fork from the same pre-call state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointStateBinding {
    /// Graph run identifier.
    pub run_id: String,
    /// Graph node identifier.
    pub node_id: String,
    /// Monotonic attempt counter (0 = first attempt).
    pub attempt_id: u64,
    /// proveKV state ID at the point of this checkpoint.
    pub state_id: String,
    /// Lease digest (non-replayable, safe for logs/receipts).
    pub lease_digest: String,
    /// Tool idempotency key for the effect that follows this checkpoint,
    /// if any. A parser retry must not replay a non-idempotent tool.
    pub tool_idempotency_key: Option<String>,
    /// Whether the tool after this checkpoint has been committed.
    pub tool_committed: bool,
}

impl CheckpointStateBinding {
    /// True if this checkpoint precedes a non-idempotent tool that has
    /// already been committed. In that case, a model retry must not
    /// replay the tool.
    pub fn blocks_retry(&self) -> bool {
        self.tool_committed && self.tool_idempotency_key.is_none()
    }

    /// True if the tool is safe to replay (idempotent or not yet
    /// committed).
    pub fn allows_retry(&self) -> bool {
        !self.tool_committed || self.tool_idempotency_key.is_some()
    }
}

/// A joined lineage receipt for a graph node execution that used
/// model-state reuse.
///
/// Links the model retry owner, checkpoint, state ID, lease digest,
/// tool idempotency key, parser outcome, and effect receipt without
/// duplicating truth across owners.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateBoundExecutionReceipt {
    /// Node execution lineage.
    pub run_id: String,
    pub node_id: String,
    pub attempt_id: u64,

    /// Checkpoint identity.
    pub checkpoint_id: String,

    /// proveKV state IDs for pre-call and post-call states.
    pub pre_call_state_id: String,
    pub post_call_state_id: Option<String>,

    /// Lease digest (non-replayable).
    pub lease_digest: String,

    /// Tool execution metadata.
    pub tool_idempotency_key: Option<String>,
    pub tool_committed: bool,

    /// Parser outcome: did the model output parse successfully?
    pub parser_outcome: Option<String>,

    /// Effect receipt from Hermes, if a tool was executed.
    pub effect_receipt_digest: Option<String>,

    /// Whether this execution was a retry.
    pub is_retry: bool,

    /// If this is a retry, which attempt was retried.
    pub retry_of_attempt: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncommitted_checkpoint_allows_retry() {
        let binding = CheckpointStateBinding {
            run_id: "r1".into(),
            node_id: "n1".into(),
            attempt_id: 0,
            state_id: "state-1".into(),
            lease_digest: "digest-1".into(),
            tool_idempotency_key: None,
            tool_committed: false,
        };
        assert!(binding.allows_retry());
        assert!(!binding.blocks_retry());
    }

    #[test]
    fn idempotent_committed_tool_allows_retry() {
        let binding = CheckpointStateBinding {
            run_id: "r1".into(),
            node_id: "n1".into(),
            attempt_id: 0,
            state_id: "state-1".into(),
            lease_digest: "digest-1".into(),
            tool_idempotency_key: Some("idem-key-1".into()),
            tool_committed: true,
        };
        assert!(binding.allows_retry());
        assert!(!binding.blocks_retry());
    }

    #[test]
    fn non_idempotent_committed_tool_blocks_retry() {
        let binding = CheckpointStateBinding {
            run_id: "r1".into(),
            node_id: "n1".into(),
            attempt_id: 0,
            state_id: "state-1".into(),
            lease_digest: "digest-1".into(),
            tool_idempotency_key: None,
            tool_committed: true,
        };
        assert!(binding.blocks_retry());
        assert!(!binding.allows_retry());
    }

    #[test]
    fn receipt_carries_lineage_without_duplicate_truth() {
        let receipt = StateBoundExecutionReceipt {
            run_id: "r1".into(),
            node_id: "n1".into(),
            attempt_id: 2,
            checkpoint_id: "ckpt-1".into(),
            pre_call_state_id: "state-pre".into(),
            post_call_state_id: Some("state-post".into()),
            lease_digest: "digest-1".into(),
            tool_idempotency_key: Some("idem-1".into()),
            tool_committed: true,
            parser_outcome: Some("parsed".into()),
            effect_receipt_digest: Some("effect-digest".into()),
            is_retry: true,
            retry_of_attempt: Some(1),
        };

        assert!(receipt.is_retry);
        assert_eq!(receipt.retry_of_attempt, Some(1));
        assert_ne!(
            receipt.pre_call_state_id,
            receipt.post_call_state_id.unwrap()
        );
    }
}
