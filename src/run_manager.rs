use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ri_agent_graph::config::GraphConfig;
use ri_agent_graph::event_sink::GraphEvent;
use ri_agent_graph::state::{AgentState, StateLimits};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Notify;

use crate::compiler::{compile, CompileContext};
use crate::model_executor::ExecutorHandle;
use crate::evidence::{bundle, digest, redact, validate_witness_dependencies};
use crate::spec::{ensure_size, GraphSpec, MAX_OUTPUT_BYTES, MAX_STATE_BYTES};
use crate::store::PersistentStore;

const MAX_RUNS: usize = 100;
const MAX_ACTIVE_RUNS: usize = 8;

fn is_terminal(status: &str) -> bool {
    matches!(
        status,
        "completed" | "failed" | "cancelled" | "checkpointed"
    )
}

#[derive(Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub trace: String,
    pub graph_id: String,
    pub graph_version: String,
    pub status: String,
    pub success: Option<bool>,
    pub input: Value,
    pub state: Value,
    pub final_state: Value,
    pub steps: Vec<Value>,
    pub error: Option<String>,
    pub events: VecDeque<Value>,
    pub next_cursor: u64,
    pub dropped_events: u64,
    pub receipt: Value,
    pub bundle: Value,
    pub persistence_status: String,
    pub persistence_error: Option<String>,
    pub budgets: Option<RunBudgets>,
    pub budget_counters: BudgetCounters,
    pub budget_exhausted: Option<String>,
    pub checkpoint_id: Option<String>,
    pub checkpoint_digest: Option<String>,
    pub approval: Option<Value>,
    pub resumed: bool,
    pub cancelled: Arc<AtomicBool>,
    pub cancellation: Arc<Notify>,
}

impl RunRecord {
    pub fn public(&self) -> Value {
        serde_json::json!({
            "run_id":self.run_id,"trace":self.trace,"graph_id":self.graph_id,"graph_version":self.graph_version,
            "storage_class": if self.persistence_status == "durable_terminal" { "sqlite_terminal_projection" } else { "volatile" },
            "persistence_status":self.persistence_status,"persistence_error":self.persistence_error,
            "status":self.status,"success":self.success,"final_state":self.final_state,
            "state":self.state,"steps":self.steps,"error":self.error,"receipt":self.receipt,
            "budgets":self.budgets,"budget_counters":self.budget_counters,
            "budget_exhausted":self.budget_exhausted,
            "checkpoint": self.checkpoint_id.as_ref().zip(self.checkpoint_digest.as_ref()).map(|(id, digest)| serde_json::json!({"checkpoint_id":id,"checkpoint_digest":digest})),
            "replay_capability":self.receipt.get("replay_capability").and_then(Value::as_str).unwrap_or("integrity_only")
        })
    }
}

/// The only budgets accepted by `graph_run_start`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunBudgets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_wall_clock_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_nodes: Option<u64>,
    /// Enforced atomically before each provider invocation: attempts are
    /// reserved pre-invocation, so failed or timed-out calls still count and
    /// concurrent nodes cannot race past the limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_llm_calls: Option<u64>,
}

impl RunBudgets {
    pub fn parse(value: Option<&Value>) -> Result<Option<Self>, String> {
        let Some(value) = value.filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        let object = value
            .as_object()
            .ok_or_else(|| "budgets must be an object".to_owned())?;
        if object.is_empty() {
            return Err("budgets must contain at least one supported positive integer".into());
        }

        let mut budgets = Self {
            max_wall_clock_ms: None,
            max_nodes: None,
            max_llm_calls: None,
        };
        for (key, raw) in object {
            let slot = match key.as_str() {
                "max_wall_clock_ms" => &mut budgets.max_wall_clock_ms,
                "max_nodes" => &mut budgets.max_nodes,
                "max_llm_calls" => &mut budgets.max_llm_calls,
                _ => return Err(format!("unknown budget field '{key}'")),
            };
            let number = raw
                .as_u64()
                .filter(|number| *number > 0)
                .ok_or_else(|| format!("budget '{key}' must be a positive integer"))?;
            *slot = Some(number);
        }
        if budgets.max_wall_clock_ms.is_none()
            && budgets.max_nodes.is_none()
            && budgets.max_llm_calls.is_none()
        {
            return Err("budgets must contain an enforceable supported field".into());
        }
        Ok(Some(budgets))
    }

