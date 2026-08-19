use crate::error::AgentGraphError;
use std::sync::Arc;
use std::time::Duration;

/// Type alias for retry predicate function.
pub type RetryPredicate = Arc<dyn Fn(&AgentGraphError) -> bool + Send + Sync>;

/// Retry policy for node execution.
#[derive(Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including first try)
    pub max_attempts: usize,
    /// Initial delay between retries
    pub initial_interval: Duration,
    /// Multiplicative backoff factor
    pub backoff_factor: f64,
    /// Maximum delay between retries
    pub max_interval: Duration,
    /// Whether to add random jitter to delays
    pub jitter: bool,
    /// Optional predicate to determine if an error is retryable
    pub retry_on: Option<RetryPredicate>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_interval: Duration::from_secs(1),
            backoff_factor: 2.0,
            max_interval: Duration::from_secs(60),
            jitter: true,
            retry_on: None,
        }
    }
}

impl RetryPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_attempts(mut self, n: usize) -> Self {
        self.max_attempts = n;
        self
    }

    pub fn with_initial_interval(mut self, d: Duration) -> Self {
        self.initial_interval = d;
        self
    }

    pub fn with_backoff_factor(mut self, f: f64) -> Self {
        self.backoff_factor = f;
        self
    }

    pub fn with_max_interval(mut self, d: Duration) -> Self {
        self.max_interval = d;
        self
    }

    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    pub fn with_retry_on(
        mut self,
        predicate: impl Fn(&AgentGraphError) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.retry_on = Some(Arc::new(predicate));
        self
    }

    /// Check if a given error should be retried
    pub fn should_retry(&self, error: &AgentGraphError) -> bool {
        match &self.retry_on {
            Some(predicate) => predicate(error),
            None => true,
        }
    }

    /// Calculate delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let base = self.initial_interval.as_secs_f64() * self.backoff_factor.powi(attempt as i32);
        let capped = base.min(self.max_interval.as_secs_f64());
        if self.jitter {
            let jitter_factor = jitter_factor();
            Duration::from_secs_f64(capped * jitter_factor)
        } else {
            Duration::from_secs_f64(capped)
        }
    }
}

/// Simple deterministic jitter factor using hash-based pseudo-randomness.
fn jitter_factor() -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let hash = hasher.finish();
    // Normalize to [0.5, 1.0] range for reasonable jitter
    0.5 + (hash as f64 / u64::MAX as f64) * 0.5
}

impl std::fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryPolicy")
            .field("max_attempts", &self.max_attempts)
            .field("initial_interval", &self.initial_interval)
            .field("backoff_factor", &self.backoff_factor)
            .field("max_interval", &self.max_interval)
            .field("jitter", &self.jitter)
            .field("retry_on", &self.retry_on.is_some())
            .finish()
    }
}
