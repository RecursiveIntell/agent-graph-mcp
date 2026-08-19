use crate::command::NodeOutput;
use crate::config::GraphConfig;
use crate::error::Result;
use crate::state::AgentState;
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

/// A node in the agent graph.
/// Nodes perform actions and can modify the shared state.
#[async_trait]
pub trait Node: Send + Sync {
    /// Execute this node
    async fn execute(&self, state: &AgentState, config: &GraphConfig) -> Result<NodeOutput>;

    /// Optional: Get a name for this node (for debugging)
    fn name(&self) -> Option<&str> {
        None
    }
}

/// Helper to create a node from an async function
pub struct FnNode<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>>
        + Send
        + Sync,
{
    func: F,
    name: Option<String>,
}

impl<F> FnNode<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>>
        + Send
        + Sync,
{
    pub fn new(func: F) -> Self {
        Self { func, name: None }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[async_trait]
impl<F> Node for FnNode<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>>
        + Send
        + Sync,
{
    async fn execute(&self, state: &AgentState, config: &GraphConfig) -> Result<NodeOutput> {
        (self.func)(state, config).await
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Helper macro to create a node from an async closure.
///
/// # Forms
///
/// ```ignore
/// // Basic form (backward compatible) - body returns Result<()>
/// node!(|state| async move {
///     state.set("key", "value").await?;
///     Ok(())
/// })
///
/// // Named form - body returns Result<()>
/// node!("my_node", |state| async move {
///     state.set("key", "value").await?;
///     Ok(())
/// })
///
/// // With config - body returns Result<NodeOutput> or Result<()>
/// node!(|state, config| async move {
///     state.set("key", "value").await?;
///     Ok(NodeOutput::Done)
/// })
///
/// // Named with config
/// node!("my_node", |state, config| async move {
///     state.set("key", "value").await?;
///     Ok(())
/// })
/// ```
#[macro_export]
macro_rules! node {
    // Form 1: |state| - backward compatible, body returns Result<impl Into<NodeOutput>>
    (|$state:ident| async move $body:block) => {
        Box::new($crate::node::FnNode::new(
            |__state: &$crate::state::AgentState, __config: &$crate::config::GraphConfig| {
                let $state = __state.clone();
                let _ = __config;
                Box::pin(async move {
                    let __result = (|| async move { $body })().await;
                    __result.map(::std::convert::Into::into)
                })
            },
        ))
    };
    // Form 2: named |state|
    ($name:expr, |$state:ident| async move $body:block) => {
        Box::new(
            $crate::node::FnNode::new(
                |__state: &$crate::state::AgentState, __config: &$crate::config::GraphConfig| {
                    let $state = __state.clone();
                    let _ = __config;
                    Box::pin(async move {
                        let __result = (|| async move { $body })().await;
                        __result.map(::std::convert::Into::into)
                    })
                },
            )
            .with_name($name),
        )
    };
    // Form 3: |state, config| - has access to config
    (|$state:ident, $config:ident| async move $body:block) => {
        Box::new($crate::node::FnNode::new(
            |__state: &$crate::state::AgentState, __config: &$crate::config::GraphConfig| {
                let $state = __state.clone();
                let $config = __config.clone();
                Box::pin(async move {
                    let __result = (|| async move { $body })().await;
                    __result.map(::std::convert::Into::into)
                })
            },
        ))
    };
    // Form 4: named |state, config|
    ($name:expr, |$state:ident, $config:ident| async move $body:block) => {
        Box::new(
            $crate::node::FnNode::new(
                |__state: &$crate::state::AgentState, __config: &$crate::config::GraphConfig| {
                    let $state = __state.clone();
                    let $config = __config.clone();
                    Box::pin(async move {
                        let __result = (|| async move { $body })().await;
                        __result.map(::std::convert::Into::into)
                    })
                },
            )
            .with_name($name),
        )
    };
}
