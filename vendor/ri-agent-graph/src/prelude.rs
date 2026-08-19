//! Convenience re-exports for common use

// Core graph types
pub use crate::config::GraphConfig;
pub use crate::edge::EdgeType;
pub use crate::error::{AgentGraphError, CheckpointStoreOperation, Result};
pub use crate::graph::{AgentGraph, AgentGraphBuilder, END, START};
pub use crate::state::{AgentState, StateLimits, StateSnapshot, StateTransaction};

// Node types
pub use crate::command::{Command, Navigation, NodeOutput, SendOp};
pub use crate::join::JoinNode;
pub use crate::node::{FnNode, Node};
pub use crate::payload::{Payload, PayloadContext, PayloadError, PayloadNode, PayloadOutput};

// Routing
pub use crate::router::{FnRouter, RouterOutput, RoutingFunction};

// Execution outcomes
pub use crate::outcome::{Interrupt, InterruptKind, NodeOutcome};

// Event system
pub use crate::event_sink::{
    CallbackEventSink, ChannelEventSink, CompositeEventSink, EventSink, GraphEvent,
    NodeOutcomeKind, NoopEventSink,
};
pub use crate::stream::{StreamEvent, StreamMode};

// Checkpoint store (granular)
pub use crate::checkpoint_store::{
    AttemptRecord, AttemptStatus, CheckpointAttemptId, CheckpointMetadata, CheckpointStore,
    InMemoryCheckpointStore, RunId, RunState, RunStatus, RunSummary,
};

// Legacy checkpoint system
pub use crate::checkpointer::{CheckpointSaver, MemorySaver};

// Executor
pub use crate::executor::{Executor, InProcessExecutor};

// Reducers
pub use crate::reducer::{
    AddReducer, AppendReducer, FnReducer, LastWriteWins, MergeReducer, Reducer,
};

// Retry
pub use crate::retry::RetryPolicy;

// Interrupt (legacy)
pub use crate::interrupt::{ExecutionResult, InterruptCheckpoint, InterruptConfig};

// Re-export macros
pub use crate::{node, router};

#[cfg(feature = "checkpointing")]
pub use crate::checkpoint::{Checkpoint, CheckpointManager};
pub use crate::execution_cursor::{ExecutionCursor, InterruptPhase};

#[cfg(feature = "checkpointing")]
pub use crate::checkpointer::SqliteSaver;
