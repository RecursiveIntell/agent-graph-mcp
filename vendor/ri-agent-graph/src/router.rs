use crate::config::GraphConfig;
use crate::error::Result;
use crate::state::AgentState;
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

/// Router output determines where execution goes next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterOutput {
    /// Route to a single node, or end execution (None)
    Next(Option<String>),
    /// Fan-out to multiple nodes simultaneously
    FanOut(Vec<String>),
}

impl From<Option<String>> for RouterOutput {
    fn from(opt: Option<String>) -> Self {
        RouterOutput::Next(opt)
    }
}

impl From<String> for RouterOutput {
    fn from(s: String) -> Self {
        RouterOutput::Next(Some(s))
    }
}

impl From<Vec<String>> for RouterOutput {
    fn from(v: Vec<String>) -> Self {
        RouterOutput::FanOut(v)
    }
}

/// Determines which node to visit next based on current state.
#[async_trait]
pub trait RoutingFunction: Send + Sync {
    /// Returns routing decision.
    /// Return `RouterOutput::Next(None)` to end execution.
    async fn route(&self, state: &AgentState, config: &GraphConfig) -> Result<RouterOutput>;

    /// Stable caller-visible identity for routing semantics. Implementations
    /// should override this when the route behavior depends on configuration.
    fn semantic_digest(&self) -> String {
        std::any::type_name::<Self>().to_string()
    }
}

/// Helper to create a router from an async function
pub struct FnRouter<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<RouterOutput>> + Send>>
        + Send
        + Sync,
{
    func: F,
}

impl<F> FnRouter<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<RouterOutput>> + Send>>
        + Send
        + Sync,
{
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

#[async_trait]
impl<F> RoutingFunction for FnRouter<F>
where
    F: Fn(&AgentState, &GraphConfig) -> Pin<Box<dyn Future<Output = Result<RouterOutput>> + Send>>
        + Send
        + Sync,
{
    async fn route(&self, state: &AgentState, config: &GraphConfig) -> Result<RouterOutput> {
        (self.func)(state, config).await
    }
}

/// Helper macro to create a router from an async closure.
///
/// # Forms
///
/// ```ignore
/// // Basic form (backward compatible) - body returns Result<Option<String>>
/// router!(|state| async move {
///     let value: i32 = state.get("value").await?;
///     Ok(if value > 5 { Some("high".to_string()) } else { None })
/// })
///
/// // With config - body returns Result<impl Into<RouterOutput>>
/// router!(|state, config| async move {
///     let value: i32 = state.get("value").await?;
///     Ok(RouterOutput::FanOut(vec!["a".to_string(), "b".to_string()]))
/// })
/// ```
#[macro_export]
macro_rules! router {
    // Form 1: |state| - backward compatible, returns Result<impl Into<RouterOutput>>
    (|$state:ident| async move $body:block) => {
        Box::new($crate::router::FnRouter::new(
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
    // Form 2: |state, config| - has access to config
    (|$state:ident, $config:ident| async move $body:block) => {
        Box::new($crate::router::FnRouter::new(
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
}
