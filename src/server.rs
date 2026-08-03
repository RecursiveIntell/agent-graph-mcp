//! MCP server handler using rmcp's #[tool_router] macro.
//!
//! Each #[tool] method becomes an MCP tool that Hermes can discover and call.
//! The rmcp macro auto-generates JSON Schema from the parameter structs in tools.rs.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler,
};
use serde_json::Value;

use std::path::PathBuf;

use crate::evidence::{digest, validate_witness_capture, WitnessCapture, WitnessError};
use crate::run_manager::{initial_state_for_input, RunBudgets, RunManager};
use crate::spec::{
    ensure_size, parse_and_validate, validate_max_graphs, GraphSpec, DEFAULT_MAX_GRAPHS,
    MAX_INPUT_BYTES,
};
use crate::store::{
    ApprovalError, ApprovalRecord, CheckpointError, CheckpointRecord, GraphDeleteResult,
    PersistentStore,
};
use crate::templates;
use crate::tools::*;

fn internal_error(message: impl Into<std::borrow::Cow<'static, str>>) -> ErrorData {
    ErrorData::internal_error(message, None)
}

fn invalid_params(message: impl Into<std::borrow::Cow<'static, str>>) -> ErrorData {
    ErrorData::invalid_params(message, None)
}

fn structured_output(value: Value) -> Json<StructuredOutput> {
    Json(StructuredOutput {
        ok: true,
        status: None,
        data: Some(value),
        error: None,
        error_code: None,
        graph_id: None,
        graph_version: None,
        run_id: None,
    })
}

fn error_output(message: impl Into<String>, code: impl Into<String>) -> Json<StructuredOutput> {
    Json(StructuredOutput {
        ok: false,
        status: None,
        data: None,
        error: Some(message.into()),
        error_code: Some(code.into()),
        graph_id: None,
        graph_version: None,
        run_id: None,
    })
}

fn structured_from_value(value: Value) -> Result<Json<StructuredOutput>, ErrorData> {
    serde_json::from_value::<StructuredOutput>(value)
        .map(Json)
        .map_err(|e| internal_error(format!("cached idempotency decode: {e}")))
}

fn canonical_request_value(value: &Value) -> Value {
    match value {
        Value::String(raw) => serde_json::from_str(raw).unwrap_or_else(|_| value.clone()),
        _ => value.clone(),
    }
}

fn check_idempotency(
    store: Option<&PersistentStore>,
    key: Option<&str>,
    request_digest: &str,
) -> Result<Option<Json<StructuredOutput>>, ErrorData> {
    let Some((store, key)) = store.zip(key) else {
        return Ok(None);
    };
    let Some((stored_digest, cached)) = store.check_idempotency(key).map_err(internal_error)?
    else {
        return Ok(None);
    };
    if stored_digest.as_deref() == Some(request_digest) {
        return structured_from_value(cached).map(Some);
    }
    Ok(Some(error_output(
        "idempotency key is already bound to different request material",
        "IDEMPOTENCY_CONFLICT",
    )))
}

fn persist_idempotency(
    store: &PersistentStore,
    key: &str,
    request_digest: &str,
    output: &Json<StructuredOutput>,
) -> Result<Option<Json<StructuredOutput>>, ErrorData> {
    let result_json =
        serde_json::to_string(&output.0).map_err(|e| internal_error(e.to_string()))?;
    if store
        .save_idempotency(key, request_digest, &result_json)
        .map_err(internal_error)?
    {
        return Ok(None);
    }
    // Another request won the insert. Return its exact cached result so a
    // concurrent same-key caller cannot observe a result that was not stored.
    check_idempotency(Some(store), Some(key), request_digest)
}

fn output_with_meta(
    data: Value,
    graph_id: Option<&str>,
    graph_version: Option<&str>,
    run_id: Option<&str>,
) -> Json<StructuredOutput> {
    Json(StructuredOutput {
        ok: true,
        status: None,
        data: Some(data),
        error: None,
        error_code: None,
        graph_id: graph_id.map(String::from),
        graph_version: graph_version.map(String::from),
        run_id: run_id.map(String::from),
    })
}

fn checkpoint_error_output(error: CheckpointError) -> Json<StructuredOutput> {
    error_output(error.message(), error.code())
}

fn approval_error_output(error: ApprovalError) -> Json<StructuredOutput> {
    error_output(error.message(), error.code())
}

fn approval_value(record: &ApprovalRecord) -> Value {
    serde_json::json!({
        "approval_id": record.approval_id,
        "checkpoint_id": record.checkpoint_id,
        "run_id": record.run_id,
        "graph_id": record.graph_id,
        "graph_version": record.graph_version,
        "checkpoint_digest": record.checkpoint_digest,
        "audience": record.audience,
        "prompt_digest": record.prompt_digest,
        "allowed_decisions": record.allowed_decisions,
        "approval_digest": record.approval_digest,
        "status": record.status,
        "decision": record.decision,
        "decided_by": record.decided_by,
        "decided_at": record.decided_at,
        "expires_at": record.expires_at,
        "created_at": record.created_at,
    })
}

fn checkpoint_value(record: &CheckpointRecord) -> Value {
    serde_json::json!({
        "checkpoint_id": record.checkpoint_id,
        "run_id": record.run_id,
        "graph_id": record.graph_id,
        "graph_version": record.graph_version,
        "next_node_cursor": record.next_node_cursor,
        "state": record.state,
        "state_digest": record.state_digest,
        "budgets": record.budgets,
        "budget_counters": record.budget_counters,
        "dependency_summary": record.dependency_summary,
        "dependency_digest": record.dependency_digest,
        "terminal_cursor": record.terminal_cursor,
        "event_cursor": record.event_cursor,
        "checkpoint_digest": record.checkpoint_digest,
        "created_at": record.created_at,
        "consumed_at": record.consumed_at,
        "status": if record.consumed_at.is_some() { "consumed" } else { "available" },
        "resume_capability": "deterministic_local_resume",
    })
}

#[derive(Clone)]
struct RegisteredGraph {
    spec: GraphSpec,
    normalized: Value,
    version: String,
    warnings: Vec<String>,
}

pub struct AgentGraphServer {
    tool_router: ToolRouter<Self>,
    base_url: String,
    default_model: String,
    /// Provider API key for http(s) llm-pipeline calls. Attached as a Bearer
    /// header only for the http(s) path; the codex-app-server:// path carries no
    /// auth. Never serialized into status, receipts, or logs.
    api_key: Option<String>,
    graphs: Mutex<HashMap<String, RegisteredGraph>>,
    runs: Mutex<RunManager>,
    store: Option<PersistentStore>,
    max_graphs: usize,
}

impl AgentGraphServer {
    fn graph_requires_witness_store(spec: &GraphSpec) -> bool {
        spec.nodes.iter().any(|node| node.evidence_required)
    }

    fn witness_error_output(error: WitnessError) -> Json<StructuredOutput> {
        error_output(error.message, error.code)
    }

    pub fn new(
        base_url: String,
        default_model: String,
        data_dir: Option<PathBuf>,
        integrity_key_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        Self::new_with_max_graphs(
            base_url,
            default_model,
            data_dir,
            integrity_key_path,
            DEFAULT_MAX_GRAPHS,
        )
    }

    pub fn new_with_max_graphs(
        base_url: String,
        default_model: String,
        data_dir: Option<PathBuf>,
        integrity_key_path: Option<PathBuf>,
        max_graphs: usize,
    ) -> Result<Self, String> {
        Self::new_with_max_graphs_and_key(
            base_url,
            default_model,
            data_dir,
            integrity_key_path,
            max_graphs,
            None,
        )
    }

