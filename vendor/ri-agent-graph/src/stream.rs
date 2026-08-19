use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Events emitted during graph execution.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamEvent {
    /// Graph execution started
    GraphStart { graph_name: Option<String> },
    /// Graph execution ended
    GraphEnd { graph_name: Option<String> },
    /// Node execution started
    NodeStart { node: String },
    /// Node execution ended
    NodeEnd { node: String },
    /// State was updated by a node
    StateUpdate {
        node: String,
        updates: HashMap<String, Value>,
    },
    /// A parallel superstep started
    SuperstepStart { step: usize, nodes: Vec<String> },
    /// A parallel superstep ended
    SuperstepEnd { step: usize },
    /// Execution was interrupted
    Interrupt { node: String, value: Option<Value> },
    /// Custom event emitted by a node
    Custom(Value),
}

/// Stream mode controls what events are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// Emit full state after each node execution
    Values,
    /// Emit only changed state keys after each node execution
    Updates,
    /// Emit all events
    #[default]
    Events,
}
