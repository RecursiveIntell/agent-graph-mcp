//! Node execution outcomes and interrupt types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The result of executing a node in the graph.
#[derive(Debug, Clone)]
pub enum NodeOutcome {
    /// Node completed successfully, producing an output value.
    Continue { output: Value },
    /// Node requests an interrupt (e.g., needs user input).
    Interrupt { interrupt: Interrupt },
    /// Node execution failed.
    Fail { error: String },
}

/// An interrupt raised by a node during execution.
///
/// Encodes what the node needs and provides a correlation token
/// so that resume can be matched safely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interrupt {
    /// What kind of interrupt this is.
    pub kind: InterruptKind,
    /// A JSON value describing what's needed (e.g., a prompt for the user).
    pub payload: Value,
    /// Unique correlation token so resume can be validated.
    pub correlation_id: String,
}

impl Interrupt {
    /// Create a new interrupt requesting user input.
    pub fn await_input(payload: Value) -> Self {
        Self {
            kind: InterruptKind::AwaitInput,
            payload,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create a new interrupt requesting approval.
    pub fn await_approval(payload: Value) -> Self {
        Self {
            kind: InterruptKind::AwaitApproval,
            payload,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create a custom interrupt.
    pub fn custom(kind: impl Into<String>, payload: Value) -> Self {
        Self {
            kind: InterruptKind::Custom(kind.into()),
            payload,
            correlation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// The kind of interrupt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InterruptKind {
    /// Node needs user input to continue.
    AwaitInput,
    /// Node needs approval to continue.
    AwaitApproval,
    /// Custom interrupt kind.
    Custom(String),
}

impl std::fmt::Display for InterruptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterruptKind::AwaitInput => write!(f, "await_input"),
            InterruptKind::AwaitApproval => write!(f, "await_approval"),
            InterruptKind::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}
