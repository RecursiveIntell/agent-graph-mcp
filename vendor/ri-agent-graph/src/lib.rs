//! # agent-graph
//!
//! Graph-based agent orchestration for Rust - LangGraph for the Rust ecosystem.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ri_agent_graph::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let graph = AgentGraph::builder()
//!         .add_node("step1", node!(|state| async move {
//!             state.set("count", 1).await?;
//!             Ok(())
//!         }))
//!         .add_node("step2", node!(|state| async move {
//!             let count: i32 = state.get("count").await?;
//!             state.set("count", count + 1).await?;
//!             Ok(())
//!         }))
//!         .add_edge("step1", "step2")
//!         .build()?;
//!
//!     let state = AgentState::new();
//!     let result = graph.execute("step1", state).await?;
//!
//!     let final_count: i32 = result.get("count").await?;
//!     assert_eq!(final_count, 2);
//!
//!     Ok(())
//! }
//! ```

pub mod builder;
pub mod checkpoint_store;
pub mod checkpointer;
pub mod command;
pub mod config;
pub mod edge;
pub mod engine;
pub mod error;
pub mod event_sink;
pub mod execution_cursor;
pub mod executor;
pub mod graph;
pub mod interrupt;
pub mod join;
pub mod node;
pub mod outcome;
pub mod payload;
pub mod prelude;
pub mod receipt;
pub mod reducer;
pub mod retry;
pub mod router;
pub mod state;
pub mod stream;

#[cfg(feature = "checkpointing")]
pub mod checkpoint;

pub use error::{AgentGraphError, CheckpointStoreOperation, Result};
pub use graph::{AgentGraph, END, START};
pub use receipt::{ExecutionOutcome, GraphExecutionReceiptV1, StepExecutionReceiptV1};
pub use state::AgentState;