    pub fn new_with_max_graphs_and_key(
        base_url: String,
        default_model: String,
        data_dir: Option<PathBuf>,
        integrity_key_path: Option<PathBuf>,
        max_graphs: usize,
        api_key: Option<String>,
    ) -> Result<Self, String> {
        let max_graphs = validate_max_graphs(max_graphs)?;
        let store = match data_dir {
            Some(ref dir) => Some(PersistentStore::open_with_integrity_key(
                dir,
                integrity_key_path.as_deref(),
            )?),
            None => None,
        };
        if let Some(ref store) = store {
            store.recover_incomplete_executions()?;
        }

        let runs = RunManager::default().with_api_key(api_key.clone());
        let server = Self {
            base_url,
            default_model,
            api_key,
            graphs: Mutex::new(HashMap::new()),
            runs: Mutex::new(runs),
            store,
            max_graphs,
            tool_router: Self::tool_router(),
        };

        // Restore persisted graphs on startup
        if let Some(ref store) = server.store {
            if let Ok(graphs) = store.list_graphs() {
                for (name, hash, _created) in graphs {
                    if let Ok(Some((spec_json, _))) = store.load_graph(&name) {
                        if let Ok(spec) = serde_json::from_str::<GraphSpec>(&spec_json) {
                            let normalized = serde_json::to_value(&spec).unwrap_or_default();
                            server.graphs.lock().unwrap().insert(
                                name,
                                RegisteredGraph {
                                    spec,
                                    normalized,
                                    version: hash,
                                    warnings: Vec::new(),
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(server)
    }

    fn safe_provider_label(&self) -> String {
        let url = &self.base_url;
        let without_fragment = url.split(['?', '#']).next().unwrap_or(url);
        if let Some((scheme, rest)) = without_fragment.split_once("://") {
            let authority_and_path = rest.rsplit_once('@').map(|(_, safe)| safe).unwrap_or(rest);
            format!("{scheme}://{authority_and_path}")
        } else {
            "server-configured".into()
        }
    }

    fn persist_terminal(
        store: Option<PersistentStore>,
        record: crate::run_manager::RunRecord,
    ) -> Result<(), String> {
        let Some(store) = store else {
            return Ok(());
        };
        let final_state = serde_json::to_string(&record.final_state)
            .map_err(|e| format!("serialize terminal state error: {e}"))?;
        // Persist one bounded terminal projection atomically. This is not replayable
        // execution history and does not make the run resumable.
        let events = record
            .events
            .iter()
            .map(|entry| {
                let seq = entry.get("cursor").and_then(Value::as_u64).unwrap_or(0);
                let event = entry.get("event").cloned().unwrap_or_else(|| {
                    serde_json::json!({"receipt": "terminal event persisted with reduced fidelity"})
                });
                let event_type = event
                    .as_object()
                    .and_then(|object| object.keys().next().cloned())
                    .unwrap_or_else(|| "run_event".into());
                Ok((seq, event_type, event.to_string()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut durable_receipt = record.receipt.clone();
        if let Some(object) = durable_receipt.as_object_mut() {
            object.insert(
                "persistence_status".into(),
                Value::String("durable_terminal".into()),
            );
        }
        let receipt = serde_json::to_string(&durable_receipt)
            .map_err(|e| format!("serialize terminal receipt error: {e}"))?;
        let durable_bundle = crate::evidence::bundle(
            &record.run_id,
            &record.graph_version,
            &record.input,
            &record.state,
            &durable_receipt,
        );
        let bundle = serde_json::to_string(&durable_bundle)
            .map_err(|e| format!("serialize terminal bundle error: {e}"))?;
        store.persist_terminal_projection(
            &record.run_id,
            &record.status,
            &final_state,
            record.steps.len(),
            &events,
            &receipt,
            &bundle,
        )?;
        Ok(())
    }

    fn persist_terminal_and_mark(
        runs: crate::run_manager::RunManager,
        store: Option<PersistentStore>,
        record: crate::run_manager::RunRecord,
    ) {
        if store.is_none() {
            runs.mark_persistence(&record.run_id, "volatile_no_store", None);
            return;
        }
        match Self::persist_terminal(store, record.clone()) {
            Ok(()) => runs.mark_persistence(&record.run_id, "durable_terminal", None),
            Err(error) => {
                tracing::error!(%error, "terminal run persistence failed; run remains volatile");
                runs.mark_persistence(&record.run_id, "volatile_persistence_failed", Some(error));
            }
        }
    }

    fn stored_run(&self, run_id: &str) -> Result<Option<Value>, ErrorData> {
        let Some(store) = &self.store else {
            return Ok(None);
        };
        let Some(mut record) = store.load_execution(run_id).map_err(internal_error)? else {
            return Ok(None);
        };
        if let Some(receipt) = store
            .load_terminal_receipt(run_id)
            .map_err(internal_error)?
            .and_then(|value| value.get("receipt").cloned())
        {
            if let Some(object) = record.as_object_mut() {
                for key in ["budgets", "budget_counters", "budget_exhausted"] {
                    if let Some(value) = receipt.get(key) {
                        object.insert(key.into(), value.clone());
                    }
                }
                object.insert("receipt".into(), receipt);
            }
        }
        Ok(Some(record))
    }

    fn resolve_graph(
        &self,
        graph_id: &str,
        requested_version: Option<&str>,
    ) -> Result<RegisteredGraph, ErrorData> {
        let current = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?
            .get(graph_id)
            .cloned()
            .ok_or_else(|| invalid_params(format!("graph '{graph_id}' not found")))?;
        let Some(requested_version) = requested_version else {
            return Ok(current);
        };
        if requested_version == current.version {
            return Ok(current);
        }
        let store = self.store.as_ref().ok_or_else(|| {
            invalid_params("historical graph versions require SQLite persistence")
        })?;
        let serialized = store
            .load_graph_version(graph_id, requested_version)
            .map_err(internal_error)?
            .ok_or_else(|| invalid_params("requested graph version was not found"))?;
        let normalized: Value = serde_json::from_str(&serialized)
            .map_err(|e| internal_error(format!("stored graph version JSON error: {e}")))?;
        let spec = parse_and_validate(&normalized)
            .map_err(|e| internal_error(format!("stored graph version validation error: {e}")))?;
        let canonical = serde_json::to_value(&spec).map_err(|e| internal_error(e.to_string()))?;
        let actual_version = digest(&canonical);
        if actual_version != requested_version {
            return Err(internal_error(
                "stored graph version digest does not match its normalized specification",
            ));
        }
        Ok(RegisteredGraph {
            warnings: spec.warnings(),
            spec,
            normalized: canonical,
            version: actual_version,
        })
    }

    fn mermaid(spec: &GraphSpec) -> String {
        let mut s = String::from("graph TD\n");
        for edge in &spec.edges {
            s.push_str(&format!("  {} --> {}\n", edge.from, edge.to));
        }
        s
    }

    fn delete_registered_graph(&self, graph_id: &str) -> Result<Json<StructuredOutput>, ErrorData> {
        let exists = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?
            .contains_key(graph_id);
        if !exists {
            return Ok(error_output(
                format!("graph '{graph_id}' not found"),
                "GRAPH_NOT_FOUND",
            ));
        }

        if let Some(store) = &self.store {
            match store.delete_graph(graph_id).map_err(internal_error)? {
                GraphDeleteResult::Deleted => {}
                GraphDeleteResult::Referenced => {
                    return Ok(error_output(
                        format!("graph '{graph_id}' is referenced by a durable execution"),
                        "GRAPH_REFERENCED",
                    ));
                }
                GraphDeleteResult::ReferencedBySubgraph => {
                    return Ok(error_output(
                        format!(
                            "graph '{graph_id}' is referenced by another graph's subgraph node"
                        ),
                        "GRAPH_SUBGRAPH_REFERENCED",
                    ));
                }
                GraphDeleteResult::RetentionApprovalRequired => {
                    return Ok(error_output(
                        format!("graph '{graph_id}' requires delete_candidate then delete_approved retention states"),
                        "RETENTION_APPROVAL_REQUIRED",
                    ));
                }
                GraphDeleteResult::NotFound => {
                    return Ok(error_output(
                        format!(
                            "graph '{graph_id}' is present in memory but missing from durable storage"
                        ),
                        "GRAPH_PERSISTENCE_MISMATCH",
                    ));
                }
            }
        }

        self.graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?
            .remove(graph_id);
        Ok(output_with_meta(
            serde_json::json!({"status": "deleted"}),
            Some(graph_id),
            None,
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_manager::RunManager;
    use crate::store::PersistentStore;

    fn configure_test_integrity_key() {
        let path = std::env::temp_dir().join("agent-graph-mcp-unit-integrity.key");
        std::fs::write(&path, [0x5au8; 32]).expect("test integrity key");
        std::env::set_var("AGENT_GRAPH_INTEGRITY_KEY_PATH", path);
    }

    #[test]
    fn terminal_projection_failure_rolls_back_sqlite_and_marks_run_volatile() {
        configure_test_integrity_key();
        let temp = tempfile::tempdir().expect("temp graph database");
        let store = PersistentStore::open(temp.path()).expect("store");
        let spec: GraphSpec = serde_json::from_value(serde_json::json!({
            "name":"fault-injection",
            "entry":"x",
            "nodes":[{"id":"x","type":"passthrough"}],
            "edges":[{"from":"x","to":"END"}]
        }))
        .expect("graph spec");
        let spec_json = serde_json::to_string(&spec).expect("spec JSON");
        store
            .save_graph("fault-injection", &spec_json, "version", false)
            .expect("graph");

        let runs = RunManager::default();
        let run_id = runs
            .allocate("fault-injection", "version", serde_json::json!({"x":1}))
            .expect("run");
        store
            .save_execution(
                &run_id,
                "fault-injection",
                "version",
                "running",
                "{\"x\":1}",
            )
            .expect("execution");
        runs.execute(
            &run_id,
            spec,
            "http://localhost".into(),
            "test-model".into(),
        )
        .expect("execution completes");

        store.fail_terminal_projection_after_events();
        AgentGraphServer::persist_terminal_and_mark(
            runs.clone(),
            Some(store.clone()),
            runs.get(&run_id).expect("terminal record"),
        );

        let public = runs.get(&run_id).expect("volatile record").public();
        assert_eq!(public["persistence_status"], "volatile_persistence_failed");
        assert_eq!(public["storage_class"], "volatile");

        let reopened = PersistentStore::open(temp.path()).expect("fresh store");
        assert_eq!(
            reopened.load_execution(&run_id).unwrap().unwrap()["status"],
            "running"
        );
        assert!(reopened.load_events(&run_id, 0, 100).unwrap().is_none());
        assert!(reopened.load_terminal_receipt(&run_id).unwrap().is_none());
    }

    #[test]
    fn capacity_is_reserved_before_direct_or_approved_checkpoint_consumption() {
        configure_test_integrity_key();
        let temp = tempfile::tempdir().expect("checkpoint database");
        let server = AgentGraphServer::new(
            "http://localhost".into(),
            "test-model".into(),
            Some(temp.path().to_owned()),
            None,
        )
        .expect("server");
        server
            .graph_create(Parameters(GraphCreateParams {
                spec: Some(serde_json::json!({
                    "name":"capacity-resume", "entry":"first",
                    "nodes":[
                        {"id":"first","type":"passthrough"},
                        {"id":"second","type":"state_transform","config":{"operations":[{"op":"set","path":"done","value":true}]}}
                    ],
                    "edges":[{"from":"first","to":"second"},{"from":"second","to":"END"}]
                })),
                action: None,
                graph_id: None,
                idempotency_key: None,
                template: None,
                overwrite: None,
            }))
            .expect("create graph");
        let checkpoint = |server: &AgentGraphServer| {
            server
                .graph_run_start(Parameters(RunStartParams {
                    graph_id: "capacity-resume".into(),
                    input: None,
                    graph_version: None,
                    thread_id: None,
                    idempotency_key: None,
                    budgets: None,
                    checkpoint: Some(true),
                }))
                .expect("checkpoint start")
                .0
                .data
                .unwrap()["checkpoint_id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let direct_checkpoint = checkpoint(&server);
        {
            let runs = server.runs.lock().expect("runs");
            for index in 0..8 {
                let run_id = runs
                    .allocate("capacity", "v1", serde_json::json!({"index":index}))
                    .expect("slot record");
                runs.admit_async(&run_id).expect("slot admission");
            }
        }
        let direct = server
            .graph_run_resume(Parameters(RunResumeParams {
                checkpoint_id: Some(direct_checkpoint.clone()),
                run_id: None,
            }))
            .expect("resume response");
        assert_eq!(direct.0.error_code.as_deref(), Some("RUN_CAPACITY"));
        let store = server.store.as_ref().expect("store");
        assert!(store
            .load_resume_checkpoint(Some(&direct_checkpoint), None)
            .expect("checkpoint")
            .expect("record")
            .consumed_at
            .is_none());

        let approval_checkpoint = checkpoint(&server);
        let approval = server
            .graph_approval_request(Parameters(ApprovalRequestParams {
                checkpoint_id: approval_checkpoint.clone(),
                audience: "operator".into(),
                prompt: "approve after capacity is available".into(),
                allowed_decisions: vec!["approve".into()],
                expiration: (Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
            }))
            .expect("approval request");
        let approval_id = approval.0.data.unwrap()["approval_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let decided = server
            .graph_approval_decide(Parameters(ApprovalDecideParams {
                approval_id: approval_id.clone(),
                decision: "approve".into(),
                claimed_actor_label: "operator".into(),
            }))
            .expect("approval response");
        assert_eq!(
            decided.0.error_code.as_deref(),
            Some("AUTHENTICATED_OPERATOR_REQUIRED")
        );
        assert_eq!(
            store
                .get_checkpoint_approval(&approval_id)
                .expect("approval")
                .expect("approval row")
                .status,
            "pending"
        );
        assert!(store
            .load_resume_checkpoint(Some(&approval_checkpoint), None)
            .expect("checkpoint")
            .expect("record")
            .consumed_at
            .is_none());
    }
}

#[tool_router]
impl AgentGraphServer {
    // ── graph_create ──────────────────────────────────────────────────

    #[tool(
        description = "Create, validate, or delete a graph-orchestrated workflow from a JSON spec. Supports template instantiation and idempotency keys."
    )]
    fn graph_create(
        &self,
        Parameters(GraphCreateParams {
            spec,
            action,
            graph_id,
            idempotency_key,
            template,
            overwrite,
        }): Parameters<GraphCreateParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let action = action.as_deref().unwrap_or("create");

        let request_digest = digest(&serde_json::json!({
            "operation": "graph_create",
            "action": action,
            "spec": spec.as_ref().map(canonical_request_value).unwrap_or(Value::Null),
            "template": template.as_ref().map(canonical_request_value).unwrap_or(Value::Null),
            "graph_id": graph_id,
            "overwrite": overwrite.unwrap_or(false),
        }));
        if action != "delete" {
            if let Some(cached) = check_idempotency(
                self.store.as_ref(),
                idempotency_key.as_deref(),
                &request_digest,
            )? {
                return Ok(cached);
            }
        }

        // Lifecycle deletion is never model-authorized. The authenticated
        // operator IPC service is the sole mutation boundary and must perform
        // its own request, peer, nonce, and state-digest validation before it
        // reaches `delete_registered_graph`.
        if action == "delete" {
            return Ok(error_output(
                "graph lifecycle deletion requires the authenticated operator service",
                "AUTHENTICATED_OPERATOR_REQUIRED",
            ));
        }

        if action != "create" && action != "validate" {
            return Ok(error_output(
                format!("unsupported graph_create action '{action}'"),
                "INVALID_ACTION",
            ));
        }

        // ── create / validate ──
        let raw = if let Some(ref tpl) = template {
            let tpl_val = if let Value::String(s) = tpl {
                serde_json::from_str(s).unwrap_or_else(|_| tpl.clone())
            } else {
                tpl.clone()
            };
            let tpl_id = tpl_val
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid_params("template.id required"))?;
            let tpl_name = tpl_val
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| graph_id.as_deref())
                .unwrap_or(tpl_id);
            templates::instantiate(tpl_id, tpl_name)
                .map_err(|e| internal_error(format!("template error: {e}")))?
        } else {
            let spec = spec
                .clone()
                .ok_or_else(|| invalid_params("missing spec for create/validate"))?;
            if let Value::String(s) = spec {
                serde_json::from_str(&s)
                    .map_err(|e| invalid_params(format!("spec string parse error: {e}")))?
            } else {
                spec
            }
        };

        let original_version = raw
            .get("spec_version")
            .and_then(Value::as_str)
            .unwrap_or("1")
            .to_owned();
        let warnings_preview = serde_json::from_value::<GraphSpec>(raw.clone())
            .ok()
            .map(|s| s.warnings())
            .unwrap_or_default();
        let spec_parsed =
            parse_and_validate(&raw).map_err(|e| invalid_params(format!("invalid spec: {e}")))?;
        if let Some(node) = spec_parsed
            .nodes
            .iter()
            .find(|node| crate::spec::GraphSpec::executable_node_type(&node.node_type).is_err())
        {
            return Ok(error_output(
                format!("node '{}' declares an unsupported executable type", node.id),
                "UNSUPPORTED_NODE_TYPE",
            ));
        }
        let normalized =
            serde_json::to_value(&spec_parsed).map_err(|e| internal_error(e.to_string()))?;
        let version = digest(&normalized);
        let warnings = if original_version == "1" {
            warnings_preview
        } else {
            spec_parsed.warnings()
        };

        if action == "validate" {
            let output = output_with_meta(
                serde_json::json!({
                    "graph_id": spec_parsed.name,
                    "graph_version": version,
                    "digest": version,
                    "normalized_spec_version": "2",
                    "warnings": warnings,
                    "storage_class": "volatile",
                    "status": "valid"
                }),
                Some(&spec_parsed.name),
                Some(&version),
                None,
            );
            if let Some(ref store) = self.store {
                if let Some(idem) = idempotency_key {
                    if let Some(cached) =
                        persist_idempotency(store, &idem, &request_digest, &output)?
                    {
                        return Ok(cached);
                    }
                }
            }
            return Ok(output);
        }

        if Self::graph_requires_witness_store(&spec_parsed) && self.store.is_none() {
            return Ok(error_output(
                "evidence-required graphs require SQLite witness persistence",
                "WITNESS_STORE_REQUIRED",
            ));
        }

        // ── register ──
        let mut graphs = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let name = spec_parsed.name.clone();
        let overwrite = overwrite.unwrap_or(false);
        if !overwrite && !graphs.contains_key(&name) && graphs.len() >= self.max_graphs {
            return Ok(error_output(
                format!("graph capacity ({}) exhausted — {} graphs registered", self.max_graphs, graphs.len()),
                "CAPACITY_EXHAUSTED",
            ));
        }

        let id = name.clone();
        if let Some(ref store) = self.store {
            let spec_str = serde_json::to_string(&normalized).unwrap_or_default();
            if let Err(error) = store.save_graph(&id, &spec_str, &version, overwrite) {
                return Ok(error_output(error, "GRAPH_VERSION_CONFLICT"));
            }
        }
        graphs.insert(
            id.clone(),
            RegisteredGraph {
                spec: spec_parsed,
                normalized: normalized.clone(),
                version: version.clone(),
                warnings: warnings.clone(),
            },
        );
        drop(graphs);

        let output = output_with_meta(
            serde_json::json!({
                "graph_id": id,
                "graph_version": version,
                "digest": version,
                "normalized_spec_version": "2",
                "warnings": warnings,
                "storage_class": "volatile",
                "status": "created"
            }),
            Some(&id),
            Some(&version),
            None,
        );

        if let Some(ref store) = self.store {
            if let Some(idem) = idempotency_key {
                if let Some(cached) = persist_idempotency(store, &idem, &request_digest, &output)? {
                    return Ok(cached);
                }
            }
        }
        Ok(output)
    }

    // ── graph_execute ─────────────────────────────────────────────────

    #[tool(
        description = "Execute a registered graph. Sync mode blocks until completion; async mode returns immediately with a run_id."
    )]
    fn graph_execute(
        &self,
        Parameters(GraphExecuteParams {
            graph_id,
            input,
            graph_version,
            thread_id,
            mode,
            idempotency_key,
        }): Parameters<GraphExecuteParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let input = input.unwrap_or(Value::Null);
        ensure_size(&input, MAX_INPUT_BYTES, "execution input").map_err(|e| invalid_params(e))?;
        let sync = mode.as_deref().is_none_or(|m| m == "sync");
        if sync {
            tracing::warn!(
                "graph_execute sync mode is deprecated — use graph_run_start + graph_run_get"
            );
        }
        if let Some(store) = self.store.as_ref() {
            if !store
                .graph_execution_allowed(&graph_id)
                .map_err(internal_error)?
            {
                return Ok(error_output(
                    format!("graph '{graph_id}' is archived or pending deletion"),
                    "GRAPH_RETIRED",
                ));
            }
        }

        let graph = self.resolve_graph(&graph_id, graph_version.as_deref())?;

        if Self::graph_requires_witness_store(&graph.spec) && self.store.is_none() {
            return Ok(error_output(
                "evidence-required graphs require SQLite witness persistence",
                "WITNESS_STORE_REQUIRED",
            ));
        }

        let request_digest = digest(&serde_json::json!({
            "operation": "graph_execute",
            "graph_id": graph_id,
            "graph_spec": graph.normalized,
            "graph_version": graph.version,
            "input": input,
            "mode": mode.clone().unwrap_or_else(|| "sync".into()),
            "thread_id": thread_id,
        }));

        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        if let Some(idem) = idempotency_key.as_deref() {
            if let Some(cached) =
                check_idempotency(self.store.as_ref(), Some(idem), &request_digest)?
            {
                return Ok(cached);
            }
        }

        let run_id = runs
            .allocate(&graph_id, &graph.version, input.clone())
            .map_err(|e| internal_error(e))?;

        if let Err(e) = runs.admit_async(&run_id) {
            runs.remove(&run_id);
            return Ok(error_output(e, "RUN_CAPACITY"));
        }
        if let Some(ref store) = self.store {
            let _ = store.save_execution(
                &run_id,
                &graph_id,
                &graph.version,
                "running",
                &input.to_string(),
            );
        }

        let is_async = mode.as_deref() == Some("async");
        if is_async {
            let terminal_store = self.store.clone();
            let completion_runs = runs.clone();
            runs.start_with_completion_with_store(
                run_id.clone(),
                graph.spec,
                self.base_url.clone(),
                self.default_model.clone(),
                self.store.clone(),
                move |record| {
                    Self::persist_terminal_and_mark(completion_runs, terminal_store, record)
                },
            );
            let output = output_with_meta(
                serde_json::json!({
                    "run_id": run_id,
                    "status": "accepted",
                    "thread_id": thread_id,
                    "storage_class": "volatile",
                    "cancellation": "provider_future_best_effort_drop; underlying_request_may_continue"
                }),
                Some(&graph_id),
                Some(&graph.version),
                Some(&run_id),
            );
            if let Some(ref store) = self.store {
                if let Some(idem) = idempotency_key {
                    if let Some(cached) =
                        persist_idempotency(store, &idem, &request_digest, &output)?
                    {
                        return Ok(cached);
                    }
                }
            }
            return Ok(output);
        }

        let terminal_store = self.store.clone();
        let completion_runs = runs.clone();
        runs.start_with_completion_with_store(
            run_id.clone(),
            graph.spec,
            self.base_url.clone(),
            self.default_model.clone(),
            self.store.clone(),
            move |record| Self::persist_terminal_and_mark(completion_runs, terminal_store, record),
        );

        let deadline = Instant::now() + Duration::from_millis(300_000);
        let output = loop {
            let r = runs
                .get(&run_id)
                .ok_or_else(|| internal_error(format!("run '{run_id}' not found")))?;
            if matches!(r.status.as_str(), "completed" | "failed" | "cancelled") {
                break output_with_meta(
                    r.public(),
                    Some(&graph_id),
                    Some(&graph.version),
                    Some(&run_id),
                );
            }
            if Instant::now() >= deadline {
                let cancellation = runs.cancel(&run_id).unwrap_or_else(
                    |_| serde_json::json!({"run_id": run_id, "status": "cancellation_requested"}),
                );
                break output_with_meta(
                    serde_json::json!({
                        "run_id": run_id,
                        "status": r.status,
                        "timed_out": true,
                        "completion_unknown": true,
                        "cancellation": "requested",
                        "cancellation_result": cancellation,
                    }),
                    Some(&graph_id),
                    Some(&graph.version),
                    Some(&run_id),
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        };

        if let Some(ref store) = self.store {
            let status = output
                .0
                .data
                .as_ref()
                .and_then(|data| data.get("status").and_then(Value::as_str))
                .unwrap_or("failed");
            let final_state = output
                .0
                .data
                .as_ref()
                .and_then(|data| data.get("final_state").cloned())
                .map(|v| serde_json::to_string(&v).unwrap_or_default());
            let _ = store.save_execution(
                &run_id,
                &graph_id,
                &graph.version,
                status,
                &input.to_string(),
            );
            let _ =
                store.update_execution_status(&run_id, status, final_state.as_deref(), None, None);
        }

        if let Some(ref store) = self.store {
            if let Some(idem) = idempotency_key {
                if let Some(cached) = persist_idempotency(store, &idem, &request_digest, &output)? {
                    return Ok(cached);
                }
            }
        }
        Ok(output)
    }

    // ── Local source witness capture ─────────────────────────────────

    #[tool(
        description = "Persist caller-supplied UTF-8 source content as a local witness receipt. The locator is metadata only; this tool never fetches or verifies it."
    )]
    fn graph_source_witness_capture(
        &self,
        Parameters(WitnessCaptureParams {
            locator,
            content,
            media_type,
            authority_class,
            retrieved_at,
        }): Parameters<WitnessCaptureParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let capture = WitnessCapture {
            locator,
            content,
            media_type,
            authority_class,
            retrieved_at: retrieved_at
                .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)),
        };
        if let Err(error) = validate_witness_capture(capture.clone()) {
            return Ok(Self::witness_error_output(error));
        }
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for source witness capture",
                "WITNESS_STORE_REQUIRED",
            ));
        };
        match store.capture_witness(capture) {
            Ok(record) => Ok(structured_output(serde_json::json!({
                "witness_id": record.witness_id,
                "digest": record.digest,
                "locator_digest": digest(&Value::String(record.locator)),
                "media_type": record.media_type,
                "authority_class": record.authority_class,
                "retrieved_at": record.retrieved_at,
                "content_bytes": record.content.len(),
                "storage_class": "sqlite_source_witness"
            }))),
            Err(error) => Ok(Self::witness_error_output(error)),
        }
    }

    #[tool(
        description = "Read one exact local source witness ID, verifying its HMAC-SHA256 authentication tag before returning metadata and captured content."
    )]
    fn graph_source_witness_get(
        &self,
        Parameters(WitnessGetParams { witness_id }): Parameters<WitnessGetParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for source witness reads",
                "WITNESS_STORE_REQUIRED",
            ));
        };
        match store.get_witness(&witness_id) {
            Ok(Some(record)) => {
                let locator_digest = digest(&Value::String(record.locator.clone()));
                Ok(structured_output(serde_json::json!({
                    "witness_id": record.witness_id,
                    "digest": record.digest,
                    "locator": record.locator,
                    "locator_digest": locator_digest,
                    "content": record.content,
                    "media_type": record.media_type,
                    "authority_class": record.authority_class,
                    "retrieved_at": record.retrieved_at,
                    "storage_class": "sqlite_source_witness"
                })))
            }
            Ok(None) => Ok(error_output(
                "source witness was not found",
                "WITNESS_NOT_FOUND",
            )),
            Err(error) => Ok(Self::witness_error_output(error)),
        }
    }

    // ── graph_status ──────────────────────────────────────────────────

    #[tool(
        description = "Query server state, graph details, run status, events, receipts, or templates."
    )]
    fn graph_status(
        &self,
        Parameters(GraphStatusParams {
            resource,
            graph_id,
            run_id,
            cursor,
            limit,
        }): Parameters<GraphStatusParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let resource = resource.as_deref();

        // Server-level summary (no resource or resource="server")
        if resource.is_none() || resource == Some("server") {
            let graphs = self
                .graphs
                .lock()
                .map_err(|e| internal_error(e.to_string()))?;
            let graph_names: Vec<&String> = graphs.keys().collect();
            let runs = self
                .runs
                .lock()
                .map_err(|e| internal_error(e.to_string()))?;
            let run_ids = runs.list();
            let durable_integrity = self
                .store
                .as_ref()
                .is_some_and(PersistentStore::has_integrity_key);

            return Ok(structured_output(serde_json::json!({
                "graphs": graph_names,
                "graph_count": graphs.len(),
                "execution_count": run_ids.len(),
                "retained_execution_count": run_ids.len(),
                "total_execution_count": run_ids.len(),
                "base_url": self.safe_provider_label(),
                "default_model": self.default_model,
                "storage_class": if self.store.is_none() {
                    "process_local"
                } else if durable_integrity {
                    "persisted_integrity_verified"
                } else {
                    "persisted_unverified"
                },
                "capabilities": {
                    "runtime": "agent_graph",
                    "async_start": true,
                    "api_key_configured": self.api_key.is_some(),
                    "cancellation": "provider_future_best_effort_drop; underlying_request_may_continue",
                    "durable_resume": if durable_integrity {
                        Value::String("deterministic_local_resume_only".into())
                    } else {
                        Value::Bool(false)
                    },
                    "terminal_persistence": if durable_integrity { "sqlite_projection_only" } else { "disabled_without_integrity_key" },
                    "checkpointing": if durable_integrity { "deterministic_local_pre_execution" } else { "unavailable" },
                    "events": if self.store.is_some() { "terminal_persisted_projection_with_sqlite_fallback" } else { "volatile_in_memory_only" },
                    "event_replay": "not_replayable_execution",
                    "restart_recovery": "interrupted_non_resumable",
                    "budgets": {
                        "max_wall_clock_ms": "enforced",
                        "max_nodes": "enforced_at_engine_superstep_boundary",
                        "max_llm_calls": "enforced_before_provider_invocation"
                    },
                    "state_write_conflicts": "rejected_without_explicit_reducer",
                    "evidence": "witness_bound_local_capture_only; locators_not_fetched; source_authority_not_verified",
                    "evidence_authority": "caller_supplied_unverified_or_local_primary_capture",
                    "hitl": if durable_integrity { "checkpoint_bound_durable_approval_only" } else { "unavailable" },
                    "replay": "integrity_only"
                },
                "limits": {"graphs": self.max_graphs},
                "capacity_state": if graphs.len() <= self.max_graphs {
                    "within_limit"
                } else {
                    "over_limit_legacy"
                }
            })));
        }

        match resource.unwrap() {
            "templates" => Ok(structured_output(templates::list())),

            "graph" => {
                let id = graph_id
                    .as_deref()
                    .ok_or_else(|| invalid_params("missing graph_id"))?;
                let graphs = self
                    .graphs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                let g = graphs
                    .get(id)
                    .ok_or_else(|| invalid_params(format!("graph '{id}' not found")))?;
                Ok(output_with_meta(
                    serde_json::json!({
                        "graph_id": id,
                        "graph_version": g.version,
                        "normalized_spec": g.normalized,
                        "mermaid": Self::mermaid(&g.spec),
                        "warnings": g.warnings,
                        "storage_class": "volatile"
                    }),
                    Some(id),
                    Some(&g.version),
                    None,
                ))
            }

            "run" => {
                let runs = self
                    .runs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                if run_id.is_none() {
                    // List all runs
                    return Ok(structured_output(serde_json::json!({
                        "runs": runs.list()
                    })));
                }
                let id = run_id.as_deref().unwrap();
                let r = runs
                    .get(id)
                    .ok_or_else(|| invalid_params(format!("run '{id}' not found")))?;
                Ok(structured_output(r.public()))
            }

            "events" => {
                let id = run_id
                    .as_deref()
                    .ok_or_else(|| invalid_params("missing run_id for events"))?;
                let runs = self
                    .runs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                let cursor_val = cursor.unwrap_or(0);
                let limit_val = limit.unwrap_or(100) as usize;
                let result = runs
                    .events(self.store.as_ref(), id, cursor_val, limit_val)
                    .map_err(|e| invalid_params(e))?;
                Ok(output_with_meta(result, None, None, Some(id)))
            }

            "receipt" => {
                let id = run_id
                    .as_deref()
                    .ok_or_else(|| invalid_params("missing run_id for receipt"))?;
                let runs = self
                    .runs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                let r = runs
                    .get(id)
                    .ok_or_else(|| invalid_params(format!("run '{id}' not found")))?;
                Ok(output_with_meta(r.receipt.clone(), None, None, Some(id)))
            }

            "bundle" => {
                let id = run_id
                    .as_deref()
                    .ok_or_else(|| invalid_params("missing run_id for bundle"))?;
                let runs = self
                    .runs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                let r = runs
                    .get(id)
                    .ok_or_else(|| invalid_params(format!("run '{id}' not found")))?;
                Ok(output_with_meta(r.bundle.clone(), None, None, Some(id)))
            }

            _ => Ok(error_output(
                format!("unknown status resource '{}'", resource.unwrap_or("")),
                "INVALID_RESOURCE",
            )),
        }
    }

