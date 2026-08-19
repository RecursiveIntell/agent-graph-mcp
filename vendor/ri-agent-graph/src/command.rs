use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Output from a node execution.
#[derive(Debug, Clone)]
pub enum NodeOutput {
    /// Standard completion - node modified state directly, follow normal edges
    Done,
    /// Command - state update + navigation instruction
    Command(Command),
}

/// A command that combines state updates with navigation instructions.
#[derive(Debug, Clone)]
pub struct Command {
    /// Optional state updates to apply
    pub update: Option<HashMap<String, Value>>,
    /// Navigation instruction
    pub goto: Navigation,
}

/// Navigation instructions for graph traversal.
#[derive(Debug, Clone)]
pub enum Navigation {
    /// Go to a specific node
    Node(String),
    /// Fan-out to multiple nodes simultaneously
    Nodes(Vec<String>),
    /// Terminate execution
    End,
    /// Dynamic fan-out with per-branch state
    Send(Vec<SendOp>),
    /// Follow normal edges (default behavior)
    Default,
}

/// A send operation for dynamic fan-out with custom state per branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendOp {
    /// Target node name
    pub node: String,
    /// Custom state values for this branch
    pub state: HashMap<String, Value>,
}

impl Command {
    /// Create a command that navigates to a specific node
    pub fn goto(node: impl Into<String>) -> Self {
        Self {
            update: None,
            goto: Navigation::Node(node.into()),
        }
    }

    /// Create a command that ends execution
    pub fn end() -> Self {
        Self {
            update: None,
            goto: Navigation::End,
        }
    }

    /// Create a command with state updates and default navigation
    pub fn update(updates: HashMap<String, Value>) -> Self {
        Self {
            update: Some(updates),
            goto: Navigation::Default,
        }
    }

    /// Add state updates to this command
    pub fn with_update(mut self, updates: HashMap<String, Value>) -> Self {
        self.update = Some(updates);
        self
    }

    /// Set the navigation for this command
    pub fn with_goto(mut self, goto: Navigation) -> Self {
        self.goto = goto;
        self
    }
}

impl NodeOutput {
    /// Create a command that navigates to a specific node
    pub fn goto(node: impl Into<String>) -> Self {
        NodeOutput::Command(Command::goto(node))
    }

    /// Create a command that ends execution
    pub fn end() -> Self {
        NodeOutput::Command(Command::end())
    }
}

impl From<()> for NodeOutput {
    fn from((): ()) -> Self {
        NodeOutput::Done
    }
}
