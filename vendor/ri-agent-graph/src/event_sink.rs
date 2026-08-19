//! Structured event pipeline for graph execution.
//!
//! [`EventSink`] is the trait for emitting runtime events. Implementations
//! must be non-blocking — use channels or fire-and-forget internally.

#![allow(deprecated)] // Internal code constructs/destructures GraphEvent legacy fields

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::stream::StreamEvent;

/// Structured runtime event emitted during graph execution.
///
/// Every variant carries both legacy and canonical trace/retry fields:
///
/// - `trace_id: String` — legacy correlation ID.
/// - `trace_ctx: Option<stack_ids::TraceCtx>` — canonical trace context.
/// - `attempt: u32` / `attempt_id: Option<stack_ids::AttemptId>` — retry family.
/// - `trial_id: Option<stack_ids::TrialId>` — individual execution trial (on
///   variants representing a single execution attempt).
///
/// ## Legacy field phase status: compatibility / migration-only
///
/// The `trace_id: String` and `attempt: u32` fields use primitive types for
/// backward compatibility. The canonical replacements are `stack_ids::TraceCtx`,
/// `stack_ids::AttemptId`, and `stack_ids::TrialId`.
///
/// **Removal condition**: legacy fields removed when all event consumers migrate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphEvent {
    /// A graph run started.
    RunStart {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        graph_name: Option<String>,
    },
    /// A graph run ended.
    RunEnd {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
    },
    /// A node execution attempt started.
    NodeStart {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        node_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        attempt: u32,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        attempt_id: Option<stack_ids::AttemptId>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trial_id: Option<stack_ids::TrialId>,
    },
    /// A node execution attempt ended.
    NodeEnd {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        node_id: String,
        outcome: NodeOutcomeKind,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        attempt_id: Option<stack_ids::AttemptId>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trial_id: Option<stack_ids::TrialId>,
    },
    /// A streaming token from a payload node.
    Token {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        node_id: String,
        token: String,
    },
    /// A checkpoint was written.
    CheckpointWritten {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        /// The checkpoint-level attempt ID (distinct from `stack_ids::AttemptId`).
        #[serde(alias = "attempt_id")]
        checkpoint_attempt_id: String,
    },
    /// An interrupt was raised by a node.
    InterruptRaised {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        node_id: String,
        kind: String,
        payload: Value,
    },
    /// State was updated by a node.
    StateUpdate {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        node_id: String,
        updates: HashMap<String, Value>,
    },
    /// A parallel superstep started.
    SuperstepStart {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        step: usize,
        nodes: Vec<String>,
    },
    /// A parallel superstep ended.
    SuperstepEnd {
        run_id: String,
        /// Phase status: compatibility / migration-only.
        /// Removal condition: all consumers migrate to `trace_ctx`/`attempt_id`/`trial_id`.
        #[deprecated(
            note = "Use trace_ctx/attempt_id/trial_id instead. Will be removed when all consumers migrate."
        )]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        step: usize,
    },
    /// Remaining parallel branches were cancelled after a sibling failed.
    ParallelCancellation {
        run_id: String,
        #[deprecated(note = "Use trace_ctx instead.")]
        trace_id: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        trace_ctx: Option<stack_ids::TraceCtx>,
        /// Cancellation cannot undo effects already handed to external systems.
        external_effects_may_have_escaped: bool,
    },
}

/// Summary of a node's outcome (for event reporting).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeOutcomeKind {
    Success,
    Failed,
    Interrupted,
}

/// Trait for emitting structured runtime events.
///
/// Implementations must be non-blocking. If the downstream consumer
/// is slow, implementations should drop events rather than block.
pub trait EventSink: Send + Sync {
    /// Emit a structured event. Must not block.
    fn emit(&self, event: GraphEvent);
}

/// No-op event sink that discards all events.
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: GraphEvent) {}
}

/// Event sink that forwards to a `tokio::sync::mpsc::Sender<StreamEvent>`.
///
/// Bridges the new [`GraphEvent`] system to the legacy [`StreamEvent`] channel
/// used by [`AgentGraph::stream()`](crate::graph::AgentGraph::stream).
pub struct ChannelEventSink {
    sender: tokio::sync::mpsc::Sender<StreamEvent>,
}

impl ChannelEventSink {
    pub fn new(sender: tokio::sync::mpsc::Sender<StreamEvent>) -> Self {
        Self { sender }
    }
}

impl EventSink for ChannelEventSink {
    fn emit(&self, event: GraphEvent) {
        let stream_event = match event {
            GraphEvent::RunStart { graph_name, .. } => StreamEvent::GraphStart { graph_name },
            GraphEvent::RunEnd { .. } => StreamEvent::GraphEnd { graph_name: None },
            GraphEvent::NodeStart { node_id, .. } => StreamEvent::NodeStart { node: node_id },
            GraphEvent::NodeEnd { node_id, .. } => StreamEvent::NodeEnd { node: node_id },
            GraphEvent::Token {
                run_id,
                node_id,
                token,
                ..
            } => StreamEvent::Custom(serde_json::json!({
                "type": "token",
                "run_id": run_id,
                "node": node_id,
                "token": token,
            })),
            GraphEvent::InterruptRaised {
                node_id, payload, ..
            } => StreamEvent::Interrupt {
                node: node_id,
                value: Some(payload),
            },
            GraphEvent::StateUpdate {
                node_id, updates, ..
            } => StreamEvent::StateUpdate {
                node: node_id,
                updates,
            },
            GraphEvent::SuperstepStart { step, nodes, .. } => {
                StreamEvent::SuperstepStart { step, nodes }
            }
            GraphEvent::SuperstepEnd { step, .. } => StreamEvent::SuperstepEnd { step },
            GraphEvent::ParallelCancellation { .. } => return,
            GraphEvent::CheckpointWritten { .. } => return, // no legacy equivalent
        };
        // try_send: non-blocking, drops if channel full
        let _ = self.sender.try_send(stream_event);
    }
}

/// Event sink that calls a user-provided closure for each event.
pub struct CallbackEventSink<F: Fn(GraphEvent) + Send + Sync> {
    callback: F,
}

impl<F: Fn(GraphEvent) + Send + Sync> CallbackEventSink<F> {
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F: Fn(GraphEvent) + Send + Sync> EventSink for CallbackEventSink<F> {
    fn emit(&self, event: GraphEvent) {
        (self.callback)(event);
    }
}

/// Event sink that fans out to multiple sinks.
pub struct CompositeEventSink {
    sinks: Vec<Arc<dyn EventSink>>,
}

impl CompositeEventSink {
    pub fn new(sinks: Vec<Arc<dyn EventSink>>) -> Self {
        Self { sinks }
    }
}

impl EventSink for CompositeEventSink {
    fn emit(&self, event: GraphEvent) {
        for sink in &self.sinks {
            sink.emit(event.clone());
        }
    }
}