    // ── graph_list (NEW) ──────────────────────────────────────────────

    #[tool(
        description = "List all registered graphs with metadata (name, node count, edge count, version)."
    )]
    fn graph_list(
        &self,
        Parameters(GraphListParams { query, limit }): Parameters<GraphListParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let graphs = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;

        let mut entries: Vec<Value> = graphs
            .iter()
            .filter(|(name, _)| {
                let visible = self
                    .store
                    .as_ref()
                    .map(|store| store.graph_is_tombstoned(name).map(|v| !v).unwrap_or(false))
                    .unwrap_or(true);
                query
                    .as_ref()
                    .map(|q| name.contains(q.as_str()))
                    .unwrap_or(true)
                    && visible
            })
            .take(limit.unwrap_or(50) as usize)
            .map(|(name, g)| {
                let version_history = self
                    .store
                    .as_ref()
                    .and_then(|store| store.list_graph_versions(name).ok())
                    .unwrap_or_else(|| vec![g.version.clone()]);
                serde_json::json!({
                    "name": name,
                    "version": g.version,
                    "current_version": g.version,
                    "version_history": version_history,
                    "historical_specs": self.store.is_some(),
                    "node_count": g.spec.nodes.len(),
                    "edge_count": g.spec.edges.len(),
                    "entry": g.spec.entry,
                    "warnings": g.warnings,
                })
            })
            .collect();

        entries.sort_by(|a, b| {
            a.get("name")
                .and_then(Value::as_str)
                .cmp(&b.get("name").and_then(Value::as_str))
        });

        Ok(structured_output(serde_json::json!({
            "graphs": entries,
            "count": entries.len(),
        })))
    }

    #[tool(
        description = "Read a durable graph retention inventory with execution, version, and inbound-subgraph reference counts. This tool never changes graph state."
    )]
    fn graph_retention_review(
        &self,
        Parameters(GraphRetentionReviewParams {
            graph_id,
            state,
            limit,
        }): Parameters<GraphRetentionReviewParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for graph retention review",
                "RETENTION_STORE_REQUIRED",
            ));
        };
        let reports = store
            .graph_retention_review(
                graph_id.as_deref(),
                state.as_deref(),
                limit.unwrap_or(100).clamp(1, 256) as usize,
            )
            .map_err(internal_error)?;
        let graphs: Vec<Value> = reports
            .into_iter()
            .map(|report| {
                serde_json::json!({
                    "graph_id": report.graph_id,
                    "state": report.state,
                    "reason": report.reason,
                    "actor": report.actor,
                    "review_after": report.review_after,
                    "created_at": report.created_at,
                    "updated_at": report.updated_at,
                    "version_count": report.version_count,
                    "execution_count": report.execution_count,
                    "last_execution_at": report.last_execution_at,
                    "state_digest": report.state_digest,
                    "tombstoned": report.tombstoned,
                    "inbound_subgraph_refs": report.inbound_subgraph_refs,
                    "deletion_eligible": report.deletion_eligible,
                })
            })
            .collect();
        Ok(structured_output(serde_json::json!({
            "graphs": graphs,
            "count": graphs.len(),
            "storage_class": "sqlite_graph_retention",
        })))
    }

    #[tool(
        description = "Set an explicit durable graph lifecycle state. delete_approved requires a prior delete_candidate state and no execution or inbound-subgraph references."
    )]
    fn graph_retention_set(
        &self,
        Parameters(GraphRetentionSetParams {
            graph_id,
            state,
            reason,
            actor,
            review_after,
        }): Parameters<GraphRetentionSetParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let _ = (graph_id, state, reason, actor, review_after);
        Ok(error_output(
            "graph lifecycle updates require the authenticated operator service",
            "AUTHENTICATED_OPERATOR_REQUIRED",
        ))
    }

    // ── graph_delete (NEW) ────────────────────────────────────────────

    #[allow(dead_code)]
    fn graph_delete(
        &self,
        Parameters(GraphDeleteParams { graph_id }): Parameters<GraphDeleteParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        self.delete_registered_graph(&graph_id)
    }

    // ── graph_inspect (NEW) ───────────────────────────────────────────

    #[tool(
        description = "Get a graph's full topology: nodes, edges, Mermaid diagram, and topology hash."
    )]
    fn graph_inspect(
        &self,
        Parameters(GraphInspectParams { graph_id }): Parameters<GraphInspectParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let graphs = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let g = graphs
            .get(&graph_id)
            .ok_or_else(|| invalid_params(format!("graph '{graph_id}' not found")))?;

        let nodes: Vec<Value> = g
            .spec
            .nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "type": n.node_type,
                    "config": n.config,
                })
            })
            .collect();

        let edges: Vec<Value> = g
            .spec
            .edges
            .iter()
            .map(|e| {
                serde_json::json!({
                    "from": e.from,
                    "to": e.to,
                })
            })
            .collect();

        Ok(output_with_meta(
            serde_json::json!({
                "name": graph_id,
                "version": g.version,
                "current_version": g.version,
                "version_history": self.store.as_ref().and_then(|store| store.list_graph_versions(&graph_id).ok()).unwrap_or_else(|| vec![g.version.clone()]),
                "historical_specs": self.store.is_some(),
                "entry": g.spec.entry,
                "max_iterations": g.spec.max_iterations,
                "max_parallelism": g.spec.max_parallelism,
                "nodes": nodes,
                "node_count": nodes.len(),
                "edges": edges,
                "edge_count": edges.len(),
                "mermaid": Self::mermaid(&g.spec),
                "topology_hash": g.version,
                "reducers": g.spec.reducers,
                "warnings": g.warnings,
            }),
            Some(&graph_id),
            Some(&g.version),
            None,
        ))
    }

    // ── Approval lifecycle ────────────────────────────────────────────

    fn validate_resume_checkpoint(
        &self,
        store: &PersistentStore,
        checkpoint: &CheckpointRecord,
    ) -> Result<
        (
            crate::store::ExecutionContract,
            RegisteredGraph,
            Option<RunBudgets>,
        ),
        (String, String),
    > {
        let contract = store
            .load_execution_contract(&checkpoint.run_id)
            .map_err(|error| (error, "CHECKPOINT_PERSISTENCE_FAILURE".into()))?
            .ok_or_else(|| {
                (
                    "checkpoint execution contract was not found".into(),
                    "CHECKPOINT_INTEGRITY_FAILURE".into(),
                )
            })?;
        if contract.graph_id != checkpoint.graph_id
            || contract.graph_version != checkpoint.graph_version
            || checkpoint.terminal_cursor != 0
            || checkpoint.event_cursor != 0
        {
            return Err((
                "checkpoint integrity validation failed".into(),
                "CHECKPOINT_INTEGRITY_FAILURE".into(),
            ));
        }
        let graph = self
            .resolve_graph(&checkpoint.graph_id, Some(&checkpoint.graph_version))
            .map_err(|_| {
                (
                    "checkpoint graph version is unavailable".into(),
                    "CHECKPOINT_INTEGRITY_FAILURE".into(),
                )
            })?;
        let eligibility = graph.spec.resume_eligibility().map_err(|_| {
            (
                "checkpoint graph is no longer in the deterministic local resume subset".into(),
                "RESUME_INELIGIBLE".into(),
            )
        })?;
        if graph.version != checkpoint.graph_version
            || checkpoint.next_node_cursor != eligibility.next_node_cursor
            || checkpoint.dependency_summary != eligibility.dependency_summary
            || checkpoint.dependency_digest != digest(&eligibility.dependency_summary)
            || checkpoint.state != initial_state_for_input(&contract.input)
            || checkpoint.budgets != contract.budgets
            || checkpoint.budget_counters
                != serde_json::json!({"nodes":0,"llm_calls":0,"wall_clock_ms":0})
        {
            return Err((
                "checkpoint integrity validation failed".into(),
                "CHECKPOINT_INTEGRITY_FAILURE".into(),
            ));
        }
        let budgets = RunBudgets::parse(Some(&checkpoint.budgets)).map_err(|_| {
            (
                "checkpoint budgets failed validation".into(),
                "CHECKPOINT_INTEGRITY_FAILURE".into(),
            )
        })?;
        Ok((contract, graph, budgets))
    }

    fn launch_resumed(
        &self,
        checkpoint: CheckpointRecord,
        contract: crate::store::ExecutionContract,
        graph: RegisteredGraph,
        budgets: Option<RunBudgets>,
        approval: Option<Value>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        if runs.get(&checkpoint.run_id).is_some() {
            let _ = runs.remove(&checkpoint.run_id);
        }
        let run_id = match runs.allocate_resumed(
            &checkpoint.run_id,
            &checkpoint.graph_id,
            &checkpoint.graph_version,
            contract.input,
            checkpoint.state.clone(),
            budgets,
            &checkpoint.checkpoint_id,
            &checkpoint.checkpoint_digest,
            approval.clone(),
        ) {
            Ok(run_id) => run_id,
            Err(error) => {
                runs.release_async_slot();
                return Ok(error_output(error, "RUN_CAPACITY"));
            }
        };
        if let Err(error) = runs.admit_reserved_async(&run_id) {
            runs.remove(&run_id);
            runs.release_async_slot();
            return Ok(error_output(error, "RUN_CAPACITY"));
        }
        self.store
            .as_ref()
            .expect("resumed launch requires SQLite")
            .update_execution_status(&run_id, "running", None, None, None)
            .map_err(internal_error)?;
        let terminal_store = self.store.clone();
        let completion_runs = runs.clone();
        runs.start_resumed_with_completion(
            run_id.clone(),
            graph.spec,
            self.base_url.clone(),
            self.default_model.clone(),
            self.store.clone(),
            move |record| Self::persist_terminal_and_mark(completion_runs, terminal_store, record),
        );
        Ok(output_with_meta(
            serde_json::json!({
                "run_id": run_id,
                "status": "running",
                "checkpoint": checkpoint_value(&checkpoint),
                "resume_capability": "deterministic_local_resume",
                "approval": approval,
            }),
            Some(&checkpoint.graph_id),
            Some(&checkpoint.graph_version),
            Some(&run_id),
        ))
    }

    #[tool(
        description = "Create a durable approval request bound to one unconsumed deterministic-local checkpoint."
    )]
    fn graph_approval_request(
        &self,
        Parameters(ApprovalRequestParams {
            checkpoint_id,
            audience,
            prompt,
            allowed_decisions,
            expiration,
        }): Parameters<ApprovalRequestParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for durable approvals",
                "APPROVAL_STORE_REQUIRED",
            ));
        };
        if audience.trim().is_empty() || audience.len() > 256 {
            return Ok(error_output(
                "audience must be non-empty and at most 256 bytes",
                "INVALID_PARAMS",
            ));
        }
        if allowed_decisions.is_empty()
            || allowed_decisions
                .iter()
                .any(|decision| !matches!(decision.as_str(), "approve" | "reject"))
        {
            return Ok(error_output(
                "allowed_decisions must be a non-empty subset of approve and reject",
                "INVALID_PARAMS",
            ));
        }
        if chrono::DateTime::parse_from_rfc3339(&expiration).is_err() {
            return Ok(error_output("expiration must be RFC3339", "INVALID_PARAMS"));
        }
        if prompt.len() > 16 * 1024 {
            return Ok(error_output(
                "prompt exceeds the bounded approval prompt size",
                "INVALID_PARAMS",
            ));
        }
        let checkpoint = match store.load_resume_checkpoint(Some(&checkpoint_id), None) {
            Ok(Some(checkpoint)) => checkpoint,
            Ok(None) => return Ok(checkpoint_error_output(CheckpointError::NotFound)),
            Err(error) => return Ok(checkpoint_error_output(error)),
        };
        if checkpoint.consumed_at.is_some() {
            return Ok(checkpoint_error_output(CheckpointError::Consumed));
        }
        if let Err((message, code)) = self.validate_resume_checkpoint(store, &checkpoint) {
            return Ok(error_output(message, code));
        }
        let prompt_digest = digest(&Value::String(prompt));
        let approval = match store.create_checkpoint_approval(
            &checkpoint.checkpoint_id,
            &checkpoint.graph_id,
            &checkpoint.graph_version,
            &checkpoint.next_node_cursor,
            &checkpoint.state,
            &checkpoint.budgets,
            &checkpoint.budget_counters,
            &checkpoint.dependency_summary,
            &audience,
            &prompt_digest,
            &allowed_decisions,
            &expiration,
        ) {
            Ok(approval) => approval,
            Err(error) => return Ok(approval_error_output(error)),
        };
        Ok(output_with_meta(
            approval_value(&approval),
            Some(&approval.graph_id),
            Some(&approval.graph_version),
            Some(&approval.run_id),
        ))
    }

    #[tool(
        description = "Read durable checkpoint-bound approval metadata from SQLite without raw prompt or checkpoint state."
    )]
    fn graph_approval_list(
        &self,
        Parameters(ApprovalListParams {
            run_id,
            status,
            limit,
        }): Parameters<ApprovalListParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for durable approvals",
                "APPROVAL_STORE_REQUIRED",
            ));
        };
        let approvals = store
            .list_checkpoint_approvals(
                run_id.as_deref(),
                status.as_deref(),
                limit.unwrap_or(50) as usize,
            )
            .map_err(|error| internal_error(error.message()))?;
        Ok(structured_output(serde_json::json!({
            "approvals": approvals.iter().map(approval_value).collect::<Vec<_>>(),
            "count": approvals.len(),
            "storage_class": "sqlite_durable_approval_metadata",
        })))
    }

    #[tool(
        description = "Read one durable checkpoint-bound approval's metadata from SQLite without raw prompt or checkpoint state."
    )]
    fn graph_approval_get(
        &self,
        Parameters(ApprovalGetParams { approval_id }): Parameters<ApprovalGetParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for durable approvals",
                "APPROVAL_STORE_REQUIRED",
            ));
        };
        match store
            .get_checkpoint_approval(&approval_id)
            .map_err(|error| internal_error(error.message()))?
        {
            Some(approval) => Ok(output_with_meta(
                approval_value(&approval),
                Some(&approval.graph_id),
                Some(&approval.graph_version),
                Some(&approval.run_id),
            )),
            None => Ok(approval_error_output(ApprovalError::NotFound)),
        }
    }

    #[allow(dead_code)]
    fn graph_approval_decide(
        &self,
        Parameters(ApprovalDecideParams {
            approval_id: _,
            decision: _,
            claimed_actor_label: _,
        }): Parameters<ApprovalDecideParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        return Ok(error_output(
            "approval decisions require authenticated operator transport",
            "AUTHENTICATED_OPERATOR_REQUIRED",
        ));
    }

    // ── Async run lifecycle ───────────────────────────────────────────

    #[tool(
        description = "Start an async graph run. Returns run_id immediately; use graph_run_wait to block on completion. Optional budgets accept only positive integer max_wall_clock_ms, max_nodes, or max_llm_calls fields; max_llm_calls is enforced before each provider invocation."
    )]
    fn graph_run_start(
        &self,
        Parameters(RunStartParams {
            graph_id,
            input,
            graph_version,
            thread_id,
            idempotency_key,
            budgets,
            checkpoint,
        }): Parameters<RunStartParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let requested_budgets = match RunBudgets::parse(budgets.as_ref()) {
            Ok(budgets) => budgets,
            Err(error) => return Ok(error_output(error, "INVALID_BUDGETS")),
        };
        let input = input.unwrap_or(Value::Null);
        let checkpoint_requested = checkpoint.unwrap_or(false);
        ensure_size(&input, MAX_INPUT_BYTES, "execution input").map_err(|e| invalid_params(e))?;

        let RegisteredGraph {
            spec,
            normalized,
            version,
            ..
        } = self.resolve_graph(&graph_id, graph_version.as_deref())?;

        if Self::graph_requires_witness_store(&spec) && self.store.is_none() {
            return Ok(error_output(
                "evidence-required graphs require SQLite witness persistence",
                "WITNESS_STORE_REQUIRED",
            ));
        }

        let eligibility = if checkpoint_requested {
            match spec.resume_eligibility() {
                Ok(eligibility) => Some(eligibility),
                Err(reason) => return Ok(error_output(reason, "RESUME_INELIGIBLE")),
            }
        } else {
            None
        };

        let request_digest = digest(&serde_json::json!({
            "operation": "graph_run_start",
            "graph_id": graph_id,
            "graph_spec": normalized,
            "graph_version": version,
            "input": input,
            "thread_id": thread_id,
            "budgets": requested_budgets
                .as_ref()
                .map(RunBudgets::requested_value)
                .unwrap_or(Value::Null),
            "checkpoint": checkpoint_requested,
        }));

        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        if let Some(idem) = idempotency_key.as_deref() {
            if let Some(cached) =
                check_idempotency(self.store.as_ref(), Some(idem), &request_digest)?
            {
                return Ok(cached);
            }
        }

        if checkpoint_requested {
            let Some(store) = self.store.as_ref() else {
                return Ok(error_output(
                    "SQLite persistence is required for deterministic checkpoints",
                    "CHECKPOINT_STORE_REQUIRED",
                ));
            };
            let eligibility = eligibility.expect("checkpoint eligibility");
            let state = initial_state_for_input(&input);
            let budgets_value = requested_budgets
                .as_ref()
                .map(RunBudgets::requested_value)
                .unwrap_or(Value::Null);
            let counters = serde_json::json!({"nodes":0,"llm_calls":0,"wall_clock_ms":0});
            let run_id = runs
                .allocate_with_budgets(
                    &graph_id,
                    &version,
                    input.clone(),
                    requested_budgets.clone(),
                )
                .map_err(|e| internal_error(e))?;
            if let Err(error) = store.save_execution_with_budgets(
                &run_id,
                &graph_id,
                &version,
                "checkpointed",
                &input.to_string(),
                Some(&budgets_value.to_string()),
            ) {
                runs.remove(&run_id);
                return Ok(error_output(error, "CHECKPOINT_PERSISTENCE_FAILURE"));
            }
            let checkpoint_record = match store.create_resume_checkpoint(
                &run_id,
                &graph_id,
                &version,
                &eligibility.next_node_cursor,
                &state,
                &budgets_value,
                &counters,
                &eligibility.dependency_summary,
                0,
                0,
            ) {
                Ok(record) => record,
                Err(error) => {
                    let _ = store.update_execution_status(&run_id, "failed", None, None, None);
                    runs.remove(&run_id);
                    return Ok(checkpoint_error_output(error));
                }
            };
            runs.mark_checkpointed(
                &run_id,
                &checkpoint_record.checkpoint_id,
                &checkpoint_record.checkpoint_digest,
            )
            .map_err(internal_error)?;
            let output = output_with_meta(
                serde_json::json!({
                    "run_id": run_id,
                    "status": "checkpointed",
                    "thread_id": thread_id,
                    "checkpoint_id": checkpoint_record.checkpoint_id,
                    "checkpoint_digest": checkpoint_record.checkpoint_digest,
                    "checkpoint": checkpoint_value(&checkpoint_record),
                    "resume_capability": "deterministic_local_resume",
                }),
                Some(&graph_id),
                Some(&version),
                Some(&run_id),
            );
            if let Some(idem) = idempotency_key {
                if let Some(cached) = persist_idempotency(store, &idem, &request_digest, &output)? {
                    return Ok(cached);
                }
            }
            return Ok(output);
        }

        let run_id = runs
            .allocate_with_budgets(&graph_id, &version, input.clone(), requested_budgets)
            .map_err(|e| internal_error(e))?;
        if let Err(e) = runs.admit_async(&run_id) {
            runs.remove(&run_id);
            return Ok(error_output(e, "RUN_CAPACITY"));
        }

        if let Some(ref store) = self.store {
            let _ =
                store.save_execution(&run_id, &graph_id, &version, "running", &input.to_string());
        }

        let terminal_store = self.store.clone();
        let completion_runs = runs.clone();
        runs.start_with_completion_with_store(
            run_id.clone(),
            spec,
            self.base_url.clone(),
            self.default_model.clone(),
            self.store.clone(),
            move |record| Self::persist_terminal_and_mark(completion_runs, terminal_store, record),
        );

        let output = output_with_meta(
            serde_json::json!({
                "run_id": run_id,
                "status": "running",
                "thread_id": thread_id,
            }),
            Some(&graph_id),
            Some(&version),
            Some(&run_id),
        );
        if let Some(ref store) = self.store {
            if let Some(idem) = idempotency_key {
                if let Some(cached) = persist_idempotency(store, &idem, &request_digest, &output)? {
                    return Ok(cached);
                }
            }
        }

        Ok(output)
    }

    #[tool(
        description = "Read one durable deterministic-local checkpoint, including its integrity-bound state and resume metadata."
    )]
    fn graph_run_checkpoint(
        &self,
        Parameters(RunCheckpointParams {
            run_id,
            checkpoint_id,
        }): Parameters<RunCheckpointParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for checkpoint reads",
                "CHECKPOINT_STORE_REQUIRED",
            ));
        };
        if run_id.is_none() && checkpoint_id.is_none() {
            return Ok(error_output(
                "run_id or checkpoint_id is required for checkpoint reads",
                "INVALID_PARAMS",
            ));
        }
        match store.load_resume_checkpoint(checkpoint_id.as_deref(), run_id.as_deref()) {
            Ok(Some(record))
                if run_id
                    .as_deref()
                    .is_none_or(|run_id| record.run_id == run_id) =>
            {
                Ok(output_with_meta(
                    checkpoint_value(&record),
                    Some(&record.graph_id),
                    Some(&record.graph_version),
                    Some(&record.run_id),
                ))
            }
            Ok(Some(_)) => Ok(checkpoint_error_output(CheckpointError::Integrity)),
            Ok(None) => Ok(checkpoint_error_output(CheckpointError::NotFound)),
            Err(error) => Ok(checkpoint_error_output(error)),
        }
    }

    #[tool(
        description = "Consume one deterministic-local checkpoint atomically and resume its pinned run exactly once."
    )]
    fn graph_run_resume(
        &self,
        Parameters(RunResumeParams {
            checkpoint_id,
            run_id,
        }): Parameters<RunResumeParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let Some(store) = self.store.as_ref() else {
            return Ok(error_output(
                "SQLite persistence is required for deterministic resume",
                "CHECKPOINT_STORE_REQUIRED",
            ));
        };
        if checkpoint_id.is_none() && run_id.is_none() {
            return Ok(error_output(
                "checkpoint_id or run_id is required for resume",
                "INVALID_PARAMS",
            ));
        }
        let checkpoint =
            match store.load_resume_checkpoint(checkpoint_id.as_deref(), run_id.as_deref()) {
                Ok(Some(record)) => record,
                Ok(None) => return Ok(checkpoint_error_output(CheckpointError::NotFound)),
                Err(error) => return Ok(checkpoint_error_output(error)),
            };
        if store
            .checkpoint_approval_status(&checkpoint.checkpoint_id)
            .map_err(|error| internal_error(error.message()))?
            .as_deref()
            == Some("pending")
        {
            return Ok(error_output(
                "checkpoint resume is pending its durable approval decision",
                "APPROVAL_PENDING",
            ));
        }
        if checkpoint.consumed_at.is_some() {
            return Ok(checkpoint_error_output(CheckpointError::Consumed));
        }
        if run_id
            .as_deref()
            .is_some_and(|run_id| run_id != checkpoint.run_id)
        {
            return Ok(checkpoint_error_output(CheckpointError::Integrity));
        }
        let Some(contract) = store
            .load_execution_contract(&checkpoint.run_id)
            .map_err(internal_error)?
        else {
            return Ok(checkpoint_error_output(CheckpointError::Integrity));
        };
        if contract.graph_id != checkpoint.graph_id
            || contract.graph_version != checkpoint.graph_version
            || checkpoint.terminal_cursor != 0
            || checkpoint.event_cursor != 0
        {
            return Ok(checkpoint_error_output(CheckpointError::Integrity));
        }
        let graph = match self.resolve_graph(&checkpoint.graph_id, Some(&checkpoint.graph_version))
        {
            Ok(graph) => graph,
            Err(_) => return Ok(checkpoint_error_output(CheckpointError::Integrity)),
        };
        if graph.version != checkpoint.graph_version {
            return Ok(checkpoint_error_output(CheckpointError::Integrity));
        }
        let eligibility = match graph.spec.resume_eligibility() {
            Ok(eligibility) => eligibility,
            Err(_) => {
                return Ok(error_output(
                    "checkpoint graph is no longer in the deterministic local resume subset",
                    "RESUME_INELIGIBLE",
                ));
            }
        };
        if checkpoint.next_node_cursor != eligibility.next_node_cursor
            || checkpoint.dependency_summary != eligibility.dependency_summary
            || checkpoint.dependency_digest != digest(&eligibility.dependency_summary)
            || checkpoint.state != initial_state_for_input(&contract.input)
            || checkpoint.budgets != contract.budgets
            || checkpoint.budget_counters
                != serde_json::json!({"nodes":0,"llm_calls":0,"wall_clock_ms":0})
        {
            return Ok(checkpoint_error_output(CheckpointError::Integrity));
        }
        let budgets = match RunBudgets::parse(Some(&checkpoint.budgets)) {
            Ok(budgets) => budgets,
            Err(_) => return Ok(checkpoint_error_output(CheckpointError::Integrity)),
        };
        let reserved_runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        if let Err(error) = reserved_runs.reserve_async_slot() {
            return Ok(error_output(error, "RUN_CAPACITY"));
        }
        drop(reserved_runs);
        let consumed = match store.consume_resume_checkpoint(&checkpoint.checkpoint_id) {
            Ok(record) => record,
            Err(error) => {
                if let Ok(runs) = self.runs.lock() {
                    runs.release_async_slot();
                }
                return Ok(checkpoint_error_output(error));
            }
        };
        self.launch_resumed(consumed, contract, graph, budgets, None)
    }

    #[tool(description = "Wait for an async run to complete, with optional timeout.")]
    fn graph_run_wait(
        &self,
        Parameters(RunWaitParams { run_id, timeout_ms }): Parameters<RunWaitParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let timeout = Duration::from_millis(timeout_ms.unwrap_or(300_000));
        let deadline = Instant::now() + timeout;
        loop {
            let r = {
                let runs = self
                    .runs
                    .lock()
                    .map_err(|e| internal_error(e.to_string()))?;
                runs.get(&run_id)
                    .ok_or_else(|| invalid_params(format!("run '{run_id}' not found")))?
            };
            if matches!(r.status.as_str(), "completed" | "failed" | "cancelled") {
                let persist = Self::persist_terminal(self.store.clone(), r.clone());
                if let Ok(runs) = self.runs.lock() {
                    if self.store.is_none() {
                        runs.mark_persistence(&run_id, "volatile_no_store", None);
                    } else {
                        match persist {
                            Ok(()) => runs.mark_persistence(&run_id, "durable_terminal", None),
                            Err(error) => runs.mark_persistence(
                                &run_id,
                                "volatile_persistence_failed",
                                Some(error),
                            ),
                        }
                    }
                }
                let public = self
                    .runs
                    .lock()
                    .ok()
                    .and_then(|runs| runs.get(&run_id).map(|record| record.public()))
                    .unwrap_or_else(|| r.public());
                return Ok(output_with_meta(public, None, None, Some(&run_id)));
            }
            if Instant::now() >= deadline {
                return Ok(output_with_meta(
                    serde_json::json!({
                        "run_id": run_id,
                        "status": r.status,
                        "timed_out": true,
                    }),
                    None,
                    None,
                    Some(&run_id),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[tool(description = "Cancel a running execution.")]
    fn graph_run_cancel(
        &self,
        Parameters(RunCancelParams { run_id, reason: _ }): Parameters<RunCancelParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        match runs.cancel(&run_id) {
            Ok(_) => {}
            Err(error) if error == "RUN_NOT_CANCELLABLE" => {
                return Ok(error_output(
                    "terminal or checkpointed runs cannot be cancelled",
                    "RUN_NOT_CANCELLABLE",
                ));
            }
            Err(error) if error == "run not found" => {
                drop(runs);
                if self
                    .store
                    .as_ref()
                    .and_then(|store| store.load_execution(&run_id).ok().flatten())
                    .is_some_and(|stored| {
                        matches!(
                            stored.get("status").and_then(Value::as_str),
                            Some("completed" | "failed" | "cancelled" | "checkpointed")
                        )
                    })
                {
                    return Ok(error_output(
                        "terminal or checkpointed runs cannot be cancelled",
                        "RUN_NOT_CANCELLABLE",
                    ));
                }
                return Err(invalid_params(error));
            }
            Err(error) => return Err(invalid_params(error)),
        }
        Ok(output_with_meta(
            serde_json::json!({
                "run_id": run_id,
                "status": "cancellation_requested",
                "cancellation_effect": "best_effort_drop_provider_future",
                "provider_request_may_still_be_in_flight": true,
                "effective_at": "provider_completion_or_cancellation_observation"
            }),
            None,
            None,
            Some(&run_id),
        ))
    }

    #[tool(description = "Get current run status, budget usage, and pending approvals.")]
    fn graph_run_get(
        &self,
        Parameters(RunGetParams { run_id }): Parameters<RunGetParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        if let Some(r) = runs.get(&run_id) {
            return Ok(output_with_meta(r.public(), None, None, Some(&run_id)));
        }
        drop(runs);
        if let Some(record) = self.stored_run(&run_id)? {
            return Ok(output_with_meta(record, None, None, Some(&run_id)));
        }
        Err(invalid_params(format!("run '{run_id}' not found")))
    }

    #[tool(
        description = "Read the in-memory state projection from a live run; use graph_run_checkpoint for a durable checkpoint state."
    )]
    fn graph_run_state(
        &self,
        Parameters(RunStateParams {
            run_id,
            checkpoint_id: _,
            json_pointer,
        }): Parameters<RunStateParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let r = runs
            .get(&run_id)
            .ok_or_else(|| invalid_params(format!("run '{run_id}' not found")))?;
        let state = if let Some(pointer) = json_pointer.as_deref() {
            if pointer.is_empty() {
                r.state.clone()
            } else {
                r.state.pointer(pointer).cloned().unwrap_or(Value::Null)
            }
        } else {
            r.state.clone()
        };
        Ok(output_with_meta(
            serde_json::json!({
                "state": state,
                "run_id": run_id,
                "status": r.status,
            }),
            None,
            None,
            Some(&run_id),
        ))
    }

    #[tool(
        description = "Read bounded events. With SQLite, terminal emitted events remain available as a persisted projection after restart; this is not replayable execution or resume support."
    )]
    fn graph_run_events(
        &self,
        Parameters(RunEventsParams {
            run_id,
            cursor,
            limit,
        }): Parameters<RunEventsParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let runs = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let result = runs
            .events(
                self.store.as_ref(),
                &run_id,
                cursor.unwrap_or(0),
                limit.unwrap_or(100) as usize,
            )
            .map_err(|e| invalid_params(e))?;
        Ok(output_with_meta(result, None, None, Some(&run_id)))
    }

    #[tool(description = "Fetch the canonical execution receipt for a run.")]
    fn graph_run_receipt(
        &self,
        Parameters(RunReceiptParams { run_id }): Parameters<RunReceiptParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        if let Some(r) = self
            .runs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?
            .get(&run_id)
        {
            // Canonical wrapper: identical shape to the durable read-back, so
            // consumers always read data.receipt. The HMAC receipt_digest is a
            // persistence artifact, so it is null until the terminal projection
            // is durably stored; storage_class marks the live-resident path.
            return Ok(output_with_meta(
                serde_json::json!({
                    "receipt": r.receipt.clone(),
                    "receipt_digest": Value::Null,
                    "storage_class": "volatile_live",
                    "replay_capability": r
                        .receipt
                        .get("replay_capability")
                        .and_then(Value::as_str)
                        .unwrap_or("integrity_only"),
                }),
                None,
                None,
                Some(&run_id),
            ));
        }
        if let Some(store) = &self.store {
            match store.load_terminal_receipt(&run_id) {
                Ok(Some(receipt)) => {
                    return Ok(output_with_meta(receipt, None, None, Some(&run_id)));
                }
                Ok(None) => {}
                Err(error) if error == "RECEIPT_INTEGRITY_FAILURE" => {
                    return Ok(error_output(
                        "terminal receipt integrity validation failed",
                        "RECEIPT_INTEGRITY_FAILURE",
                    ));
                }
                Err(error) if error == "INTEGRITY_KEY_REQUIRED" => {
                    return Ok(error_output(
                        "an external integrity key is required for terminal receipt reads",
                        "INTEGRITY_KEY_REQUIRED",
                    ));
                }
                Err(error) => return Err(internal_error(error)),
            }
        }
        Ok(error_output(
            format!("run '{run_id}' not found"),
            "RUN_NOT_FOUND",
        ))
    }

    // ── Policy + render ───────────────────────────────────────────────

    #[tool(description = "Preflight a graph against policy before execution.")]
    fn graph_policy_check(
        &self,
        Parameters(PolicyCheckParams { graph_id, input: _ }): Parameters<PolicyCheckParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let graphs = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let g = graphs
            .get(&graph_id)
            .ok_or_else(|| invalid_params(format!("graph '{graph_id}' not found")))?;

        let node_count = g.spec.nodes.len();
        let edge_count = g.spec.edges.len();
        let issues: Vec<String> = Vec::new();

        Ok(structured_output(serde_json::json!({
            "graph_id": graph_id,
            "passed": issues.is_empty(),
            "issues": issues,
            "stats": {
                "node_count": node_count,
                "edge_count": edge_count,
                "max_iterations": g.spec.max_iterations,
                "max_parallelism": g.spec.max_parallelism,
            },
            "capabilities": {
                "models": [self.default_model.clone()],
                "tools": [],
            }
        })))
    }

    #[tool(description = "Render a graph as Mermaid diagram or JSON topology.")]
    fn graph_render(
        &self,
        Parameters(RenderParams { graph_id, format }): Parameters<RenderParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        let graphs = self
            .graphs
            .lock()
            .map_err(|e| internal_error(e.to_string()))?;
        let g = graphs
            .get(&graph_id)
            .ok_or_else(|| invalid_params(format!("graph '{graph_id}' not found")))?;
        let fmt = format.as_deref().unwrap_or("mermaid");

        match fmt {
            "json" => Ok(output_with_meta(
                serde_json::json!({
                    "name": graph_id,
                    "nodes": g.spec.nodes.iter().map(|n| serde_json::json!({
                        "id": n.id, "type": n.node_type
                    })).collect::<Vec<_>>(),
                    "edges": g.spec.edges.iter().map(|e| serde_json::json!({
                        "from": e.from, "to": e.to
                    })).collect::<Vec<_>>(),
                }),
                Some(&graph_id),
                Some(&g.version),
                None,
            )),
            _ => Ok(output_with_meta(
                serde_json::json!({
                    "mermaid": Self::mermaid(&g.spec),
                    "name": graph_id,
                }),
                Some(&graph_id),
                Some(&g.version),
                None,
            )),
        }
    }

    // ── Templates ─────────────────────────────────────────────────────

    #[tool(description = "List available built-in graph templates.")]
    fn graph_template_list(
        &self,
        Parameters(TemplateListParams { query: _ }): Parameters<TemplateListParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        Ok(structured_output(templates::list()))
    }

    #[tool(
        description = "Instantiate a template into a graph spec that can be passed to graph_create."
    )]
    fn graph_template_instantiate(
        &self,
        Parameters(TemplateInstantiateParams { template_id, name }): Parameters<
            TemplateInstantiateParams,
        >,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        match templates::instantiate(&template_id, &name) {
            Ok(spec) => Ok(structured_output(serde_json::json!({
                "template_id": template_id,
                "name": name,
                "spec": spec,
            }))),
            Err(e) => Ok(error_output(e, "GRAPH_INVALID")),
        }
    }
    #[tool(description = "Read-only list of template promotion candidates.")]
    fn graph_template_candidates(
        &self,
        Parameters(TemplateCandidatesParams { state: _ }): Parameters<TemplateCandidatesParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        Ok(structured_output(serde_json::json!({ "candidates": [] })))
    }

    #[tool(description = "Read-only list of recorded outcomes for a template.")]
    fn graph_template_outcomes(
        &self,
        Parameters(TemplateOutcomesParams { template_id }): Parameters<TemplateOutcomesParams>,
    ) -> Result<Json<StructuredOutput>, ErrorData> {
        Ok(structured_output(serde_json::json!({
            "template_id": template_id,
            "outcomes": [],
        })))
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "agent-graph-mcp",
    version = "0.2.0",
    instructions = "Graph orchestration for bounded multi-step LLM workflows with parallel fan-out, conditional routing, state transforms, joins, cooperative cancellation, and optional enforced max_wall_clock_ms/max_nodes/max_llm_calls run budgets (max_llm_calls reserves each provider attempt before invocation; failed or timed-out attempts still count). Parallel unordered state writes require an explicit reducer. Cancellation can drop the local provider future on request, best effort; an underlying provider request may continue. Optional SQLite stores terminal projections plus explicit pre-execution checkpoints. Durable checkpoints, approvals, terminal receipts, and source witnesses require an external key file named by AGENT_GRAPH_INTEGRITY_KEY_PATH; without it their operations fail closed with INTEGRITY_KEY_REQUIRED. Deterministic local resume is limited to linear passthrough/state_transform chains and is never generic replay; uncheckpointed or ineligible runs remain interrupted_non_resumable after restart. SQLite-backed approvals can decide only an immutable deterministic-local checkpoint and resume that checkpoint; HumanApproval nodes and arbitrary external actions remain unsupported. Source witnesses are caller-supplied local captures: locators are never fetched, HMAC-authenticated witness integrity and bounded evidence spans are checked against SQLite, and source authority is not independently verified. Receipts provide integrity_only except a successfully resumed deterministic-local path, which reports deterministic_local_resume. Define graphs with graph_create, execute with graph_execute or graph_run_start, checkpoint with checkpoint:true, inspect with graph_run_get/wait/cancel/state/events/receipt/checkpoint, request or decide checkpoint approvals with graph_approval_request/decide, and resume with graph_run_resume."
)]
impl ServerHandler for AgentGraphServer {}
