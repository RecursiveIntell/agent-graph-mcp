//! Receipt types for graph execution auditability.
//!
//! Receipts capture the full state of graph execution steps for replay,
//! debugging, and compliance auditing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Digests the full state of a graph execution step for auditability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionReceiptV1 {
    pub step_index: usize,
    pub agent_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub input_digest: String,
    pub output_digest: String,
    pub tool_calls: Vec<ToolCallReceipt>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallReceipt {
    pub tool_name: String,
    pub arguments_digest: String,
    pub result_digest: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphExecutionReceiptV1 {
    pub graph_id: String,
    pub execution_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub steps: Vec<StepExecutionReceiptV1>,
    pub outcome: ExecutionOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Completed,
    Partial { failed_step: usize },
    Cancelled,
    InternalError { message: String },
}
