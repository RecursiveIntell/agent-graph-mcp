use crate::state::AgentState;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for interrupt points in graph execution.
#[derive(Debug, Clone, Default)]
pub struct InterruptConfig {
    /// Nodes to interrupt BEFORE execution
    pub interrupt_before: Vec<String>,
    /// Nodes to interrupt AFTER execution
    pub interrupt_after: Vec<String>,
}

impl InterruptConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn before(mut self, node: impl Into<String>) -> Self {
        self.interrupt_before.push(node.into());
        self
    }

    pub fn after(mut self, node: impl Into<String>) -> Self {
        self.interrupt_after.push(node.into());
        self
    }

    pub fn should_interrupt_before(&self, node: &str) -> bool {
        self.interrupt_before.iter().any(|n| n == node)
    }

    pub fn should_interrupt_after(&self, node: &str) -> bool {
        self.interrupt_after.iter().any(|n| n == node)
    }

    pub fn is_empty(&self) -> bool {
        self.interrupt_before.is_empty() && self.interrupt_after.is_empty()
    }
}

/// Result of a graph execution that may be interrupted.
#[derive(Debug)]
pub enum ExecutionResult {
    /// Execution completed normally
    Complete(AgentState),
    /// Execution was interrupted
    Interrupted {
        /// Current state at interrupt point
        state: AgentState,
        /// Node where interruption occurred
        node: String,
        /// Value passed to the interrupt function
        interrupt_value: Option<Value>,
        /// Data needed to resume execution
        checkpoint_data: Option<InterruptCheckpoint>,
    },
    /// Execution failed with a typed error.
    ///
    /// AG-001: Ordinary (non-interrupt) errors are preserved as `Failed`
    /// instead of being silently mapped to `Complete`. The original error
    /// is carried so callers can inspect and handle it.
    Failed {
        /// The error that caused execution to fail.
        error: crate::error::AgentGraphError,
        /// State at the point of failure (may be partially mutated).
        state: AgentState,
    },
}

/// Data needed to resume from an interrupt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptCheckpoint {
    /// The node to resume from
    pub resume_node: String,
    /// Whether to re-execute the node or continue to next
    pub resume_before: bool,
    /// The iteration count at interrupt
    pub iteration: usize,
    /// Active nodes in current superstep
    pub active_nodes: Vec<String>,
    /// Hash of the graph topology at checkpoint time.
    /// Used to detect graph-definition drift on resume.
    #[serde(default)]
    pub graph_hash: Option<String>,
}
