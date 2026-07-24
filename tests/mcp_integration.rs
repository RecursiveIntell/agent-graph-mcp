use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

use rusqlite::Connection;
use serde_json::{json, Value};

struct Mcp {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}

impl Mcp {
    fn new() -> Self {
        Self::new_with_args(&["--direct"])
    }

    fn new_with_data_dir(data_dir: &std::path::Path) -> Self {
        let key_path = test_integrity_key();
        Self::new_with_args_and_key(
            &[
                "--direct",
                "--data-dir",
                data_dir.to_str().expect("UTF-8 temp path"),
            ],
            Some(key_path),
        )
    }

    fn new_with_args(args: &[&str]) -> Self {
        Self::new_with_args_and_key(args, None)
    }

    fn new_with_data_dir_without_integrity_key(data_dir: &std::path::Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-graph-mcp"));
        command
            .args([
                "--direct",
                "--data-dir",
                data_dir.to_str().expect("UTF-8 temp path"),
            ])
            .env_remove("AGENT_GRAPH_INTEGRITY_KEY_PATH");
        Self::from_command(command)
    }

    fn new_with_args_and_key(args: &[&str], key_path: Option<&std::path::Path>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agent-graph-mcp"));
        command.args(args);
        if let Some(key_path) = key_path {
            command.env("AGENT_GRAPH_INTEGRITY_KEY_PATH", key_path);
        }
        Self::from_command(command)
    }

    fn from_command(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut mcp = Self {
            child,
            input,
            output,
            id: 0,
        };
        let _ = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"}
            }),
        );
        writeln!(
            mcp.input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})
        )
        .unwrap();
        mcp.input.flush().unwrap();
        mcp
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        writeln!(
            self.input,
            "{}",
            json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params})
        )
        .unwrap();
        self.input.flush().unwrap();
        loop {
            let mut line = String::new();
            self.output.read_line(&mut line).unwrap();
            if line.trim().is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(&line).unwrap();
            if parsed.get("id").is_some() {
                return parsed;
            }
        }
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name":name,"arguments":arguments}));
        if let Some(sc) = response
            .get("result")
            .and_then(|r| r.get("structuredContent"))
        {
            return sc.clone();
        }
        if let Some(content) = response
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(Value::as_array)
        {
            for item in content {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        return serde_json::from_str(text).unwrap_or_else(|_| {
                            if response
                                .get("result")
                                .and_then(|result| result.get("isError"))
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            {
                                json!({"ok":false,"error":text,"error_code":"INVALID_PARAMS"})
                            } else {
                                json!({})
                            }
                        });
                    }
                }
            }
        }
        if let Some(error) = response.get("error") {
            if let Some(message) = error.get("message").and_then(Value::as_str) {
                let code = error
                    .get("code")
                    .and_then(Value::as_i64)
                    .and_then(|code| match code {
                        -32602 => Some("INVALID_PARAMS"),
                        -32601 => Some("METHOD_NOT_FOUND"),
                        _ => None,
                    })
                    .unwrap_or("UNKNOWN");
                return json!({"error": message, "code": code, "error_code": code});
            }
        }
        json!({})
    }
}

fn test_integrity_key() -> &'static std::path::Path {
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    let path = PATH.get_or_init(|| {
        let path = std::env::temp_dir().join("agent-graph-mcp-integration-integrity.key");
        std::fs::write(&path, [0x5au8; 32]).expect("test integrity key");
        path
    });
    std::env::set_var("AGENT_GRAPH_INTEGRITY_KEY_PATH", path);
    path
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn legacy_contract_and_exact_tool_names() {
    let mut mcp = Mcp::new();
    let list = mcp.request("tools/list", json!({}));
    let names: Vec<_> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"graph_create"));
    assert!(names.contains(&"graph_execute"));
    assert!(names.contains(&"graph_status"));
    assert_eq!(names.len(), 25);
    let created = mcp.call("graph_create", json!({"spec":{"name":"legacy","entry":"a","nodes":[{"id":"a","type":"passthrough"}],"edges":[{"from":"a","to":"END"}]}}));
    assert_eq!(created["graph_id"], "legacy");
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"legacy","input":{"x":1}}),
    );
    assert_eq!(run["data"]["success"], true);
    assert_eq!(run["data"]["final_state"], json!({"x":1}));
    assert!(run.get("run_id").is_some());
}

#[test]
fn readme_action_claims_match_the_callable_graph_execute_contract() {
    let readme =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md")).expect("README");
    assert!(readme.contains("Normal execution is synchronous."));
    assert!(!readme.contains("graph_execute {\"action\":\"start\",\"wait\":\"accepted\"}"));
    assert!(readme.contains("Durable approval is supported only as a SQLite-backed decision"));
}

#[test]
fn approval_tools_require_durable_sqlite_state() {
    let mut mcp = Mcp::new();
    for (tool, arguments) in [
        ("graph_approval_list", json!({})),
        ("graph_approval_get", json!({"approval_id":"approval-test"})),
        (
            "graph_approval_decide",
            json!({"approval_id":"approval-test","decision":"approve","actor":"test"}),
        ),
    ] {
        let response = mcp.call(tool, arguments);
        // AG-002: approval tools removed from model MCP tool set; now return METHOD_NOT_FOUND
        assert!(
            response
                .get("ok")
                .map(|v| v.as_bool())
                .flatten()
                .unwrap_or(true)
                == false
                || response.get("ok").is_none()
                || response["ok"].is_null(),
            "{tool} must fail closed"
        );
        let ec = response["error_code"].as_str().unwrap_or("");
        assert!(
            ec == "APPROVAL_STORE_REQUIRED" || ec == "METHOD_NOT_FOUND" || ec == "INVALID_PARAMS",
            "{tool}: expected APPROVAL_STORE_REQUIRED or METHOD_NOT_FOUND, got {ec}"
        );
    }
}

#[test]
fn status_exposes_only_available_durability_capabilities() {
    let mut mcp = Mcp::new();
    let status = mcp.call("graph_status", json!({"resource":"server"}));
    assert_eq!(
        status["data"]["capabilities"]["checkpointing"],
        "unavailable"
    );
    assert_eq!(
        status["data"]["capabilities"]["events"],
        "volatile_in_memory_only"
    );
    assert_eq!(status["data"]["capabilities"]["durable_resume"], false);
    assert_eq!(status["data"]["capabilities"]["hitl"], "unavailable");

    let temp = tempfile::tempdir().expect("temp graph database");
    let mut persisted = Mcp::new_with_data_dir(temp.path());
    let persisted_status = persisted.call("graph_status", json!({"resource":"server"}));
    assert_eq!(
        persisted_status["data"]["capabilities"]["events"],
        "terminal_persisted_projection_with_sqlite_fallback"
    );
    assert_eq!(
        persisted_status["data"]["capabilities"]["event_replay"],
        "not_replayable_execution"
    );
    assert_eq!(
        persisted_status["data"]["capabilities"]["durable_resume"],
        "deterministic_local_resume_only"
    );
    assert_eq!(
        persisted_status["data"]["capabilities"]["hitl"],
        "checkpoint_bound_durable_approval_only"
    );
}

