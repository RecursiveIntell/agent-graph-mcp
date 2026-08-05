//! Model invocation executor — a generic seam for attaching local model
//! backends (e.g. proveKV + Qwen) to graph LLM nodes without putting
//! lease material in prompts, graph state, provider bodies, or logs.
//!
//! When an executor is attached to a RunContext, LlmNode checks it before
//! falling through to the default HTTP/OpenRouter provider path. If no
//! executor is attached, the existing path operates identically.

use std::sync::Arc;

/// Opaque handle returned by an executor on successful acquisition.
/// The lease digest is the only field safe for logs/receipts.
#[derive(Debug, Clone)]
pub struct BackendHandle {
    /// Non-replayable lease digest (BLAKE3 over serialized lease).
    /// Safe for logs — contains no bearer secret.
    pub lease_digest: String,
    /// Content-addressed state ID. Not a bearer secret.
    pub state_id: String,
}

/// A model invocation executor that can be attached to a graph run.
///
/// Implementations provide a local model backend (e.g. proveKV + Qwen)
/// as an alternative to the default HTTP/OpenRouter provider path.
///
/// # Security invariants
///
/// - Lease material must never enter the rendered prompt, graph state,
///   node output, provider request body, or logs.
/// - Only the `lease_digest` in `BackendHandle` is safe for receipts.
/// - Expired, revoked, cross-principal, or write-widened leases must
///   be rejected before any state lookup.
/// - The default provider path must remain identical when no executor
///   is attached.
pub trait ModelInvocationExecutor: Send + Sync {
    /// Called before LlmNode::execute. Returns a backend handle if the
    /// run/node/attempt tuple is eligible for local execution, or `None`
    /// to fall through to the default provider path.
    fn acquire(&self, run_id: &str, node_id: &str, attempt_id: u64) -> Option<BackendHandle>;

    /// Called after node completion (success or failure). The executor
    /// may release the lease, update accounting, or perform cleanup.
    fn release(&self, handle: BackendHandle);
}

/// A no-op executor used in tests to verify the seam without model calls.
pub struct TestExecutor {
    /// If true, acquire always returns a handle.
    pub always_acquire: bool,
    /// Tracks released handles for test assertions.
    pub released: std::sync::Mutex<Vec<BackendHandle>>,
}

impl TestExecutor {
    pub fn new(always_acquire: bool) -> Self {
        Self {
            always_acquire,
            released: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl ModelInvocationExecutor for TestExecutor {
    fn acquire(&self, _run_id: &str, _node_id: &str, _attempt_id: u64) -> Option<BackendHandle> {
        if self.always_acquire {
            Some(BackendHandle {
                lease_digest: "test-lease-digest-0000000000000000000000000000000000000000000000000000000000000000".into(),
                state_id: "test-state-id-0000000000000000000000000000000000000000000000000000000000000000".into(),
            })
        } else {
            None
        }
    }

    fn release(&self, handle: BackendHandle) {
        self.released.lock().unwrap().push(handle);
    }
}

/// Thread-safe wrapper for an optional executor.
#[derive(Clone)]
pub struct ExecutorHandle {
    inner: Option<Arc<dyn ModelInvocationExecutor>>,
}

impl ExecutorHandle {
    /// Create a handle with no executor attached.
    pub fn none() -> Self {
        Self { inner: None }
    }

    /// Attach an executor.
    pub fn with(executor: Arc<dyn ModelInvocationExecutor>) -> Self {
        Self {
            inner: Some(executor),
        }
    }

    /// Try to acquire a backend handle. Returns None if no executor is
    /// attached or the executor declines.
    pub fn try_acquire(
        &self,
        run_id: &str,
        node_id: &str,
        attempt_id: u64,
    ) -> Option<BackendHandle> {
        self.inner
            .as_ref()
            .and_then(|e| e.acquire(run_id, node_id, attempt_id))
    }

    /// Release a previously acquired handle.
    pub fn release(&self, handle: BackendHandle) {
        if let Some(executor) = &self.inner {
            executor.release(handle);
        }
    }

    /// True if an executor is attached.
    pub fn is_attached(&self) -> bool {
        self.inner.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_executor_returns_none() {
        let handle = ExecutorHandle::none();
        assert!(!handle.is_attached());
        assert!(handle.try_acquire("r1", "n1", 0).is_none());
    }

    #[test]
    fn test_executor_acquires_when_configured() {
        let executor = Arc::new(TestExecutor::new(true));
        let handle = ExecutorHandle::with(executor.clone());
        assert!(handle.is_attached());

        let backend = handle.try_acquire("r1", "n1", 0);
        assert!(backend.is_some());
        assert!(backend
            .unwrap()
            .lease_digest
            .starts_with("test-lease-digest"));

        // Release should be tracked.
        let backend = handle.try_acquire("r1", "n2", 1).unwrap();
        handle.release(backend);
        assert_eq!(executor.released.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_executor_declines_when_configured() {
        let executor = Arc::new(TestExecutor::new(false));
        let handle = ExecutorHandle::with(executor);
        assert!(handle.is_attached());
        assert!(handle.try_acquire("r1", "n1", 0).is_none());
    }

    #[test]
    fn handle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ExecutorHandle>();
        assert_send_sync::<BackendHandle>();
    }
}
