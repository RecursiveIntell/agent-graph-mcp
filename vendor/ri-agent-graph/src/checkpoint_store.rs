//! Granular checkpoint store for recording node execution attempts.
//!
//! [`CheckpointStore`] provides per-attempt recording with input/output/status,
//! complementing the legacy [`CheckpointSaver`](crate::checkpointer::CheckpointSaver)
//! which operates at the superstep level.

use crate::outcome::Interrupt;
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unique identifier for a graph run.
pub type RunId = String;
/// Unique identifier for a checkpoint-level node execution attempt.
///
/// This is an opaque checkpoint-level ID, distinct from `stack_ids::AttemptId`
/// which represents a retry-lineage primitive. The checkpoint store generates
/// these IDs internally for tracking per-node execution records.
pub type CheckpointAttemptId = String;

/// Status of a node execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AttemptStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
    Cancelled,
}

/// Record of a single node execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt_id: CheckpointAttemptId,
    pub run_id: RunId,
    pub node_id: String,
    pub attempt: u32,
    pub input: Value,
    pub output: Option<Value>,
    pub status: AttemptStatus,
    pub error: Option<String>,
    pub meta: HashMap<String, Value>,
    /// Canonical trace context for this attempt.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_ctx: Option<stack_ids::TraceCtx>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Persisted state of a run, sufficient to resume execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    pub run_id: RunId,
    pub graph_name: String,
    pub status: RunStatus,
    pub attempts: Vec<AttemptRecord>,
    pub state_snapshot: HashMap<String, Value>,
    pub interrupted: Option<Interrupt>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Overall status of a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
    Cancelled,
}

/// Granular checkpoint store for per-attempt recording.
///
/// This trait uses boxed futures instead of async-trait for forward compat.
pub trait CheckpointStore: Send + Sync {
    /// Create a new run and return its ID.
    fn create_run(
        &self,
        graph_name: &str,
    ) -> Pin<Box<dyn Future<Output = Result<RunId>> + Send + '_>>;

    /// Record a new node attempt (status: Running).
    fn record_attempt(
        &self,
        run_id: &str,
        node_id: &str,
        attempt: u32,
        input: &Value,
    ) -> Pin<Box<dyn Future<Output = Result<CheckpointAttemptId>> + Send + '_>>;

