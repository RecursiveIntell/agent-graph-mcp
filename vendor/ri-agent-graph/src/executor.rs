//! Executor abstraction for running node attempts.
//!
//! The [`Executor`] trait allows plugging in different execution strategies
//! (in-process, external job queue, etc.) without changing graph logic.

use crate::command::NodeOutput;
use crate::config::GraphConfig;
use crate::node::Node;
use crate::state::AgentState;
use crate::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Trait for executing individual node attempts.
///
/// The default [`InProcessExecutor`] runs nodes directly in the current tokio runtime.
/// Alternative implementations (e.g., tauri-queue) can be provided behind feature flags.
///
/// Uses boxed futures instead of async-trait.
pub trait Executor: Send + Sync {
    /// Execute a single node attempt.
    ///
    /// The executor owns the `Arc<dyn Node>`, `AgentState`, and `GraphConfig`
    /// so the returned future is `'static` and can be spawned on a task.
    fn execute_node(
        &self,
        node: Arc<dyn Node>,
        state: AgentState,
        config: GraphConfig,
    ) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>>;
}

/// Default executor: runs nodes directly in the current tokio runtime.
pub struct InProcessExecutor;

impl InProcessExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InProcessExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for InProcessExecutor {
    fn execute_node(
        &self,
        node: Arc<dyn Node>,
        state: AgentState,
        config: GraphConfig,
    ) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>> {
        Box::pin(async move { node.execute(&state, &config).await })
    }
}