    pub fn requested_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct BudgetCounters {
    pub nodes: u64,
    pub llm_calls: u64,
    pub wall_clock_ms: u64,
}

#[derive(Clone)]
pub struct RunManager {
    inner: Arc<Mutex<Inner>>,
    counter: Arc<AtomicU64>,
    /// Provider API key attached to http(s) llm-pipeline calls as a Bearer
    /// header. Never serialized; codex-app-server:// carries no auth.
    api_key: Option<String>,
}
struct Inner {
    runs: HashMap<String, RunRecord>,
    order: VecDeque<String>,
    reserved_async_slots: usize,
}

impl Default for RunManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                runs: HashMap::new(),
                order: VecDeque::new(),
                reserved_async_slots: 0,
            })),
            counter: Arc::new(AtomicU64::new(1)),
            api_key: None,
        }
    }
}

impl RunManager {
    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }
    pub fn allocate(
        &self,
        graph_id: &str,
        graph_version: &str,
        input: Value,
    ) -> Result<String, String> {
        self.allocate_with_budgets(graph_id, graph_version, input, None)
    }

    pub fn allocate_with_budgets(
        &self,
        graph_id: &str,
        graph_version: &str,
        input: Value,
        budgets: Option<RunBudgets>,
    ) -> Result<String, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let run_id = format!("run-{millis:x}-{n:x}");
        let record = RunRecord {
            run_id: run_id.clone(),
            trace: format!("trace-{millis:x}-{n:x}"),
            graph_id: graph_id.into(),
            graph_version: graph_version.into(),
            status: "accepted".into(),
            success: None,
            input,
            state: Value::Null,
            final_state: Value::Null,
            steps: vec![],
            error: None,
            events: VecDeque::new(),
            next_cursor: 0,
            dropped_events: 0,
            receipt: Value::Null,
            bundle: Value::Null,
            persistence_status: "volatile_active".into(),
            persistence_error: None,
            budgets,
            budget_counters: BudgetCounters::default(),
            budget_exhausted: None,
            checkpoint_id: None,
            checkpoint_digest: None,
            approval: None,
            resumed: false,
            cancelled: Arc::new(AtomicBool::new(false)),
            cancellation: Arc::new(Notify::new()),
        };
        self.insert_record(record)
    }

    fn insert_record(&self, record: RunRecord) -> Result<String, String> {
        let run_id = record.run_id.clone();
        let mut inner = self.inner.lock().expect("run registry poisoned");
        if inner.order.len() == MAX_RUNS {
            let Some(old) = inner.order.iter().find_map(|id| {
                inner
                    .runs
                    .get(id)
                    .filter(|run| is_terminal(&run.status))
                    .map(|_| id.clone())
            }) else {
                return Err(format!(
                    "run retention capacity reached: {MAX_RUNS} live runs cannot be evicted"
                ));
            };
            inner.order.retain(|id| id != &old);
            inner.runs.remove(&old);
        }
        inner.order.push_back(run_id.clone());
        inner.runs.insert(run_id.clone(), record);
        Ok(run_id)
    }

    pub fn allocate_resumed(
        &self,
        run_id: &str,
        graph_id: &str,
        graph_version: &str,
        input: Value,
        state: Value,
        budgets: Option<RunBudgets>,
        checkpoint_id: &str,
        checkpoint_digest: &str,
        approval: Option<Value>,
    ) -> Result<String, String> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let record = RunRecord {
            run_id: run_id.to_owned(),
            trace: format!("trace-{millis:x}-resume"),
            graph_id: graph_id.into(),
            graph_version: graph_version.into(),
            status: "accepted".into(),
            success: None,
            input,
            state,
            final_state: Value::Null,
            steps: vec![],
            error: None,
            events: VecDeque::new(),
            next_cursor: 0,
            dropped_events: 0,
            receipt: Value::Null,
            bundle: Value::Null,
            persistence_status: "volatile_active".into(),
            persistence_error: None,
            budgets,
            budget_counters: BudgetCounters::default(),
            budget_exhausted: None,
            checkpoint_id: Some(checkpoint_id.to_owned()),
            checkpoint_digest: Some(checkpoint_digest.to_owned()),
            approval,
            resumed: true,
            cancelled: Arc::new(AtomicBool::new(false)),
            cancellation: Arc::new(Notify::new()),
        };
        self.insert_record(record)
    }

    pub fn mark_checkpointed(
        &self,
        id: &str,
        checkpoint_id: &str,
        checkpoint_digest: &str,
    ) -> Result<(), String> {
        self.update(id, |run| {
            run.status = "checkpointed".into();
            run.checkpoint_id = Some(checkpoint_id.to_owned());
            run.checkpoint_digest = Some(checkpoint_digest.to_owned());
        })
    }

    /// Atomically reserves one of the bounded execution slots for an accepted run.
    pub fn admit_async(&self, id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
        let active_runs = inner
            .runs
            .values()
            .filter(|run| run.status == "running")
            .count();
        if active_runs + inner.reserved_async_slots >= MAX_ACTIVE_RUNS {
            return Err(format!(
                "active run capacity reached: {MAX_ACTIVE_RUNS} concurrent runs"
            ));
        }
        let run = inner.runs.get_mut(id).ok_or("run not found")?;
        if run.status != "accepted" {
            return Err(format!("run '{id}' is not awaiting admission"));
        }
        run.status = "running".into();
        Ok(())
    }

    /// Reserve capacity before an irreversible durable transition. The caller
    /// must either activate or release this reservation.
    pub fn reserve_async_slot(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
        let active_runs = inner
            .runs
            .values()
            .filter(|run| run.status == "running")
            .count();
        if active_runs + inner.reserved_async_slots >= MAX_ACTIVE_RUNS {
            return Err(format!(
                "active run capacity reached: {MAX_ACTIVE_RUNS} concurrent runs"
            ));
        }
        inner.reserved_async_slots += 1;
        Ok(())
    }

    pub fn release_async_slot(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.reserved_async_slots = inner.reserved_async_slots.saturating_sub(1);
        }
    }

    pub fn admit_reserved_async(&self, id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
        if inner.reserved_async_slots == 0 {
            return Err("no asynchronous execution slot was reserved".into());
        }
        let run = inner.runs.get_mut(id).ok_or("run not found")?;
        if run.status != "accepted" {
            return Err(format!("run '{id}' is not awaiting admission"));
        }
        run.status = "running".into();
        inner.reserved_async_slots -= 1;
        Ok(())
    }

    pub fn execute(
        &self,
        run_id: &str,
        spec: GraphSpec,
        base_url: String,
        default_model: String,
    ) -> Result<Value, String> {
        self.execute_with_store(run_id, spec, base_url, default_model, None)
    }

    pub fn execute_with_store(
        &self,
        run_id: &str,
        spec: GraphSpec,
        base_url: String,
        default_model: String,
        store: Option<PersistentStore>,
    ) -> Result<Value, String> {
        self.execute_with_store_options(
            run_id,
            spec,
            base_url,
            default_model,
            store,
            None,
            BudgetCounters::default(),
        )
    }

    pub fn execute_with_store_options(
        &self,
        run_id: &str,
        spec: GraphSpec,
        base_url: String,
        default_model: String,
        store: Option<PersistentStore>,
        initial_state: Option<Value>,
        initial_counters: BudgetCounters,
    ) -> Result<Value, String> {
        self.update(run_id, |r| r.status = "running".into())?;
        let (
            input,
            cancelled,
            cancellation,
            budgets,
            record_state,
            checkpoint_id,
            checkpoint_digest,
            approval,
            resumed,
        ) = {
            let inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
            let r = inner.runs.get(run_id).ok_or("run not found")?;
            (
                r.input.clone(),
                r.cancelled.clone(),
                r.cancellation.clone(),
                r.budgets.clone(),
                r.state.clone(),
                r.checkpoint_id.clone(),
                r.checkpoint_digest.clone(),
                r.approval.clone(),
                r.resumed,
            )
        };
        let started_at = Instant::now();
        let provider = safe_provider_label(&base_url);
        let configured_model = default_model.clone();
        let events = Arc::new(Mutex::new(Vec::<GraphEvent>::new()));
        // Shared per-run LLM-attempt accounting: one atomic counter and one
        // typed invocation ledger are created here, handed into the compiled
        // nodes, and read back at the terminal boundary so counters and
        // receipts reflect the same observed provider attempts.
        let llm_calls = Arc::new(AtomicU64::new(0));
        let llm_invocations = Arc::new(Mutex::new(Vec::<Value>::new()));
        let graph = compile(
            &spec,
            CompileContext {
                base_url,
                default_model,
                cancelled: cancelled.clone(),
                cancellation: cancellation.clone(),
                events: events.clone(),
                llm_calls: llm_calls.clone(),
                max_llm_calls: budgets.as_ref().and_then(|budget| budget.max_llm_calls),
                llm_invocations: llm_invocations.clone(),
                api_key: self.api_key.clone(),
                executor: ExecutorHandle::none(),
            },
        )?;
        let starting_state = initial_state
            .or_else(|| resumed.then_some(record_state))
            .unwrap_or_else(|| initial_state_for_input(&input));
        let Value::Object(map) = starting_state else {
            return Err("checkpoint state must be a JSON object".into());
        };
        let initial = map.into_iter().collect::<HashMap<_, _>>();
        let state = AgentState::with_data_and_limits(
            initial,
            StateLimits {
                max_keys: 1000,
                max_value_bytes: 256 * 1024,
                max_history_len: 100,
                lock_timeout: std::time::Duration::from_secs(5),
            },
        );
        let snapshot = state.clone();
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let base_iteration_limit = spec.max_iterations.unwrap_or(64);
        let budget_iteration_limit =
            budgets
                .as_ref()
                .and_then(|budget| budget.max_nodes)
                .map(|max_nodes| {
                    budget_iteration_limit(&spec, max_nodes.saturating_sub(initial_counters.nodes))
                });
        let config = GraphConfig::new()
            .with_recursion_limit(
                budget_iteration_limit
                    .unwrap_or(base_iteration_limit)
                    .min(base_iteration_limit),
            )
            .with_max_parallelism(spec.max_parallelism.unwrap_or(8));
        let graph = Arc::new(graph);
        let (handle, engine_cancel) =
            rt.block_on(async { graph.execute_cancellable(&spec.entry, state, config) });
        let timed_out = Arc::new(AtomicBool::new(false));
        let timeout_notify = Arc::new(Notify::new());
        let (timeout_complete, timeout_thread) = budgets
            .as_ref()
            .and_then(|budget| budget.max_wall_clock_ms)
            .map(|limit| {
                let (complete_tx, complete_rx) = std::sync::mpsc::channel::<()>();
                let timed_out = timed_out.clone();
                let timeout_notify = timeout_notify.clone();
                let thread = std::thread::spawn(move || {
                    if matches!(
                        complete_rx.recv_timeout(Duration::from_millis(limit)),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    ) {
                        timed_out.store(true, Ordering::SeqCst);
                        timeout_notify.notify_waiters();
                    }
                });
                (complete_tx, thread)
            })
            .map_or((None, None), |(complete_tx, thread)| {
                (Some(complete_tx), Some(thread))
            });
        let mut handle = handle;
        let result = rt.block_on(async {
            if timeout_thread.is_some() {
                tokio::select! {
                    joined = &mut handle => joined
                        .map_err(|error| format!("execution task failed: {error}"))?
                        .map_err(|error| error.to_string()),
                    _ = timeout_notify.notified() => {
                        engine_cancel.store(true, Ordering::SeqCst);
                        // Wake a cancellable provider/node without marking the
                        // user cancellation flag. Terminal cancellation must
                        // retain precedence only for an actual cancel request.
                        cancellation.notify_waiters();
                        handle.abort();
                        let _ = handle.await;
                        Err("BUDGET_EXHAUSTED".to_owned())
                    }
                }
            } else {
                handle
                    .await
                    .map_err(|error| format!("execution task failed: {error}"))?
                    .map_err(|error| error.to_string())
            }
        });
        if let Some(complete) = timeout_complete {
            let _ = complete.send(());
        }
        if let Some(thread) = timeout_thread {
            let _ = thread.join();
        }
        // derive a receipt from the summary
        let core_receipt = serde_json::json!({
            "api": "execute_with_summary",
            "note": "receipt not yet fully typed; upgrade to execute_with_config + receipt"
        });
        let exported = rt.block_on(snapshot.export());
        let state_value = serde_json::to_value(exported).map_err(|e| e.to_string())?;
        ensure_size(&state_value, MAX_STATE_BYTES, "total state")?;
        let final_state = spec
            .output_key
            .as_deref()
            .and_then(|key| state_value.get(key).cloned())
            .or_else(|| state_value.get("__input__").cloned())
            .unwrap_or(Value::Null);
        ensure_size(&final_state, MAX_OUTPUT_BYTES, "execution output")?;
        let graph_events = events
            .lock()
            .map_err(|_| "event registry poisoned")?
            .clone();
        let observed_llm_calls = llm_calls.load(Ordering::SeqCst);
        let observed_invocations = llm_invocations
            .lock()
            .map_err(|_| "llm invocation registry poisoned")?
            .clone();
        let mut steps: Vec<Value> = Vec::new();
        for event in &graph_events {
            if let GraphEvent::StateUpdate {
                node_id, updates, ..
            } = event
            {
                let output = updates
                    .get("__input__")
                    .or_else(|| updates.get("__route__"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::to_value(updates).unwrap_or(Value::Null));
                steps.push(serde_json::json!({
                    "node_id": node_id,
                    "output": output,
                }));
            }
        }
        let budget_counters = BudgetCounters {
            nodes: initial_counters.nodes.saturating_add(
                graph_events
                    .iter()
                    .filter(|event| matches!(event, GraphEvent::NodeStart { .. }))
                    .count() as u64,
            ),
            // Observed from the shared pre-invocation reservation counter, so
            // the terminal record matches actual provider attempts (including
            // failed or timed-out attempts).
            llm_calls: initial_counters
                .llm_calls
                .saturating_add(observed_llm_calls),
            wall_clock_ms: initial_counters
                .wall_clock_ms
                .saturating_add(started_at.elapsed().as_millis().min(u64::MAX as u128) as u64),
        };
        let wall_exhausted = budgets.as_ref().is_some_and(|budget| {
            budget
                .max_wall_clock_ms
                .is_some_and(|limit| budget_counters.wall_clock_ms >= limit)
                || timed_out.load(Ordering::SeqCst)
        });
        let node_exhausted = budgets.as_ref().and_then(|budget| {
            let limit = budget.max_nodes?;
            let iteration_error = result
                .as_ref()
                .err()
                .is_some_and(|error| error.contains("iteration"));
            let budget_limited_iterations =
                budget_iteration_limit.is_some_and(|limit| limit <= base_iteration_limit);
            (budget_counters.nodes > limit
                || (budget_counters.nodes >= limit && iteration_error && budget_limited_iterations))
                .then_some("max_nodes".into())
        });
        let llm_exhausted = budgets.as_ref().is_some_and(|budget| {
            budget.max_llm_calls.is_some()
                && result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.contains("BUDGET_EXHAUSTED"))
        });
        let budget_exhausted = if wall_exhausted {
            Some("max_wall_clock_ms".to_owned())
        } else if let Some(node) = node_exhausted {
            Some(node)
        } else if llm_exhausted {
            Some("max_llm_calls".to_owned())
        } else {
            None
        };
        let mut dependency_envelopes = Value::Array(Vec::new());
        let mut dependency_envelopes_complete = false;
        let mut evidence_error = None;
        if result.is_ok() && spec.nodes.iter().any(|node| node.evidence_required) {
            match store.as_ref() {
                Some(store) => {
                    let mut collected = Vec::new();
                    for node in spec.nodes.iter().filter(|node| node.evidence_required) {
                        let output_key = node
                            .config
                            .get("output_key")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let Some(evidence) = state_value.get(output_key) else {
                            evidence_error = Some("WITNESS_EVIDENCE_MISSING".to_owned());
                            break;
                        };
                        match validate_witness_dependencies(evidence, store) {
                            Ok(Value::Array(dependencies)) => collected.extend(dependencies),
                            Ok(_) => {
                                evidence_error = Some("WITNESS_EVIDENCE_INVALID".to_owned());
                                break;
                            }
                            Err(error) => {
                                evidence_error = Some(error.code);
                                break;
                            }
                        }
                    }
                    if evidence_error.is_none() {
                        let mut unique = std::collections::BTreeMap::new();
                        for dependency in collected {
                            if let Some(id) = dependency.get("witness_id").and_then(Value::as_str) {
                                unique.insert(id.to_owned(), dependency);
                            }
                        }
                        dependency_envelopes = Value::Array(unique.into_values().collect());
                        dependency_envelopes_complete = true;
                    }
                }
                None => evidence_error = Some("WITNESS_STORE_REQUIRED".to_owned()),
            }
        }
        let error = if budget_exhausted.is_some() {
            Some("BUDGET_EXHAUSTED".to_owned())
        } else if evidence_error.is_some() {
            evidence_error
        } else {
            result.err().map(|e| e.to_string())
        };
        let trace = self.get(run_id).ok_or("run not found")?.trace;
        let graph_version = self.get(run_id).ok_or("run not found")?.graph_version;
        let models: Vec<Value> = spec.nodes.iter().filter(|node| matches!(node.node_type, crate::spec::NodeType::Llm)).map(|node| serde_json::json!({
            "node_id":node.id,"model_alias":node.model.as_deref().unwrap_or("server_default"),"prompt_digest":digest(&Value::String(node.prompt.clone().unwrap_or_else(||"{input}".into())))
        })).collect();
        let model_labels: Vec<Value> = models
            .iter()
            .filter_map(|m| m.get("model_alias").cloned())
            .collect();
        let terminal_output_key = spec
            .output_key
            .clone()
            .unwrap_or_else(|| "__input__".to_owned());
        let terminal_output = serde_json::json!({
            "state_key": terminal_output_key,
            "provenance": if spec.output_key.is_some() { "declared_output_key" } else { "legacy_input_fallback" },
            "value_digest": digest(&final_state),
        });
        let receipt = serde_json::json!({"schema":"agent-graph-mcp-receipt-v2","run_id":run_id,"trace":trace,"graph_version":graph_version,
            "input_digest":digest(&input),"output_digest":digest(&state_value),"step_count":steps.len(),"models":models,
            "provider":provider,"default_model":configured_model,"model_labels":model_labels,
            "core":core_receipt,"terminal_output":terminal_output,"llm_invocations":observed_invocations,
            "dependency_envelopes":dependency_envelopes,"dependency_envelopes_complete":dependency_envelopes_complete,"replay_capability":if resumed { "deterministic_local_resume" } else { "integrity_only" },
            "resume_supported":resumed,
            "checkpoint":checkpoint_id.as_ref().zip(checkpoint_digest.as_ref()).map(|(id, digest)| serde_json::json!({"checkpoint_id":id,"checkpoint_digest":digest})),
            "approval":approval,
            "evidence_authority":if dependency_envelopes_complete { "local_capture_receipt_only; source_authority_not_verified" } else { "structural_unverified" },"persistence_status":"pending",
            "budgets":budgets.as_ref().map(RunBudgets::requested_value).unwrap_or(Value::Null),
            "budget_counters":budget_counters,"budget_exhausted":budget_exhausted});
        let artifact = bundle(run_id, &graph_version, &input, &state_value, &receipt);
        self.update(run_id, |r| {
            let cancellation_observed = r.cancelled.load(Ordering::SeqCst);
            let terminal_error = (!cancellation_observed).then(|| error.clone()).flatten();
            let terminal = terminal_outcome(cancellation_observed, terminal_error.as_deref());
            r.status = terminal.status.into();
            r.success = Some(terminal.success);
            r.state = state_value.clone();
            r.final_state = final_state.clone();
            r.steps = steps.clone();
            r.error = terminal_error;
            r.budget_counters = budget_counters.clone();
            r.budget_exhausted = budget_exhausted.clone();
            r.receipt = receipt.clone();
            r.bundle = artifact.clone();
            r.persistence_status = "pending".into();
            for event in graph_events {
                push_event(r, serde_json::to_value(event).unwrap_or(Value::Null));
            }
        })?;
        Ok(self.get(run_id).expect("updated run").public())
    }

    pub fn start(&self, run_id: String, spec: GraphSpec, base_url: String, model: String) {
        self.start_with_completion(run_id, spec, base_url, model, |_| {});
    }

    pub fn start_with_completion<F>(
        &self,
        run_id: String,
        spec: GraphSpec,
        base_url: String,
        model: String,
        on_completion: F,
    ) where
        F: FnOnce(RunRecord) + Send + 'static,
    {
        self.start_with_completion_with_store(run_id, spec, base_url, model, None, on_completion);
    }

    pub fn start_with_completion_with_store<F>(
        &self,
        run_id: String,
        spec: GraphSpec,
        base_url: String,
        model: String,
        store: Option<PersistentStore>,
        on_completion: F,
    ) where
        F: FnOnce(RunRecord) + Send + 'static,
    {
        let manager = self.clone();
        std::thread::spawn(move || {
            if let Err(error) = manager.execute_with_store(&run_id, spec, base_url, model, store) {
                let _ = manager.update(&run_id, |r| {
                    r.status = "failed".into();
                    r.success = Some(false);
                    r.error = Some(error.clone());
                });
            }
            if let Some(record) = manager.get(&run_id) {
                on_completion(record);
            }
        });
    }

    pub fn start_resumed_with_completion<F>(
        &self,
        run_id: String,
        spec: GraphSpec,
        base_url: String,
        model: String,
        store: Option<PersistentStore>,
        on_completion: F,
    ) where
        F: FnOnce(RunRecord) + Send + 'static,
    {
        let manager = self.clone();
        std::thread::spawn(move || {
            if let Err(error) = manager.execute_with_store_options(
                &run_id,
                spec,
                base_url,
                model,
                store,
                None,
                BudgetCounters::default(),
            ) {
                let _ = manager.update(&run_id, |r| {
                    r.status = "failed".into();
                    r.success = Some(false);
                    r.error = Some(error.clone());
                });
            }
            if let Some(record) = manager.get(&run_id) {
                on_completion(record);
            }
        });
    }
    pub fn cancel(&self, id: &str) -> Result<Value, String> {
        let cancellation = {
            let mut inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
            let r = inner.runs.get_mut(id).ok_or("run not found")?;
            if is_terminal(&r.status) {
                return Err("RUN_NOT_CANCELLABLE".into());
            }
            r.cancelled.store(true, Ordering::SeqCst);
            r.cancellation.clone()
        };
        cancellation.notify_waiters();
        Ok(serde_json::json!({
            "run_id":id,
            "status":"cancellation_requested",
            "cancellation_effect":"best_effort_drop_provider_future",
            "provider_request_may_still_be_in_flight":true,
            "effective_at":"provider_completion_or_cancellation_observation"
        }))
    }
    pub fn get(&self, id: &str) -> Option<RunRecord> {
        self.inner.lock().ok()?.runs.get(id).cloned()
    }
    pub fn mark_persistence(&self, id: &str, status: &str, error: Option<String>) {
        let _ = self.update(id, |run| {
            run.persistence_status = status.into();
            run.persistence_error = error.clone();
            if let Some(object) = run.receipt.as_object_mut() {
                object.insert("persistence_status".into(), Value::String(status.into()));
                if let Some(error) = error {
                    object.insert("persistence_error".into(), Value::String(error));
                }
            }
            run.bundle = bundle(
                &run.run_id,
                &run.graph_version,
                &run.input,
                &run.state,
                &run.receipt,
            );
        });
    }
    pub(crate) fn remove(&self, id: &str) -> Option<RunRecord> {
        let mut inner = self.inner.lock().ok()?;
        if inner
            .runs
            .get(id)
            .is_some_and(|run| run.status == "running")
        {
            return None;
        }
        inner.order.retain(|run_id| run_id != id);
        inner.runs.remove(id)
    }
    pub fn list(&self) -> Vec<Value> {
        self.inner
            .lock()
            .map(|i| {
                i.order
                    .iter()
                    .filter_map(|id| i.runs.get(id).map(RunRecord::public))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn events(
        &self,
        store: Option<&PersistentStore>,
        id: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<Value, String> {
        let Some(r) = self.get(id) else {
            if let Some(store) = store {
                if let Some(events) = store.load_events(id, cursor, limit)? {
                    return Ok(events);
                }
            }
            return Err("run not found".into());
        };
        let first = r
            .events
            .front()
            .and_then(|v| v.get("cursor"))
            .and_then(Value::as_u64)
            .unwrap_or(r.next_cursor);
        let events: Vec<_> = r
            .events
            .iter()
            .filter(|v| v["cursor"].as_u64().unwrap_or(0) >= cursor)
            .take(limit.min(200))
            .cloned()
            .collect();
        Ok(
            serde_json::json!({"run_id":id,"events":events,"next_cursor":r.next_cursor,"gap":cursor<first,"truncated":r.dropped_events>0,"dropped":r.dropped_events}),
        )
    }
    pub fn set_state_value(&self, id: &str, key: &str, value: Value) -> Result<(), String> {
        self.update(id, |run| {
            if let Some(state) = run.state.as_object_mut() {
                state.insert(key.to_owned(), value.clone());
            } else {
                let mut state = serde_json::Map::new();
                state.insert(key.to_owned(), value);
                run.state = Value::Object(state);
            }
        })
    }
    fn update(&self, id: &str, f: impl FnOnce(&mut RunRecord)) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "run registry poisoned")?;
        let r = inner.runs.get_mut(id).ok_or("run not found")?;
        f(r);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalOutcome {
    status: &'static str,
    success: bool,
}

fn terminal_outcome(cancelled: bool, error: Option<&str>) -> TerminalOutcome {
    if cancelled {
        TerminalOutcome {
            status: "cancelled",
            success: false,
        }
    } else if error.is_some() {
        TerminalOutcome {
            status: "failed",
            success: false,
        }
    } else {
        TerminalOutcome {
            status: "completed",
            success: true,
        }
    }
}

/// Return the greatest number of engine supersteps whose conservative frontier
/// size stays within a node budget. The engine checks recursion before entering
/// a superstep, so this prevents a later parallel frontier from starting after
/// the budget is exhausted. Conditional edges are treated as possible edges;
/// this can stop early, but never allows an over-budget frontier.
fn budget_iteration_limit(spec: &GraphSpec, max_nodes: u64) -> usize {
    let graph_limit = spec.max_iterations.unwrap_or(64);
    let mut frontier = vec![spec.entry.clone()];
    let mut observed = 0u64;
    let mut allowed = 0usize;

    for _ in 0..graph_limit {
        frontier.retain(|node| node != "END");
        if frontier.is_empty() {
            break;
        }
        let width = frontier.len() as u64;
        if observed.saturating_add(width) > max_nodes {
            break;
        }
        observed = observed.saturating_add(width);
        allowed += 1;

        let current: HashSet<&str> = frontier.iter().map(String::as_str).collect();
        let mut next = Vec::new();
        for edge in &spec.edges {
            if current.contains(edge.from.as_str()) && edge.to != "END" {
                next.push(edge.to.clone());
            }
        }
        let mut seen = HashSet::new();
        next.retain(|node| seen.insert(node.clone()));
        frontier = next;
    }

    allowed.max(1)
}

fn push_event(run: &mut RunRecord, event: Value) {
    if run.events.len() == 512 {
        run.events.pop_front();
        run.dropped_events += 1;
    }
    let cursor = run.next_cursor;
    run.next_cursor += 1;
    run.events
        .push_back(serde_json::json!({"cursor":cursor,"event":redact(&event)}));
}

fn safe_provider_label(url: &str) -> String {
    let without_fragment = url.split(['?', '#']).next().unwrap_or(url);
    if let Some((scheme, rest)) = without_fragment.split_once("://") {
        let authority_and_path = rest.rsplit_once('@').map(|(_, safe)| safe).unwrap_or(rest);
        format!("{scheme}://{authority_and_path}")
    } else {
        "server-configured".into()
    }
}

pub fn initial_state_for_input(input: &Value) -> Value {
    let mut initial = serde_json::Map::new();
    initial.insert("__input__".into(), input.clone());
    if let Value::Object(map) = input {
        initial.extend(map.clone());
    }
    Value::Object(initial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_admission_is_bounded_atomically() {
        let manager = RunManager::default();
        for index in 0..MAX_ACTIVE_RUNS {
            let id = manager
                .allocate("graph", "version", serde_json::json!({"index": index}))
                .expect("allocate admitted run");
            manager.admit_async(&id).expect("admit within cap");
        }
        let overflow = manager
            .allocate("graph", "version", Value::Null)
            .expect("registry still has room");
        assert!(manager.admit_async(&overflow).is_err());
        manager.remove(&overflow);
        assert!(manager.get(&overflow).is_none());
    }

    #[test]
    fn retention_never_evicts_live_runs() {
        let manager = RunManager::default();
        let mut ids = Vec::new();
        for _ in 0..MAX_RUNS {
            ids.push(
                manager
                    .allocate("graph", "version", Value::Null)
                    .expect("allocate retained run"),
            );
        }
        assert!(manager.allocate("graph", "version", Value::Null).is_err());
        manager
            .update(&ids[0], |record| record.status = "completed".into())
            .expect("mark terminal");
        let replacement = manager
            .allocate("graph", "version", Value::Null)
            .expect("terminal record may be evicted");
        assert!(manager.get(&ids[0]).is_none());
        assert!(manager.get(&replacement).is_some());
    }

    #[test]
    fn cancelled_terminal_run_is_never_successful() {
        let outcome = terminal_outcome(true, None);
        assert_eq!(outcome.status, "cancelled");
        assert!(!outcome.success);

        let completed = terminal_outcome(false, None);
        assert_eq!(completed.status, "completed");
        assert!(completed.success);

        let failed = terminal_outcome(false, Some("provider error"));
        assert_eq!(failed.status, "failed");
        assert!(!failed.success);
    }

    #[test]
    fn cancellation_is_only_a_request_until_a_node_boundary() {
        let manager = RunManager::default();
        let id = manager
            .allocate("graph", "version", Value::Null)
            .expect("allocate run");
        manager.admit_async(&id).expect("admit run");
        let response = manager.cancel(&id).expect("request cancellation");
        assert_eq!(response["status"], "cancellation_requested");
        assert_eq!(
            response["cancellation_effect"],
            "best_effort_drop_provider_future"
        );
        assert_eq!(manager.get(&id).expect("run").status, "running");
    }

    #[test]
    fn persistence_failure_is_publicly_volatile_and_recorded() {
        let manager = RunManager::default();
        let id = manager
            .allocate("graph", "version", Value::Null)
            .expect("allocate run");
        manager.mark_persistence(
            &id,
            "volatile_persistence_failed",
            Some("database is unavailable".into()),
        );
        let public = manager.get(&id).expect("run").public();
        assert_eq!(public["storage_class"], "volatile");
        assert_eq!(public["persistence_error"], "database is unavailable");
    }
}