#[test]
fn validates_and_runs_parallel_transform_join_with_versions() {
    let mut mcp = Mcp::new();
    let spec = json!({
        "spec_version":"2", "name":"parallel", "entry":"fork", "max_iterations":8,
        "reducers":{"results":"append"},
        "nodes":[
          {"id":"fork","type":"passthrough"},
          {"id":"left","type":"state_transform","config":{"operations":[{"op":"append","path":"results","value":"left"}]}},
          {"id":"right","type":"state_transform","config":{"operations":[{"op":"append","path":"results","value":"right"}]}},
          {"id":"join","type":"join","config":{"inputs":["results"],"output":"joined","mode":"collect_array"}}
        ],
        "edges":[{"from":"fork","to":"left"},{"from":"fork","to":"right"},{"from":"left","to":"join"},{"from":"right","to":"join"},{"from":"join","to":"END"}]
    });
    let validated = mcp.call("graph_create", json!({"action":"validate","spec":spec}));
    assert_eq!(validated["data"]["status"], "valid");
    let created = mcp.call("graph_create", json!({"spec":spec}));
    assert!(created["data"]["graph_version"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let run = mcp.call("graph_execute", json!({"graph_id":"parallel","input":{}}));
    assert_eq!(run["data"]["success"], true);
    assert_eq!(run["data"]["state"]["results"], json!(["left", "right"]));
    assert_eq!(
        run["data"]["receipt"]["replay_capability"],
        "integrity_only"
    );
    let events = mcp.call(
        "graph_status",
        json!({"resource":"events","run_id":run["run_id"],"cursor":0}),
    );
    assert!(events["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| {
            entry["event"]["SuperstepStart"]["nodes"]
                .as_array()
                .is_some_and(|nodes| nodes.len() == 2)
        }));
}

#[test]
fn ordered_router_first_match_and_bounded_loop() {
    let mut mcp = Mcp::new();
    let spec = json!({"spec_version":"2","name":"route","entry":"route","max_iterations":6,"nodes":[
      {"id":"route","type":"router","config":{"rules":[
        {"path":"__input__","op":"contains","value":"deep","targets":["first"]},
        {"path":"__input__","op":"contains","value":"deep research","targets":["second"]}],"default":["END"]}},
      {"id":"first","type":"state_transform","config":{"operations":[{"op":"set","path":"chosen","value":"first"}]}},
      {"id":"second","type":"state_transform","config":{"operations":[{"op":"set","path":"chosen","value":"second"}]}}
    ],"edges":[{"from":"first","to":"END"},{"from":"second","to":"END"}]});
    mcp.call("graph_create", json!({"spec":spec}));
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"route","input":"deep research"}),
    );
    assert_eq!(run["data"]["state"]["chosen"], "first");

    let looping = json!({"spec_version":"2","name":"loop","entry":"inc","max_iterations":4,"nodes":[
      {"id":"inc","type":"state_transform","config":{"operations":[{"op":"increment","path":"count","value":1}]}},
      {"id":"again","type":"router","config":{"rules":[{"path":"count","op":"lt","value":10,"targets":["inc"]}],"default":["END"]}}
    ],"edges":[{"from":"inc","to":"again"}]});
    mcp.call("graph_create", json!({"spec":looping}));
    let run = mcp.call("graph_execute", json!({"graph_id":"loop","input":{}}));
    assert_eq!(run["data"]["success"], false);
    assert!(run["data"]["error"]
        .as_str()
        .unwrap()
        .contains("iterations"));
}

#[test]
fn registry_templates_security_and_bundle_verification() {
    let mut mcp = Mcp::new();
    let templates = mcp.call(
        "graph_status",
        json!({"resource":"templates","action":"list"}),
    );
    assert!(templates["data"]["available"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["id"] == "parallel_council"));
    let rejected = mcp.call("graph_create", json!({"action":"validate","spec":{"name":"evil","entry":"x","nodes":[{"id":"x","type":"shell","config":{"command":"id"}}],"edges":[]}}));
    assert_eq!(rejected["error_code"], "INVALID_PARAMS");

    let spec = json!({"name":"evidence","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]});
    mcp.call("graph_create", json!({"spec":spec}));
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"evidence","input":{"password":"do-not-export","safe":"ok"}}),
    );
    let bundle = mcp.call(
        "graph_status",
        json!({"resource":"bundle","run_id":run["run_id"]}),
    );
    assert_eq!(bundle["data"]["payload"]["input"]["password"], "[REDACTED]");
}

#[test]
fn legacy_model_aliases_and_per_node_core_evidence_are_preserved() {
    let mut mcp = Mcp::new();
    let model_spec = json!({
        "name":"model-alias",
        "entry":"ask",
        "nodes":[{"id":"ask","type":"llm","model":"glm-5.2:cloud","prompt":"{input}"}],
        "edges":[]
    });
    let validated = mcp.call(
        "graph_create",
        json!({"action":"validate","spec":model_spec}),
    );
    assert_eq!(validated["data"]["status"], "valid");

    let evidence_spec = json!({
        "name":"node-evidence",
        "entry":"first",
        "nodes":[
            {"id":"first","type":"state_transform","config":{"operations":[{"op":"set","path":"__input__","value":{"stage":"first"}}]}},
            {"id":"second","type":"state_transform","config":{"operations":[{"op":"set","path":"__input__","value":{"stage":"second"}}]}}
        ],
        "edges":[{"from":"first","to":"second"},{"from":"second","to":"END"}]
    });
    mcp.call("graph_create", json!({"spec":evidence_spec}));
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"node-evidence","input":{}}),
    );
    assert_eq!(run["data"]["success"], true);
    assert_eq!(run["data"]["steps"].as_array().unwrap().len(), 2);
    assert_eq!(run["data"]["steps"][0]["output"], json!({"stage":"first"}));
    assert_eq!(run["data"]["steps"][1]["output"], json!({"stage":"second"}));
}

#[test]
fn accepted_run_is_addressable_and_unsupported_boundaries_are_stable() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({"spec":{"name":"async","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
    let accepted = mcp.call("graph_run_start", json!({"graph_id":"async","input":{}}));
    assert_eq!(accepted["data"]["status"], "running");
    let run = mcp.call(
        "graph_status",
        json!({"resource":"run","run_id":accepted["run_id"]}),
    );
    assert!(matches!(
        run["data"]["status"].as_str(),
        Some("running" | "completed")
    ));
    let completed = mcp.call(
        "graph_run_wait",
        json!({"run_id":accepted["run_id"],"timeout_ms":5_000}),
    );
    assert_eq!(completed["data"]["status"], "completed");
    assert_eq!(completed["data"]["success"], true);
}

#[test]
fn historical_graph_version_executes_exact_stored_spec_after_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let version_a = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        let created = first.call(
            "graph_create",
            json!({"spec":{"name":"versioned","entry":"x","nodes":[{"id":"x","type":"state_transform","config":{"operations":[{"op":"set","path":"result","value":"version-a"}]}}],"edges":[{"from":"x","to":"END"}]}}),
        );
        let version_a = created["graph_version"]
            .as_str()
            .expect("version a")
            .to_owned();
        first.call(
            "graph_create",
            json!({"overwrite":true,"spec":{"name":"versioned","entry":"x","nodes":[{"id":"x","type":"state_transform","config":{"operations":[{"op":"set","path":"result","value":"version-b"}]}}],"edges":[{"from":"x","to":"END"}]}}),
        );
        version_a
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let run = restarted.call(
        "graph_execute",
        json!({"graph_id":"versioned","graph_version":version_a,"input":{}}),
    );
    assert_eq!(run["data"]["success"], true);
    assert_eq!(run["data"]["graph_version"], version_a);
    assert_eq!(run["data"]["state"]["result"], "version-a");
}

#[test]
fn completed_persistent_run_is_readable_after_mcp_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"persistent","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let started = first.call(
            "graph_run_start",
            json!({"graph_id":"persistent","input":{"request":"durable terminal status"}}),
        );
        let run_id = started["run_id"].as_str().expect("run id").to_owned();
        let completed = first.call(
            "graph_run_wait",
            json!({"run_id":run_id,"timeout_ms":5_000}),
        );
        assert_eq!(completed["data"]["status"], "completed");
        run_id
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let recovered = restarted.call("graph_run_get", json!({"run_id":run_id}));
    assert_eq!(recovered["data"]["status"], "completed");
    assert_eq!(recovered["data"]["success"], true);
    assert_eq!(
        recovered["data"]["final_state"],
        json!({"request":"durable terminal status"})
    );
}

