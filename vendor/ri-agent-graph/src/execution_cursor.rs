use serde::{Deserialize, Serialize};

/// Exact position of an interrupted execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionCursor {
    pub iteration: usize,
    pub step_number: usize,
    pub interrupt_phase: InterruptPhase,
    pub resume_boundary: String,
    pub active_nodes: Vec<String>,
    pub remaining_nodes: Vec<String>,
    pub completed_nodes_in_superstep: Vec<String>,
    pub graph_version: String,
    pub state_digest: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InterruptPhase {
    Before,
    After,
}