    /// Mark an attempt as completed with output.
    fn complete_attempt(
        &self,
        attempt_id: &str,
        output: &Value,
        meta: &HashMap<String, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Mark an attempt as failed.
    fn fail_attempt(
        &self,
        attempt_id: &str,
        error: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Record an interrupt on an attempt.
    fn record_interrupt(
        &self,
        attempt_id: &str,
        interrupt: &Interrupt,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Save the current state snapshot for a run.
    fn save_state_snapshot(
        &self,
        run_id: &str,
        state: &HashMap<String, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Load the full run state (for resume).
    fn load_run(
        &self,
        run_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<RunState>>> + Send + '_>>;

    /// Mark a run as completed.
    fn complete_run(&self, run_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Mark a run as failed.
    fn fail_run(
        &self,
        run_id: &str,
        error: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// Metadata attached to a checkpoint for validation and auditing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    /// Hash of the graph definition at checkpoint time.
    /// Used to detect graph-definition drift on resume.
    pub graph_hash: String,
    /// The run this checkpoint belongs to.
    pub run_id: String,
    /// Node that was active when the checkpoint was taken.
    pub node_id: String,
    /// Superstep number.
    pub step: usize,
    /// When the checkpoint was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Summary of a completed (or failed) graph run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run_id: String,
    pub graph_name: String,
    pub status: RunStatus,
    pub total_nodes_executed: usize,
    pub total_attempts: usize,
    pub failed_attempts: usize,
    /// Phase status: compatibility / migration-only
    pub trace_id: Option<String>,
    /// Canonical trace context for this run.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_ctx: Option<stack_ids::TraceCtx>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl InMemoryCheckpointStore {
    /// Build a [`RunSummary`] for the given run.
    pub async fn summarize_run(&self, run_id: &str) -> Option<RunSummary> {
        let runs = self.runs.read().await;
        let run = runs.get(run_id)?;
        let total_attempts = run.attempts.len();
        let failed_attempts = run
            .attempts
            .iter()
            .filter(|a| a.status == AttemptStatus::Failed)
            .count();
        let trace_id = run.attempts.iter().find_map(|attempt| {
            attempt
                .meta
                .get("trace_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        });
        let trace_ctx = run
            .attempts
            .iter()
            .find_map(|attempt| attempt.trace_ctx.clone());
        let unique_nodes: std::collections::HashSet<&str> =
            run.attempts.iter().map(|a| a.node_id.as_str()).collect();
        Some(RunSummary {
            run_id: run.run_id.clone(),
            graph_name: run.graph_name.clone(),
            status: run.status.clone(),
            total_nodes_executed: unique_nodes.len(),
            total_attempts,
            failed_attempts,
            trace_id,
            trace_ctx,
            started_at: run.created_at,
            finished_at: if run.status == RunStatus::Running {
                None
            } else {
                Some(run.updated_at)
            },
        })
    }
}

/// In-memory checkpoint store for testing and lightweight use.
pub struct InMemoryCheckpointStore {
    runs: Arc<RwLock<HashMap<RunId, RunState>>>,
    attempts: Arc<RwLock<HashMap<CheckpointAttemptId, AttemptRecord>>>,
}

impl InMemoryCheckpointStore {
    pub fn new() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
            attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// List all runs. Useful for testing and inspection.
    pub async fn list_runs(&self) -> Vec<RunState> {
        self.runs.read().await.values().cloned().collect()
    }
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn create_run(
        &self,
        graph_name: &str,
    ) -> Pin<Box<dyn Future<Output = Result<RunId>> + Send + '_>> {
        let graph_name = graph_name.to_string();
        Box::pin(async move {
            let run_id = stack_ids::GraphRunId::random("agent-graph").to_string();
            let now = chrono::Utc::now();
            let run = RunState {
                run_id: run_id.clone(),
                graph_name,
                status: RunStatus::Running,
                attempts: Vec::new(),
                state_snapshot: HashMap::new(),
                interrupted: None,
                created_at: now,
                updated_at: now,
            };
            self.runs.write().await.insert(run_id.clone(), run);
            Ok(run_id)
        })
    }

    fn record_attempt(
        &self,
        run_id: &str,
        node_id: &str,
        attempt: u32,
        input: &Value,
    ) -> Pin<Box<dyn Future<Output = Result<CheckpointAttemptId>> + Send + '_>> {
        let run_id = run_id.to_string();
        let node_id = node_id.to_string();
        let input = input.clone();
        Box::pin(async move {
            let attempt_id =
                stack_ids::GraphCheckpointAttemptId::random("agent-graph-checkpoint").to_string();
            let now = chrono::Utc::now();
            let record = AttemptRecord {
                attempt_id: attempt_id.clone(),
                run_id: run_id.clone(),
                node_id: node_id.clone(),
                attempt,
                input,
                output: None,
                status: AttemptStatus::Running,
                error: None,
                meta: HashMap::new(),
                trace_ctx: None,
                started_at: now,
                finished_at: None,
            };
            self.attempts
                .write()
                .await
                .insert(attempt_id.clone(), record.clone());
            if let Some(run) = self.runs.write().await.get_mut(&run_id) {
                run.attempts.push(record);
                run.updated_at = now;
            }
            Ok(attempt_id)
        })
    }

    fn complete_attempt(
        &self,
        attempt_id: &str,
        output: &Value,
        meta: &HashMap<String, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attempt_id = attempt_id.to_string();
        let output = output.clone();
        let meta = meta.clone();
        Box::pin(async move {
            let now = chrono::Utc::now();
            let mut attempts = self.attempts.write().await;
            if let Some(record) = attempts.get_mut(&attempt_id) {
                record.status = AttemptStatus::Completed;
                record.output = Some(output.clone());
                record.meta = meta.clone();
                record.finished_at = Some(now);
                // Also update in run
                let run_id = record.run_id.clone();
                drop(attempts);
                if let Some(run) = self.runs.write().await.get_mut(&run_id) {
                    if let Some(a) = run.attempts.iter_mut().find(|a| a.attempt_id == attempt_id) {
                        a.status = AttemptStatus::Completed;
                        a.output = Some(output);
                        a.meta = meta;
                        a.finished_at = Some(now);
                    }
                    run.updated_at = now;
                }
            }
            Ok(())
        })
    }

    fn fail_attempt(
        &self,
        attempt_id: &str,
        error: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attempt_id = attempt_id.to_string();
        let error = error.to_string();
        Box::pin(async move {
            let now = chrono::Utc::now();
            let mut attempts = self.attempts.write().await;
            if let Some(record) = attempts.get_mut(&attempt_id) {
                record.status = AttemptStatus::Failed;
                record.error = Some(error.clone());
                record.finished_at = Some(now);
                let run_id = record.run_id.clone();
                drop(attempts);
                if let Some(run) = self.runs.write().await.get_mut(&run_id) {
                    if let Some(a) = run.attempts.iter_mut().find(|a| a.attempt_id == attempt_id) {
                        a.status = AttemptStatus::Failed;
                        a.error = Some(error);
                        a.finished_at = Some(now);
                    }
                    run.updated_at = now;
                }
            }
            Ok(())
        })
    }

    fn record_interrupt(
        &self,
        attempt_id: &str,
        interrupt: &Interrupt,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let attempt_id = attempt_id.to_string();
        let interrupt = interrupt.clone();
        Box::pin(async move {
            let now = chrono::Utc::now();
            let mut attempts = self.attempts.write().await;
            if let Some(record) = attempts.get_mut(&attempt_id) {
                record.status = AttemptStatus::Interrupted;
                record.finished_at = Some(now);
                let run_id = record.run_id.clone();
                drop(attempts);
                if let Some(run) = self.runs.write().await.get_mut(&run_id) {
                    run.interrupted = Some(interrupt);
                    run.status = RunStatus::Interrupted;
                    if let Some(a) = run.attempts.iter_mut().find(|a| a.attempt_id == attempt_id) {
                        a.status = AttemptStatus::Interrupted;
                        a.finished_at = Some(now);
                    }
                    run.updated_at = now;
                }
            }
            Ok(())
        })
    }

    fn save_state_snapshot(
        &self,
        run_id: &str,
        state: &HashMap<String, Value>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let run_id = run_id.to_string();
        let state = state.clone();
        Box::pin(async move {
            if let Some(run) = self.runs.write().await.get_mut(&run_id) {
                run.state_snapshot = state;
                run.updated_at = chrono::Utc::now();
            }
            Ok(())
        })
    }

    fn load_run(
        &self,
        run_id: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<RunState>>> + Send + '_>> {
        let run_id = run_id.to_string();
        Box::pin(async move { Ok(self.runs.read().await.get(&run_id).cloned()) })
    }

    fn complete_run(&self, run_id: &str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let run_id = run_id.to_string();
        Box::pin(async move {
            let mut runs = self.runs.write().await;
            let run = runs
                .get_mut(&run_id)
                .ok_or_else(|| crate::AgentGraphError::RunNotFound(run_id.clone()))?;
            if run.status != RunStatus::Running {
                return Err(crate::AgentGraphError::TerminalStateConflict(run_id));
            }
            run.status = RunStatus::Completed;
            run.updated_at = chrono::Utc::now();
            Ok(())
        })
    }

    fn fail_run(
        &self,
        run_id: &str,
        _error: &str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let run_id = run_id.to_string();
        Box::pin(async move {
            let mut runs = self.runs.write().await;
            let run = runs
                .get_mut(&run_id)
                .ok_or_else(|| crate::AgentGraphError::RunNotFound(run_id.clone()))?;
            if run.status != RunStatus::Running {
                return Err(crate::AgentGraphError::TerminalStateConflict(run_id));
            }
            run.status = RunStatus::Failed;
            run.updated_at = chrono::Utc::now();
            Ok(())
        })
    }
}