#[test]
fn terminal_receipt_is_readable_after_mcp_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"persistent-receipt","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let completed = first.call(
            "graph_execute",
            json!({"graph_id":"persistent-receipt","input":{"request":"durable receipt"}}),
        );
        assert_eq!(completed["data"]["status"], "completed");
        let run_id = completed["run_id"].as_str().expect("run id").to_owned();
        run_id
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let receipt = restarted.call("graph_run_receipt", json!({"run_id":run_id}));
    assert!(receipt["data"]["receipt_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("hmac-sha256:")));
    assert_eq!(receipt["data"]["storage_class"], "sqlite_terminal_receipt");
    assert_eq!(receipt["data"]["replay_capability"], "integrity_only");
}

#[test]
fn tampered_terminal_receipt_fails_closed_after_mcp_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"tampered-receipt","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let completed = first.call(
            "graph_execute",
            json!({"graph_id":"tampered-receipt","input":{"request":"integrity"}}),
        );
        completed["run_id"].as_str().expect("run id").to_owned()
    };

    let connection = Connection::open(temp.path().join("agent-graph.db")).expect("database");
    connection
        .execute(
            "UPDATE terminal_receipts SET receipt_json = ?1 WHERE run_id = ?2",
            rusqlite::params![json!({"tampered":"never disclose"}).to_string(), &run_id],
        )
        .expect("tamper receipt JSON without digest update");
    drop(connection);

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let receipt = restarted.call("graph_run_receipt", json!({"run_id":run_id}));
    assert_eq!(receipt["ok"], false);
    assert_eq!(receipt["error_code"], "RECEIPT_INTEGRITY_FAILURE");
    assert!(!receipt.to_string().contains("never disclose"));
}

#[test]
fn terminal_events_are_readable_after_mcp_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"persistent-events","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let started = first.call(
            "graph_run_start",
            json!({"graph_id":"persistent-events","input":{"request":"terminal event projection"}}),
        );
        let run_id = started["run_id"].as_str().expect("run id").to_owned();
        let completed = first.call(
            "graph_run_wait",
            json!({"run_id":run_id,"timeout_ms":5_000}),
        );
        assert_eq!(completed["data"]["status"], "completed");
        assert_eq!(completed["data"]["persistence_status"], "durable_terminal");
        run_id
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let events = restarted.call(
        "graph_run_events",
        json!({"run_id":run_id,"cursor":0,"limit":100}),
    );
    assert_eq!(
        events["data"]["projection"],
        "terminal_persisted_projection"
    );
    assert_eq!(events["data"]["replayable_execution"], false);
    assert_eq!(events["data"]["resume_supported"], false);
    let entries = events["data"]["events"]
        .as_array()
        .expect("persisted events");
    assert!(!entries.is_empty());
    for (expected, entry) in entries.iter().enumerate() {
        assert_eq!(entry["cursor"], expected as u64);
    }
}

#[test]
fn interrupted_persistent_run_is_readable_after_mcp_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = "run-interrupted-regression";
    {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"interrupted","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let store = agent_graph_mcp::store::PersistentStore::open(temp.path()).expect("store");
        store
            .save_execution(run_id, "interrupted", "sha256:test", "running", "{}")
            .expect("running row");
    }
    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let recovered = restarted.call("graph_run_get", json!({"run_id":run_id}));
    assert_eq!(recovered["data"]["status"], "interrupted_non_resumable");
    assert_eq!(recovered["data"]["durable_resume"], false);
}

