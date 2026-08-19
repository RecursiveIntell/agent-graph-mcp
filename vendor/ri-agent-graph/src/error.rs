use thiserror::Error;

/// The checkpoint-store operation that failed during a configured durable run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointStoreOperation {
    CreateRun,
    RecordAttempt,
    CompleteAttempt,
    FailAttempt,
    SaveStateSnapshot,
    CompleteRun,
    FailRun,
}

impl std::fmt::Display for CheckpointStoreOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let operation = match self {
            Self::CreateRun => "create run",
            Self::RecordAttempt => "record attempt",
            Self::CompleteAttempt => "complete attempt",
            Self::FailAttempt => "fail attempt",
            Self::SaveStateSnapshot => "save state snapshot",
            Self::CompleteRun => "complete run",
            Self::FailRun => "fail run",
        };
        f.write_str(operation)
    }
}

#[derive(Error, Debug)]
pub enum AgentGraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("Max iterations exceeded: {current}/{max}")]
    MaxIterationsExceeded { current: usize, max: usize },

    #[error("Cycle detected: {path:?}")]
    CycleDetected { path: Vec<String> },

    #[error("Checkpoint error: {0}")]
    CheckpointError(String),

    /// A configured granular checkpoint store failed; durable execution cannot continue.
    #[error("Checkpoint store failed to {operation}: {message}")]
    CheckpointStore {
        operation: CheckpointStoreOperation,
        message: String,
    },

    #[error("Checkpoint graph mismatch: expected hash '{expected}', got '{actual}'")]
    CheckpointMismatch { expected: String, actual: String },

    #[error("run not found: {0}")]
    RunNotFound(String),
    #[error("attempt not found: {0}")]
    AttemptNotFound(String),
    #[error("attempt '{attempt_id}' belongs to run '{actual_run}', not '{expected_run}'")]
    AttemptRunMismatch {
        attempt_id: String,
        expected_run: String,
        actual_run: String,
    },
    #[error("invalid checkpoint transition: {0}")]
    InvalidTransition(String),
    #[error("terminal state conflict for run '{0}'")]
    TerminalStateConflict(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Interrupt at node '{node}'")]
    InterruptError {
        node: String,
        value: Option<serde_json::Value>,
    },

    #[error("Payload error: {0}")]
    PayloadError(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[cfg(feature = "checkpointing")]
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),

    #[error("{0}")]
    Other(String),
}

impl AgentGraphError {
    /// Stable string discriminant for structured logging (PRIMITIVES_CONTRACT §2).
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NodeNotFound(_) => "node_not_found",
            Self::RoutingError(_) => "routing",
            Self::StateError(_) => "state",
            Self::MaxIterationsExceeded { .. } => "max_iterations",
            Self::CycleDetected { .. } => "cycle_detected",
            Self::CheckpointError(_) => "checkpoint",
            Self::CheckpointStore { .. } => "checkpoint_store",
            Self::CheckpointMismatch { .. } => "checkpoint_mismatch",
            Self::RunNotFound(_) => "run_not_found",
            Self::AttemptNotFound(_) => "attempt_not_found",
            Self::AttemptRunMismatch { .. } => "attempt_run_mismatch",
            Self::InvalidTransition(_) => "invalid_transition",
            Self::TerminalStateConflict(_) => "terminal_state_conflict",
            Self::ExecutionError(_) => "execution",
            Self::InterruptError { .. } => "interrupt",
            Self::PayloadError(_) => "payload",
            Self::Cancelled => "cancelled",
            Self::SerializationError(_) => "serialization",
            #[cfg(feature = "checkpointing")]
            Self::DatabaseError(_) => "database",
            Self::Other(_) => "other",
        }
    }
}

pub type Result<T> = std::result::Result<T, AgentGraphError>;

/// Create an interrupt error that can be returned from within a node.
/// This causes the graph executor to pause execution at the current node.
pub fn interrupt(node: impl Into<String>, value: Option<serde_json::Value>) -> AgentGraphError {
    AgentGraphError::InterruptError {
        node: node.into(),
        value,
    }
}