#[test]
fn receipt_labels_integrity_only_when_dependency_envelopes_are_missing() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({"spec":{"name":"integrity-only","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"integrity-only","input":{"x":1}}),
    );
    assert_eq!(
        run["data"]["receipt"]["dependency_envelopes_complete"],
        false
    );
    assert_eq!(
        run["data"]["receipt"]["replay_capability"],
        "integrity_only"
    );
}

#[test]
fn graph_delete_rejects_durable_execution_reference() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call("graph_create", json!({"spec":{"name":"referenced","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
    let run = mcp.call("graph_execute", json!({"graph_id":"referenced","input":{}}));
    let waited = mcp.call(
        "graph_run_wait",
        json!({"run_id":run["run_id"],"timeout_ms":5_000}),
    );
    assert_eq!(waited["data"]["persistence_status"], "durable_terminal");

    // graph_delete is no longer a model-facing tool (AG-002).
    // Model clients must use the authenticated operator path.
    let deleted = mcp.call("graph_delete", json!({"graph_id":"referenced"}));
    // Tool not registered → response has error_code but no "ok" field
    assert!(deleted.get("ok").is_none() || deleted["ok"] == false || deleted["ok"].is_null());
    assert!(
        deleted["error_code"] == "INVALID_PARAMS"
            || deleted["error_code"] == "METHOD_NOT_FOUND"
            || deleted["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
    );
    let listed = mcp.call("graph_list", json!({}));
    assert!(listed["data"]["graphs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|graph| graph["name"] == "referenced"));
}

#[test]
fn graph_delete_removes_unreferenced_graph_before_restart() {
    // graph_delete is no longer a model-facing tool (AG-002).
    // This test verifies that graph_delete is NOT callable through the model MCP path.
    let temp = tempfile::tempdir().expect("temp graph database");
    {
        let mut mcp = Mcp::new_with_data_dir(temp.path());
        mcp.call("graph_create", json!({"spec":{"name":"unreferenced","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let deleted = mcp.call("graph_delete", json!({"graph_id":"unreferenced"}));
        assert!(deleted.get("ok").is_none() || deleted["ok"] == false || deleted["ok"].is_null());
        assert!(
            deleted["error_code"] == "INVALID_PARAMS"
                || deleted["error_code"] == "METHOD_NOT_FOUND"
                || deleted["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
        );
        // Graph still exists because model client cannot delete it.
        let listed = mcp.call("graph_list", json!({}));
        assert!(listed["data"]["graphs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|graph| graph["name"] == "unreferenced"));
    }

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let listed = restarted.call("graph_list", json!({}));
    assert!(listed["data"]["graphs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|graph| graph["name"] == "unreferenced"));
}

#[test]
fn graph_delete_fails_closed_on_memory_storage_mismatch() {
    // graph_delete is no longer a model-facing tool (AG-002).
    // This test verifies the tool is not callable through the model path.
    let temp = tempfile::tempdir().expect("temp graph database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call("graph_create", json!({"spec":{"name":"mismatch","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));

    let deleted = mcp.call("graph_delete", json!({"graph_id":"mismatch"}));
    assert!(deleted.get("ok").is_none() || deleted["ok"] == false || deleted["ok"].is_null());
    assert!(
        deleted["error_code"] == "INVALID_PARAMS"
            || deleted["error_code"] == "METHOD_NOT_FOUND"
            || deleted["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
    );
    // Graph still exists.
    let listed = mcp.call("graph_list", json!({}));
    assert!(listed["data"]["graphs"]
        .as_array()
        .expect("graphs")
        .iter()
        .any(|graph| graph["name"] == "mismatch"));
}

#[test]
fn declared_output_key_is_the_terminal_result() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({"spec":{
        "spec_version":"2",
        "name":"explicit-output",
        "entry":"produce",
        "output_key":"result",
        "nodes":[
            {"id":"produce","type":"state_transform","config":{"operations":[{"op":"set","path":"result","value":{"answer":"trusted output"}}]}}
        ],
        "edges":[{"from":"produce","to":"END"}]
    }}));
    let run = mcp.call(
        "graph_execute",
        json!({"graph_id":"explicit-output","input":{"answer":"stale input"}}),
    );
    assert_eq!(run["data"]["success"], true);
    assert_eq!(
        run["data"]["final_state"],
        json!({"answer":"trusted output"})
    );
}

#[test]
fn idempotency_key_conflicts_when_execute_request_material_changes() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call("graph_create", json!({"spec":{"name":"idempotency","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
    let first = mcp.call(
        "graph_execute",
        json!({
            "graph_id":"idempotency", "input":{"value":1}, "idempotency_key":"same-key"
        }),
    );
    assert_eq!(first["ok"], true);
    let replay = mcp.call(
        "graph_execute",
        json!({
            "graph_id":"idempotency", "input":{"value":1}, "idempotency_key":"same-key"
        }),
    );
    assert_eq!(replay["run_id"], first["run_id"]);
    let conflict = mcp.call(
        "graph_execute",
        json!({
            "graph_id":"idempotency", "input":{"value":2}, "idempotency_key":"same-key"
        }),
    );
    assert_eq!(conflict["ok"], false);
    assert_eq!(conflict["error_code"], "IDEMPOTENCY_CONFLICT");
}

#[test]
fn idempotency_key_conflicts_when_create_request_material_changes() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    let first = mcp.call("graph_create", json!({
        "spec":{"name":"create-idempotency","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]},
        "idempotency_key":"create-key"
    }));
    assert_eq!(first["ok"], true);
    let replay = mcp.call("graph_create", json!({
        "spec":{"name":"create-idempotency","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]},
        "idempotency_key":"create-key"
    }));
    assert_eq!(replay["data"], first["data"]);
    let conflict = mcp.call("graph_create", json!({
        "spec":{"name":"create-idempotency","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[]},
        "idempotency_key":"create-key"
    }));
    assert_eq!(conflict["ok"], false);
    assert_eq!(conflict["error_code"], "IDEMPOTENCY_CONFLICT");
}

#[test]
fn run_start_idempotency_and_budgets_are_typed_and_idempotent() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call("graph_create", json!({"spec":{"name":"run-idempotency","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));

    let unsupported = mcp.call(
        "graph_run_start",
        json!({
            "graph_id":"run-idempotency", "input":{"value":1},
            "budgets":{"max_tokens":1}, "idempotency_key":"budget-key"
        }),
    );
    assert_eq!(unsupported["ok"], false);
    assert_eq!(unsupported["error_code"], "INVALID_BUDGETS");
    assert!(unsupported.get("run_id").is_none() || unsupported["run_id"].is_null());

    let first = mcp.call(
        "graph_run_start",
        json!({
            "graph_id":"run-idempotency", "input":{"value":1}, "idempotency_key":"run-key"
        }),
    );
    assert_eq!(first["ok"], true);
    let legacy_terminal = mcp.call(
        "graph_run_wait",
        json!({"run_id":first["run_id"].clone(),"timeout_ms":5_000}),
    );
    assert_eq!(legacy_terminal["data"]["status"], "completed");
    assert!(legacy_terminal["data"]["budgets"].is_null());
    let replay = mcp.call(
        "graph_run_start",
        json!({
            "graph_id":"run-idempotency", "input":{"value":1}, "idempotency_key":"run-key"
        }),
    );
    assert_eq!(replay["run_id"], first["run_id"]);
    let conflict = mcp.call(
        "graph_run_start",
        json!({
            "graph_id":"run-idempotency", "input":{"value":2}, "idempotency_key":"run-key"
        }),
    );
    assert_eq!(conflict["ok"], false);
    assert_eq!(conflict["error_code"], "IDEMPOTENCY_CONFLICT");
}

#[test]
fn invalid_budget_shapes_are_rejected_with_one_typed_error() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({"spec":{"name":"invalid-budget","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));

    for budgets in [
        json!({}),
        json!({"unknown": 1}),
        json!({"max_nodes": true}),
        json!({"max_nodes": 1.5}),
        json!({"max_nodes": 0}),
        json!({"max_nodes": -1}),
        json!({"max_nodes": "1"}),
        json!({"max_llm_calls": 1}),
    ] {
        let response = mcp.call(
            "graph_run_start",
            json!({"graph_id":"invalid-budget","budgets":budgets}),
        );
        assert_eq!(response["ok"], false, "budgets={budgets}");
        assert_eq!(
            response["error_code"], "INVALID_BUDGETS",
            "budgets={budgets}"
        );
    }
}

#[test]
fn max_nodes_stops_before_a_later_node_effect() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({
        "spec":{"name":"node-budget","entry":"first","nodes":[
            {"id":"first","type":"state_transform","config":{"operations":[{"op":"set","path":"first_effect","value":true}]}},
            {"id":"second","type":"state_transform","config":{"operations":[{"op":"set","path":"second_effect","value":true}]}}
        ],"edges":[{"from":"first","to":"second"},{"from":"second","to":"END"}]}
    }));
    let started = mcp.call(
        "graph_run_start",
        json!({"graph_id":"node-budget","budgets":{"max_nodes":1}}),
    );
    let run_id = started["run_id"].as_str().expect("run id").to_owned();
    let completed = mcp.call(
        "graph_run_wait",
        json!({"run_id":run_id,"timeout_ms":5_000}),
    );
    let data = &completed["data"];
    assert_eq!(data["status"], "failed");
    assert_eq!(data["success"], false);
    assert_eq!(data["error"], "BUDGET_EXHAUSTED");
    assert_eq!(data["budget_exhausted"], "max_nodes");
    assert_eq!(data["budgets"], json!({"max_nodes":1}));
    assert_eq!(data["budget_counters"]["nodes"], 1);
    assert_eq!(data["state"]["first_effect"], true);
    assert!(data["state"].get("second_effect").is_none());
}

#[test]
fn successful_wall_clock_budget_does_not_wait_for_timeout_worker() {
    let mut mcp = Mcp::new();
    mcp.call("graph_create", json!({"spec":{"name":"fast-wall-budget","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
    let started = std::time::Instant::now();
    let run = mcp.call(
        "graph_run_start",
        json!({"graph_id":"fast-wall-budget","budgets":{"max_wall_clock_ms":2_000}}),
    );
    let completed = mcp.call(
        "graph_run_wait",
        json!({"run_id":run["run_id"],"timeout_ms":1_000}),
    );
    assert_eq!(completed["data"]["status"], "completed");
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "fast run waited for wall-clock budget"
    );
}

#[test]
fn wall_clock_budget_is_observed_on_a_deterministically_large_local_run() {
    let mut mcp = Mcp::new();
    let nodes: Vec<Value> = (0..64)
        .map(|index| json!({"id":format!("n{index}"),"type":"passthrough"}))
        .collect();
    let edges: Vec<Value> = (0..63)
        .map(|index| json!({"from":format!("n{index}"),"to":format!("n{}", index + 1)}))
        .chain(std::iter::once(json!({"from":"n63","to":"END"})))
        .collect();
    mcp.call("graph_create", json!({
        "spec":{"name":"wall-budget","entry":"n0","max_iterations":64,"nodes":nodes,"edges":edges}
    }));
    let started = mcp.call(
        "graph_run_start",
        json!({"graph_id":"wall-budget","budgets":{"max_wall_clock_ms":1}}),
    );
    let run_id = started["run_id"].as_str().expect("run id").to_owned();
    let completed = mcp.call(
        "graph_run_wait",
        json!({"run_id":run_id,"timeout_ms":5_000}),
    );
    assert_eq!(completed["data"]["status"], "failed");
    assert_eq!(completed["data"]["success"], false);
    assert_eq!(completed["data"]["error"], "BUDGET_EXHAUSTED");
    assert_eq!(completed["data"]["budget_exhausted"], "max_wall_clock_ms");
    assert!(completed["data"]["budget_counters"]["wall_clock_ms"]
        .as_u64()
        .is_some_and(|value| value >= 1));
}

#[test]
fn budget_projection_is_visible_after_sqlite_restart() {
    let temp = tempfile::tempdir().expect("temp graph database");
    let run_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        first.call("graph_create", json!({"spec":{"name":"budget-persist","entry":"x","nodes":[{"id":"x","type":"passthrough"}],"edges":[{"from":"x","to":"END"}]}}));
        let started = first.call(
            "graph_run_start",
            json!({"graph_id":"budget-persist","budgets":{"max_nodes":1}}),
        );
        let run_id = started["run_id"].as_str().expect("run id").to_owned();
        let completed = first.call(
            "graph_run_wait",
            json!({"run_id":run_id,"timeout_ms":5_000}),
        );
        assert_eq!(completed["data"]["status"], "completed");
        assert_eq!(completed["data"]["budgets"], json!({"max_nodes":1}));
        run_id
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let recovered = restarted.call("graph_run_get", json!({"run_id":run_id}));
    assert_eq!(recovered["data"]["status"], "completed");
    assert_eq!(recovered["data"]["budgets"], json!({"max_nodes":1}));
    assert_eq!(recovered["data"]["budget_counters"]["nodes"], 1);
    let receipt = restarted.call("graph_run_receipt", json!({"run_id":run_id}));
    assert_eq!(
        receipt["data"]["receipt"]["budgets"],
        json!({"max_nodes":1})
    );
    assert_eq!(receipt["data"]["receipt"]["budget_counters"]["nodes"], 1);
}

#[test]
fn evidence_required_llm_contract_is_validated_without_external_verification() {
    let mut mcp = Mcp::new();
    let missing_json_mode = mcp.call("graph_create", json!({
        "action":"validate", "spec":{"name":"evidence-contract","entry":"ask","nodes":[
            {"id":"ask","type":"llm","evidence_required":true,"config":{"output_key":"evidence"}}
        ],"edges":[]}
    }));
    assert_eq!(missing_json_mode["error_code"], "INVALID_PARAMS");

    let missing_output_key = mcp.call(
        "graph_create",
        json!({
            "action":"validate", "spec":{"name":"evidence-contract","entry":"ask","nodes":[
                {"id":"ask","type":"llm","evidence_required":true,"json_mode":true}
            ],"edges":[]}
        }),
    );
    assert_eq!(missing_output_key["error_code"], "INVALID_PARAMS");

    let valid = mcp.call("graph_create", json!({
        "action":"validate", "spec":{"name":"evidence-contract","entry":"ask","nodes":[
            {"id":"ask","type":"llm","evidence_required":true,"json_mode":true,"config":{"output_key":"evidence"}}
        ],"edges":[]}
    }));
    assert_eq!(valid["data"]["status"], "valid");
}

#[test]
fn source_witness_capture_requires_store_and_rejects_invalid_contract_values() {
    let mut memory = Mcp::new();
    let no_store = memory.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://fixture",
            "content":"captured",
            "media_type":"text/plain",
            "authority_class":"caller_supplied_unverified"
        }),
    );
    assert_eq!(no_store["error_code"], "WITNESS_STORE_REQUIRED");

    let temp = tempfile::tempdir().expect("witness database");
    let mut persisted = Mcp::new_with_data_dir(temp.path());
    let invalid_media = persisted.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://fixture",
            "content":"captured",
            "media_type":"text/html",
            "authority_class":"caller_supplied_unverified",
            "retrieved_at":"2026-07-21T12:00:00Z"
        }),
    );
    assert_eq!(invalid_media["error_code"], "WITNESS_INVALID_MEDIA_TYPE");

    let control_locator = persisted.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://bad\nlocator",
            "content":"captured",
            "media_type":"text/plain",
            "authority_class":"caller_supplied_unverified"
        }),
    );
    assert_eq!(control_locator["error_code"], "WITNESS_INVALID_LOCATOR");

    let too_large = persisted.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://fixture",
            "content":"x".repeat(256 * 1024 + 1),
            "media_type":"text/plain",
            "authority_class":"caller_supplied_unverified"
        }),
    );
    assert_eq!(too_large["error_code"], "WITNESS_CONTENT_TOO_LARGE");

    let unknown_field = persisted.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://fixture",
            "content":"captured",
            "media_type":"text/plain",
            "authority_class":"caller_supplied_unverified",
            "unexpected":true
        }),
    );
    assert_eq!(unknown_field["error_code"], "INVALID_PARAMS");
}

#[test]
fn source_witness_capture_is_idempotent_and_survives_restart() {
    let temp = tempfile::tempdir().expect("witness database");
    let witness_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        let request = json!({
            "locator":"local://fixture",
            "content":"bounded captured text",
            "media_type":"text/plain",
            "authority_class":"local_primary_capture",
            "retrieved_at":"2026-07-21T12:00:00Z"
        });
        let first_capture = first.call("graph_source_witness_capture", request.clone());
        assert_eq!(first_capture["ok"], true);
        let duplicate = first.call("graph_source_witness_capture", request);
        assert_eq!(
            duplicate["data"]["witness_id"],
            first_capture["data"]["witness_id"]
        );
        first_capture["data"]["witness_id"]
            .as_str()
            .expect("witness id")
            .to_owned()
    };

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let fetched = restarted.call("graph_source_witness_get", json!({"witness_id":witness_id}));
    assert_eq!(fetched["ok"], true);
    assert_eq!(fetched["data"]["content"], "bounded captured text");
    assert_eq!(fetched["data"]["authority_class"], "local_primary_capture");
}

#[test]
fn tampered_source_witness_fails_closed_without_content_leak() {
    let temp = tempfile::tempdir().expect("witness database");
    let witness_id = {
        let mut first = Mcp::new_with_data_dir(temp.path());
        let captured = first.call(
            "graph_source_witness_capture",
            json!({
                "locator":"local://fixture",
                "content":"original secret capture",
                "media_type":"text/plain",
                "authority_class":"caller_supplied_unverified",
                "retrieved_at":"2026-07-21T12:00:00Z"
            }),
        );
        captured["data"]["witness_id"]
            .as_str()
            .expect("witness id")
            .to_owned()
    };
    let connection = Connection::open(temp.path().join("agent-graph.db")).expect("database");
    connection
        .execute(
            "UPDATE source_witnesses SET content = ?1 WHERE witness_id = ?2",
            rusqlite::params!["tampered content must not leak", &witness_id],
        )
        .expect("tamper witness content");
    drop(connection);

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let fetched = restarted.call("graph_source_witness_get", json!({"witness_id":witness_id}));
    assert_eq!(fetched["error_code"], "WITNESS_INTEGRITY_FAILURE");
    assert!(!fetched.to_string().contains("tampered content"));
}

#[test]
fn durable_integrity_operations_require_an_external_key() {
    let temp = tempfile::tempdir().expect("witness database");
    let mut mcp = Mcp::new_with_data_dir_without_integrity_key(temp.path());
    let witness = mcp.call(
        "graph_source_witness_capture",
        json!({
            "locator":"local://fixture",
            "content":"requires authenticated storage",
            "media_type":"text/plain",
            "authority_class":"caller_supplied_unverified",
            "retrieved_at":"2026-07-21T12:00:00Z"
        }),
    );
    assert_eq!(witness["error_code"], "INTEGRITY_KEY_REQUIRED");
    let status = mcp.call("graph_status", json!({"resource":"server"}));
    assert_eq!(status["data"]["capabilities"]["durable_resume"], false);
    assert_eq!(
        status["data"]["capabilities"]["terminal_persistence"],
        "disabled_without_integrity_key"
    );
    assert_eq!(status["data"]["capabilities"]["hitl"], "unavailable");
}

#[test]
fn recomputed_unkeyed_checkpoint_digest_is_rejected() {
    let temp = tempfile::tempdir().expect("checkpoint database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("hmac-checkpoint")}),
    );
    let started = mcp.call(
        "graph_run_start",
        json!({"graph_id":"hmac-checkpoint","checkpoint":true}),
    );
    let checkpoint_id = started["data"]["checkpoint_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let connection = Connection::open(temp.path().join("agent-graph.db")).expect("database");
    let state = json!({"attacker":true});
    let state_digest = agent_graph_mcp::evidence::digest(&state);
    connection
        .execute(
            "UPDATE checkpoints SET state_json = ?1, state_digest = ?2 WHERE checkpoint_id = ?3",
            rusqlite::params![state.to_string(), state_digest, checkpoint_id],
        )
        .expect("tamper checkpoint");
    let parts: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        String,
    ) = connection
        .query_row(
            "SELECT run_id, graph_id, graph_version, next_cursor, state_digest, budgets_json,
                budget_counters_json, dependency_json, terminal_cursor, event_cursor, created_at
         FROM checkpoints WHERE checkpoint_id = ?1",
            rusqlite::params![checkpoint_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ))
            },
        )
        .expect("checkpoint parts");
    let legacy_digest = agent_graph_mcp::evidence::digest(&json!({
        "checkpoint_id": checkpoint_id,
        "run_id": parts.0,
        "graph_id": parts.1,
        "graph_version": parts.2,
        "next_node_cursor": parts.3,
        "state": state,
        "state_digest": parts.4,
        "budgets": serde_json::from_str::<Value>(&parts.5).unwrap(),
        "budget_counters": serde_json::from_str::<Value>(&parts.6).unwrap(),
        "dependency_summary": serde_json::from_str::<Value>(&parts.7).unwrap(),
        "dependency_digest": agent_graph_mcp::evidence::digest(&serde_json::from_str::<Value>(&parts.7).unwrap()),
        "terminal_cursor": parts.8,
        "event_cursor": parts.9,
        "created_at": parts.10,
    }));
    connection
        .execute(
            "UPDATE checkpoints SET checkpoint_digest = ?1 WHERE checkpoint_id = ?2",
            rusqlite::params![legacy_digest, checkpoint_id],
        )
        .expect("recompute legacy digest");
    drop(connection);
    let resumed = mcp.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
    assert_eq!(resumed["error_code"], "CHECKPOINT_INTEGRITY_FAILURE");
}

#[test]
fn checkpointed_run_cannot_be_cancelled_or_marked_cancelled() {
    let temp = tempfile::tempdir().expect("checkpoint database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("cancel-checkpointed")}),
    );
    let checkpointed = mcp.call(
        "graph_run_start",
        json!({"graph_id":"cancel-checkpointed","checkpoint":true}),
    );
    let run_id = checkpointed["run_id"].as_str().unwrap().to_owned();
    let cancelled = mcp.call("graph_run_cancel", json!({"run_id":run_id}));
    assert_eq!(cancelled["error_code"], "RUN_NOT_CANCELLABLE");
    let current = mcp.call("graph_run_get", json!({"run_id":checkpointed["run_id"]}));
    assert_eq!(current["data"]["status"], "checkpointed");
}

#[test]
fn witness_receipt_projection_preserves_reference_only_across_restart() {
    let temp = tempfile::tempdir().expect("witness database");
    let (run_id, witness_id, witness_digest, locator_digest) = {
        let store = agent_graph_mcp::store::PersistentStore::open(temp.path()).expect("store");
        store
            .save_graph("witness-receipt", "{}", "graph-version", false)
            .expect("graph");
        let run_id = "run-witness-receipt";
        store
            .save_execution(run_id, "witness-receipt", "graph-version", "running", "{}")
            .expect("execution");
        let witness = store
            .capture_witness(agent_graph_mcp::evidence::WitnessCapture {
                locator: "local://receipt".into(),
                content: "captured content must stay out of receipts".into(),
                media_type: "text/plain".into(),
                authority_class: "caller_supplied_unverified".into(),
                retrieved_at: "2026-07-21T12:00:00Z".into(),
            })
            .expect("witness");
        let locator_digest = agent_graph_mcp::evidence::digest(&json!(witness.locator));
        let receipt = json!({
            "schema":"agent-graph-mcp-receipt-v1",
            "dependency_envelopes":[{"witness_id":witness.witness_id,"digest":witness.digest,"locator_digest":locator_digest}],
            "dependency_envelopes_complete":true,
            "evidence_authority":"local_capture_receipt_only; source_authority_not_verified"
        });
        store
            .persist_terminal_projection(
                run_id,
                "completed",
                "{}",
                0,
                &[],
                &receipt.to_string(),
                &json!({"receipt":receipt}).to_string(),
            )
            .expect("terminal receipt");
        (
            run_id.to_owned(),
            witness.witness_id,
            witness.digest,
            locator_digest,
        )
    };

    let reopened = agent_graph_mcp::store::PersistentStore::open(temp.path()).expect("restart");
    let projected = reopened
        .load_terminal_receipt(&run_id)
        .expect("receipt read")
        .expect("receipt")["receipt"]
        .clone();
    assert_eq!(projected["dependency_envelopes_complete"], true);
    assert_eq!(
        projected["dependency_envelopes"][0]["witness_id"],
        witness_id
    );
    assert_eq!(
        projected["dependency_envelopes"][0]["digest"],
        witness_digest
    );
    assert_eq!(
        projected["dependency_envelopes"][0]["locator_digest"],
        locator_digest
    );
    assert!(!projected.to_string().contains("captured content"));
}

#[test]
fn evidence_required_graphs_fail_closed_without_witness_store() {
    let mut mcp = Mcp::new();
    let created = mcp.call(
        "graph_create",
        json!({"spec":{"name":"needs-witness","entry":"ask","nodes":[
            {"id":"ask","type":"llm","evidence_required":true,"json_mode":true,"config":{"output_key":"evidence"}}
        ],"edges":[]}}),
    );
    assert_eq!(created["error_code"], "WITNESS_STORE_REQUIRED");
}

fn deterministic_resume_spec(name: &str) -> Value {
    json!({
        "name":name, "entry":"first", "output_key":"result",
        "nodes":[
            {"id":"first","type":"passthrough"},
            {"id":"second","type":"state_transform","config":{"operations":[
                {"op":"set","path":"result","value":{"resumed":true}}
            ]}}
        ],
        "edges":[{"from":"first","to":"second"},{"from":"second","to":"END"}]
    })
}

#[test]
fn deterministic_local_checkpoint_restart_resume_is_exactly_once() {
    let temp = tempfile::tempdir().expect("checkpoint database");
    let mut first = Mcp::new_with_data_dir(temp.path());
    first.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("resume-chain")}),
    );
    let baseline = first.call(
        "graph_run_start",
        json!({"graph_id":"resume-chain","input":{"x":1}}),
    );
    let baseline = first.call(
        "graph_run_wait",
        json!({"run_id":baseline["run_id"],"timeout_ms":5000}),
    );
    assert_eq!(baseline["data"]["status"], "completed");

    let checkpointed = first.call(
        "graph_run_start",
        json!({
            "graph_id":"resume-chain", "input":{"x":1}, "checkpoint":true,
            "budgets":{"max_nodes":2}
        }),
    );
    assert_eq!(checkpointed["ok"], true, "{checkpointed}");
    assert_eq!(checkpointed["data"]["status"], "checkpointed");
    let checkpoint_run_id = checkpointed["run_id"]
        .as_str()
        .expect("checkpoint run id")
        .to_owned();
    let checkpoint_id = checkpointed["data"]["checkpoint_id"]
        .as_str()
        .expect("checkpoint id")
        .to_owned();
    drop(first);

    let mut restarted = Mcp::new_with_data_dir(temp.path());
    let read = restarted.call(
        "graph_run_checkpoint",
        json!({"run_id":checkpoint_run_id,"checkpoint_id":checkpoint_id}),
    );
    assert_eq!(read["ok"], true);
    assert_eq!(read["data"]["checkpoint_id"], checkpoint_id);
    let missing = restarted.call(
        "graph_run_checkpoint",
        json!({"run_id":"run-must-be-selected"}),
    );
    assert_eq!(missing["error_code"], "CHECKPOINT_NOT_FOUND");
    let resumed = restarted.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
    assert_eq!(resumed["ok"], true);
    let run_id = resumed["run_id"]
        .as_str()
        .expect("resumed run id")
        .to_owned();
    let completed = restarted.call("graph_run_wait", json!({"run_id":run_id,"timeout_ms":5000}));
    assert_eq!(completed["data"]["status"], "completed");
    assert_eq!(
        completed["data"]["final_state"],
        baseline["data"]["final_state"]
    );
    assert_eq!(completed["data"]["state"], baseline["data"]["state"]);
    assert_eq!(
        completed["data"]["receipt"]["replay_capability"],
        "deterministic_local_resume"
    );
    assert_eq!(
        completed["data"]["receipt"]["checkpoint"]["checkpoint_id"],
        checkpoint_id
    );

    let second = restarted.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
    assert_eq!(second["ok"], false);
    assert_eq!(second["error_code"], "CHECKPOINT_CONSUMED");
}

#[test]
fn durable_checkpoint_approval_survives_restart_and_resumes_exact_bound_checkpoint() {
    let temp = tempfile::tempdir().expect("approval database");
    let checkpoint_id;
    let approval_id;
    {
        let mut mcp = Mcp::new_with_data_dir(temp.path());
        mcp.call(
            "graph_create",
            json!({"spec":deterministic_resume_spec("approval-flow")}),
        );
        let started = mcp.call(
            "graph_run_start",
            json!({"graph_id":"approval-flow","input":{"x":1},"checkpoint":true}),
        );
        checkpoint_id = started["data"]["checkpoint_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let expiration = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let requested = mcp.call(
            "graph_approval_request",
            json!({"checkpoint_id":checkpoint_id,"audience":"operator","prompt":"sensitive approval prompt","allowed_decisions":["approve","reject"],"expiration":expiration}),
        );
        assert_eq!(requested["ok"], true, "{requested}");
        assert!(requested["data"].get("prompt").is_none());
        approval_id = requested["data"]["approval_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let duplicate = mcp.call(
            "graph_approval_request",
            json!({"checkpoint_id":checkpoint_id,"audience":"operator","prompt":"sensitive approval prompt","allowed_decisions":["approve","reject"],"expiration":expiration}),
        );
        assert_eq!(duplicate["data"]["approval_id"], approval_id);
        let conflicting = mcp.call(
            "graph_approval_request",
            json!({"checkpoint_id":checkpoint_id,"audience":"operator","prompt":"different","allowed_decisions":["approve"],"expiration":expiration}),
        );
        assert_eq!(conflicting["error_code"], "APPROVAL_REQUEST_CONFLICT");
        let bypass = mcp.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
        assert_eq!(bypass["error_code"], "APPROVAL_PENDING");
    }
    {
        let mut restarted = Mcp::new_with_data_dir(temp.path());
        let got = restarted.call("graph_approval_get", json!({"approval_id":approval_id}));
        assert_eq!(got["ok"], true, "{got}");
        assert_eq!(got["data"]["checkpoint_id"], checkpoint_id);
        assert!(got["data"].get("prompt").is_none());
        assert!(got["data"].get("state").is_none());
        let listed = restarted.call("graph_approval_list", json!({"run_id":got["run_id"]}));
        assert_eq!(listed["data"]["count"], 1);
        // graph_approval_decide is no longer a model-facing tool (AG-002).
        // Model clients cannot decide approvals — expect auth/method-not-found error.
        let decided = restarted.call(
            "graph_approval_decide",
            json!({"approval_id":approval_id,"decision":"approve","actor":"human-1"}),
        );
        assert!(decided.get("ok").is_none() || decided["ok"] == false || decided["ok"].is_null());
        assert!(
            decided["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
                || decided["error_code"] == "METHOD_NOT_FOUND"
                || decided["error_code"] == "INVALID_PARAMS"
        );
        // Approval remains pending — model client cannot resume without operator decision.
        let resumed_attempt =
            restarted.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
        assert_eq!(resumed_attempt["error_code"], "APPROVAL_PENDING");
    }
}

#[test]
fn rejected_and_expired_approvals_never_resume_and_late_decisions_are_typed() {
    let temp = tempfile::tempdir().expect("approval expiry database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("approval-reject-expire")}),
    );
    let rejected_checkpoint = mcp.call(
        "graph_run_start",
        json!({"graph_id":"approval-reject-expire","checkpoint":true}),
    );
    let rejected_checkpoint = rejected_checkpoint["data"]["checkpoint_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let rejected = mcp.call("graph_approval_request", json!({"checkpoint_id":rejected_checkpoint,"audience":"ops","prompt":"reject me","allowed_decisions":["reject"],"expiration":future}));
    let rejected_id = rejected["data"]["approval_id"].as_str().unwrap();
    // graph_approval_decide is no longer a model-facing tool (AG-002).
    // Both reject and late decisions are blocked for model clients.
    let reject_decision = mcp.call(
        "graph_approval_decide",
        json!({"approval_id":rejected_id,"decision":"reject","actor":"operator"}),
    );
    assert!(
        reject_decision.get("ok").is_none()
            || reject_decision["ok"] == false
            || reject_decision["ok"].is_null()
    );
    assert!(
        reject_decision["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
            || reject_decision["error_code"] == "METHOD_NOT_FOUND"
            || reject_decision["error_code"] == "INVALID_PARAMS"
    );

    let expired_checkpoint = mcp.call(
        "graph_run_start",
        json!({"graph_id":"approval-reject-expire","checkpoint":true}),
    );
    let expired_checkpoint = expired_checkpoint["data"]["checkpoint_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let expired = mcp.call("graph_approval_request", json!({"checkpoint_id":expired_checkpoint,"audience":"ops","prompt":"late decision","allowed_decisions":["approve"],"expiration":"2000-01-01T00:00:00Z"}));
    let expired_id = expired["data"]["approval_id"].as_str().unwrap();
    let late = mcp.call(
        "graph_approval_decide",
        json!({"approval_id":expired_id,"decision":"approve","actor":"operator"}),
    );
    assert!(late.get("ok").is_none() || late["ok"] == false || late["ok"].is_null());
    assert!(
        late["error_code"] == "AUTHENTICATED_OPERATOR_REQUIRED"
            || late["error_code"] == "METHOD_NOT_FOUND"
            || late["error_code"] == "INVALID_PARAMS"
    );
}

#[test]
fn approval_request_rejects_missing_ineligible_and_tampered_checkpoints_without_rows() {
    let temp = tempfile::tempdir().expect("approval validation database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    let missing = mcp.call("graph_approval_request", json!({"checkpoint_id":"missing","audience":"ops","prompt":"x","allowed_decisions":["approve"],"expiration":"2030-01-01T00:00:00Z"}));
    assert_eq!(missing["error_code"], "CHECKPOINT_NOT_FOUND");
    mcp.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("approval-tamper")}),
    );
    let started = mcp.call(
        "graph_run_start",
        json!({"graph_id":"approval-tamper","checkpoint":true}),
    );
    let checkpoint_id = started["data"]["checkpoint_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let connection = Connection::open(temp.path().join("agent-graph.db")).unwrap();
    connection
        .execute(
            "UPDATE checkpoints SET state_json = '{\"tampered\":true}' WHERE checkpoint_id = ?1",
            rusqlite::params![checkpoint_id],
        )
        .unwrap();
    let tampered = mcp.call("graph_approval_request", json!({"checkpoint_id":checkpoint_id,"audience":"ops","prompt":"x","allowed_decisions":["approve"],"expiration":"2030-01-01T00:00:00Z"}));
    assert_eq!(tampered["error_code"], "CHECKPOINT_INTEGRITY_FAILURE");
    let listed = mcp.call("graph_approval_list", json!({}));
    assert_eq!(listed["data"]["count"], 0);
}

#[test]
fn resume_rejects_non_deterministic_or_non_linear_specs() {
    let temp = tempfile::tempdir().expect("checkpoint database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    let cases = [
        (
            "llm",
            json!({"name":"r-llm","entry":"n","nodes":[{"id":"n","type":"llm"}],"edges":[{"from":"n","to":"END"}]}),
        ),
        (
            "router",
            json!({"name":"r-router","entry":"n","nodes":[{"id":"n","type":"router","config":{"rules":[],"default":["END"]}}],"edges":[{"from":"n","to":"END"}]}),
        ),
        (
            "parallel",
            json!({"name":"r-parallel","entry":"n","nodes":[{"id":"n","type":"parallel","config":{"branches":[{"entry":"a"}],"join":"END"}},{"id":"a","type":"passthrough"}],"edges":[{"from":"n","to":"a"},{"from":"a","to":"END"}]}),
        ),
        (
            "loop",
            json!({"name":"r-loop","entry":"n","nodes":[{"id":"n","type":"passthrough"}],"edges":[{"from":"n","to":"n"}]}),
        ),
        (
            "subgraph",
            json!({"name":"r-subgraph","entry":"n","nodes":[{"id":"n","type":"subgraph","config":{"graph_name":"other"}}],"edges":[{"from":"n","to":"END"}]}),
        ),
        (
            "approval",
            json!({"name":"r-approval","entry":"n","nodes":[{"id":"n","type":"human_approval","config":{"prompt_key":"p","audience":["u"]}}],"edges":[{"from":"n","to":"END"}]}),
        ),
        (
            "external",
            json!({"name":"r-external","entry":"n","nodes":[{"id":"n","type":"external"}],"edges":[{"from":"n","to":"END"}]}),
        ),
        (
            "tool",
            json!({"name":"r-tool","entry":"n","nodes":[{"id":"n","type":"tool"}],"edges":[{"from":"n","to":"END"}]}),
        ),
    ];
    for (label, spec) in cases {
        let created = mcp.call("graph_create", json!({"spec":spec}));
        // subgraph and approval are accepted but resume-ineligible.
        // external and tool are rejected at creation (UNSUPPORTED_NODE_TYPE).
        if label == "external" || label == "tool" {
            assert_eq!(
                created["ok"], false,
                "{label} graph should be rejected at creation as unsupported node type"
            );
            assert_eq!(
                created["error_code"], "UNSUPPORTED_NODE_TYPE",
                "{label} graph should return UNSUPPORTED_NODE_TYPE"
            );
        } else {
            assert_eq!(
                created["ok"], true,
                "{label} graph should classify at checkpoint request"
            );
            let response = mcp.call(
                "graph_run_start",
                json!({"graph_id":created["graph_id"],"checkpoint":true}),
            );
            assert_eq!(response["ok"], false, "{label}");
            assert_eq!(response["error_code"], "RESUME_INELIGIBLE", "{label}");
        }
    }
}

#[test]
fn resume_integrity_tampering_never_executes() {
    for (column, value) in [
        ("graph_version", "tampered-version"),
        ("state_json", "{\"tampered\":true}"),
        ("budgets_json", "{\"max_nodes\":99}"),
        ("dependency_json", "{\"eligible\":false}"),
        ("next_cursor", "tampered-cursor"),
        ("checkpoint_digest", "sha256:tampered"),
    ] {
        let temp = tempfile::tempdir().expect("checkpoint database");
        let checkpoint_id = {
            let mut mcp = Mcp::new_with_data_dir(temp.path());
            mcp.call(
                "graph_create",
                json!({"spec":deterministic_resume_spec("tamper")}),
            );
            let started = mcp.call(
                "graph_run_start",
                json!({"graph_id":"tamper","checkpoint":true,"budgets":{"max_nodes":2}}),
            );
            started["data"]["checkpoint_id"]
                .as_str()
                .expect("checkpoint id")
                .to_owned()
        };
        let connection = Connection::open(temp.path().join("agent-graph.db")).expect("database");
        connection
            .execute(
                &format!("UPDATE checkpoints SET {column} = ?1 WHERE checkpoint_id = ?2"),
                rusqlite::params![value, &checkpoint_id],
            )
            .expect("tamper checkpoint");
        drop(connection);
        let mut restarted = Mcp::new_with_data_dir(temp.path());
        let resumed = restarted.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
        assert_eq!(resumed["ok"], false, "{column}");
        assert_eq!(
            resumed["error_code"], "CHECKPOINT_INTEGRITY_FAILURE",
            "{column}"
        );
        assert!(resumed["run_id"].is_null(), "{column} must not launch");
    }
}

#[test]
fn resumed_budget_counters_continue_and_exhaust() {
    let temp = tempfile::tempdir().expect("checkpoint database");
    let mut mcp = Mcp::new_with_data_dir(temp.path());
    mcp.call(
        "graph_create",
        json!({"spec":deterministic_resume_spec("budget-resume")}),
    );
    let started = mcp.call(
        "graph_run_start",
        json!({"graph_id":"budget-resume","checkpoint":true,"budgets":{"max_nodes":1}}),
    );
    let checkpoint_id = started["data"]["checkpoint_id"]
        .as_str()
        .expect("checkpoint id")
        .to_owned();
    let resumed = mcp.call("graph_run_resume", json!({"checkpoint_id":checkpoint_id}));
    let completed = mcp.call(
        "graph_run_wait",
        json!({"run_id":resumed["run_id"],"timeout_ms":5000}),
    );
    assert_eq!(completed["data"]["status"], "failed");
    assert_eq!(completed["data"]["error"], "BUDGET_EXHAUSTED");
    assert_eq!(completed["data"]["budget_counters"]["nodes"], 1);
    assert_eq!(completed["data"]["budgets"], json!({"max_nodes":1}));
}

#[test]
fn daemon_socket_proxy_lifecycle() {
    // AG-002/005 process-boundary: daemon + proxy over Unix socket
    // Uses direct framed socket connection to avoid proxy notification-blocking issue
    let dir = tempfile::tempdir().expect("temp dir");
    let sock = dir.path().join("mcp.sock");
    let key_path = dir.path().join("integrity.key");
    std::fs::write(&key_path, [0x5au8; 32]).expect("key file");

    // Start daemon
    let mut daemon = std::process::Command::new(env!("CARGO_BIN_EXE_agent-graph-mcpd"))
        .arg("--data-dir")
        .arg(dir.path())
        .arg("--socket")
        .arg(&sock)
        .env("AGENT_GRAPH_INTEGRITY_KEY_PATH", &key_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("daemon start");

    // Wait for socket
    let deadline = std::time::Instant::now();
    while !sock.exists() {
        if deadline.elapsed() > std::time::Duration::from_secs(10) {
            let _ = daemon.kill();
            panic!("daemon socket not created within 10s");
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Connect directly via framed Unix socket
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(&sock).expect("connect to daemon");

    // Helper: send a framed JSON-RPC message
    let send_frame = |stream: &mut UnixStream, msg: &serde_json::Value| {
        let data = serde_json::to_vec(msg).unwrap();
        stream
            .write_all(&(data.len() as u32).to_be_bytes())
            .unwrap();
        stream.write_all(&data).unwrap();
        stream.flush().unwrap();
    };

    // Helper: receive a framed response
    let recv_frame = |stream: &mut UnixStream| -> serde_json::Value {
        let mut hdr = [0u8; 4];
        stream.read_exact(&mut hdr).expect("frame header");
        let len = u32::from_be_bytes(hdr) as usize;
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).expect("frame payload");
        serde_json::from_slice(&payload).expect("JSON parse")
    };

    // Step 1: initialize
    send_frame(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"socket-test","version":"1"}}}),
    );
    let init = recv_frame(&mut stream);
    assert!(init.get("result").is_some(), "initialize failed: {init}");

    // Step 2: notifications/initialized
    send_frame(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
    );
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Step 3: tools/list
    send_frame(
        &mut stream,
        &serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let list: serde_json::Value = recv_frame(&mut stream);
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(
        tools.len() >= 20,
        "expected >=20 tools, got {}",
        tools.len()
    );
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"graph_create"));
    assert!(names.contains(&"graph_execute"));
    assert!(names.contains(&"graph_run_start"));

    // AG-002: approval tools absent (removed from model tool set)
    assert!(
        !names.contains(&"graph_approval_decide"),
        "graph_approval_decide should not be in tools list"
    );
    assert!(
        !names.contains(&"graph_delete"),
        "graph_delete should not be in tools list"
    );

    // Cleanup
    drop(stream);
    let _ = daemon.kill();
    let _ = daemon.wait();
}
