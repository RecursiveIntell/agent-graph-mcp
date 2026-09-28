//! Process-boundary regressions: connection resources do not own run resources.
#![cfg(target_os = "linux")]
use agent_graph_mcp::transport::{read_frame, write_frame};
use serde_json::{json, Value};
use std::io::Write;
use std::os::unix::{fs::PermissionsExt, net::UnixStream, process::CommandExt};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Fixture {
    child: Child,
    root: tempfile::TempDir,
    socket: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("m.sock");
        let key = root.path().join("key");
        std::fs::write(&key, [7u8; 32]).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let fake = bin.join("codex");
        std::fs::write(&fake, r#"#!/usr/bin/python3
import json,sys,time,os
for line in sys.stdin:
 r=json.loads(line);m=r.get('method');i=r.get('id')
 if m=='initialize':print(json.dumps({'id':i,'result':{}}),flush=True)
 elif m=='thread/start':print(json.dumps({'id':i,'result':{'thread':{'id':'fixture-thread'}}}),flush=True)
 elif m=='turn/start':
  with open(os.path.join(os.environ['HOME'],'prompts.jsonl'),'a') as f:f.write(json.dumps(r)+'\n')
  print(json.dumps({'id':i,'result':{'turn':{'id':'fixture-turn'}}}),flush=True)
  time.sleep(2)
  print(json.dumps({'method':'item/agentMessage/delta','params':{'delta':'fixture output'}}),flush=True)
  print(json.dumps({'method':'turn/completed','params':{'turn':{'id':'fixture-turn','status':'completed'}}}),flush=True)
"#).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let executable = std::env::var_os("AGENT_GRAPH_TEST_BINARY")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_agent-graph-mcpd").into());
        let mut cmd = Command::new(executable);
        cmd.args([
            "--data-dir",
            root.path().join("store").to_str().unwrap(),
            "--socket",
            socket.to_str().unwrap(),
            "--base-url",
            "codex-app-server://",
            "--model",
            "fixture",
        ])
        .env_clear()
        .env("HOME", root.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("AGENT_GRAPH_INTEGRITY_KEY_PATH", key)
        .env("AGENT_GRAPH_CODEX_DISABLED_MCP_SERVERS_JSON", "[]")
        .env("AGENT_GRAPH_CODEX_MAX_PROCESSES", "2")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                let limit = libc::rlimit {
                    rlim_cur: 192,
                    rlim_max: 192,
                };
                if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().unwrap();
        let mut f = Self {
            child,
            root,
            socket,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if f.socket.exists() {
                break;
            }
            assert!(
                f.child.try_wait().unwrap().is_none(),
                "fixture daemon exited before readiness"
            );
            assert!(Instant::now() < deadline, "fixture readiness timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        f
    }
    fn connect(&self) -> UnixStream {
        let s = UnixStream::connect(&self.socket).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
        s.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        s
    }
    fn initialized(&self) -> UnixStream {
        let mut s = self.connect();
        write_frame(
            &mut s,
            &serde_json::to_vec(&json!({"hello":{"protocol_version":1}})).unwrap(),
        )
        .unwrap();
        let _: Value = serde_json::from_slice(&read_frame(&mut s).unwrap()).unwrap();
        let reply = request(
            &mut s,
            1,
            "initialize",
            json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"connection-test","version":"1"}}),
        );
        assert!(reply.get("result").is_some(), "{reply}");
        write_frame(
            &mut s,
            &serde_json::to_vec(
                &json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
            )
            .unwrap(),
        )
        .unwrap();
        s
    }
    fn fd_count(&self) -> usize {
        std::fs::read_dir(format!("/proc/{}/fd", self.child.id()))
            .unwrap()
            .count()
    }
    fn released(&mut self, baseline: usize) {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "daemon died under connection churn"
            );
            if self.fd_count() <= baseline + 2 {
                return;
            }
            assert!(
                Instant::now() < end,
                "descriptors retained: baseline={baseline}, after={}",
                self.fd_count()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.root.path();
    }
}
fn request(s: &mut UnixStream, id: u64, method: &str, params: Value) -> Value {
    write_frame(
        s,
        &serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .unwrap(),
    )
    .unwrap();
    let reply: Value = serde_json::from_slice(&read_frame(s).unwrap()).unwrap();
    assert_eq!(reply["id"], id);
    reply
}
fn tool(s: &mut UnixStream, id: u64, name: &str, arguments: Value) -> Value {
    let reply = request(
        s,
        id,
        "tools/call",
        json!({"name":name,"arguments":arguments}),
    );
    let r = &reply["result"];
    let data = r.get("structuredContent").cloned().unwrap_or_else(|| {
        serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap()
    });
    assert_eq!(data["ok"], true, "{data}");
    data
}

#[test]
fn planning_template_runtime_prompts_preserve_original_evidence() {
    let f = Fixture::new();
    let mut client = f.initialized();
    let mut spec =
        agent_graph_mcp::templates::instantiate("plan_critique_refine", "evidence-runtime")
            .unwrap();
    for node in spec["nodes"].as_array_mut().unwrap() {
        if node["type"] == "llm" {
            node["model"] = json!("fixture");
            node["config"]["timeout_ms"] = json!(10000);
        }
    }
    tool(&mut client, 2, "graph_create", json!({"spec":spec}));
    let start = tool(
        &mut client,
        3,
        "graph_run_start",
        json!({"graph_id":"evidence-runtime","input":{"source_marker":"original-evidence-unique"},"budgets":{"max_nodes":8,"max_llm_calls":3,"max_wall_clock_ms":20000}}),
    );
    let rid = start["run_id"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status = tool(
            &mut client,
            4,
            "graph_run_get",
            json!({"run_id":rid,"compact":true}),
        );
        if status["data"]["persistence_status"] == "durable_terminal" {
            assert_eq!(status["data"]["success"], true, "{status}");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    let captured = std::fs::read_to_string(f.root.path().join("prompts.jsonl")).unwrap();
    let prompts: Vec<&str> = captured.lines().collect();
    assert_eq!(prompts.len(), 3);
    assert!(prompts
        .iter()
        .all(|p| p.contains("original-evidence-unique")));
    assert!(
        prompts[2].contains("draft")
            && prompts[2].contains("critique")
            && prompts[2].contains("fixture output")
    );
}

#[test]
fn initialized_close_reclaims_socket_descriptors() {
    let mut f = Fixture::new();
    let baseline = f.fd_count();
    for _ in 0..20 {
        drop(f.initialized());
    }
    f.released(baseline);
}

#[test]
fn three_batches_of_200_connections_do_not_exhaust_low_nofile() {
    let mut f = Fixture::new();
    let baseline = f.fd_count();
    for _ in 0..3 {
        for _ in 0..200 {
            let mut s = f.initialized();
            tool(&mut s, 2, "graph_status", json!({"resource":"server"}));
        }
        f.released(baseline);
    }
}

#[test]
fn malformed_and_incomplete_clients_reclaim_resources() {
    let mut f = Fixture::new();
    let baseline = f.fd_count();
    for _ in 0..20 {
        drop(f.connect());
        let mut partial = f.connect();
        partial.write_all(&[0, 0]).unwrap();
        drop(partial);
        let mut invalid = f.connect();
        invalid.write_all(&(1_048_577u32).to_be_bytes()).unwrap();
        drop(invalid);
        let mut bad_init = f.connect();
        write_frame(&mut bad_init, b"not-json").unwrap();
        drop(bad_init);
        let mut after_init = f.initialized();
        after_init.write_all(&(1_048_577u32).to_be_bytes()).unwrap();
        drop(after_init);
    }
    f.released(baseline);
}

#[test]
fn held_partial_handshake_expires_without_daemon_exit() {
    let mut f = Fixture::new();
    let mut s = f.connect();
    s.write_all(&[0, 0]).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(7))).unwrap();
    let result = read_frame(&mut s);
    let err = result.unwrap_err();
    assert!(
        !err.to_string().contains("temporarily unavailable"),
        "peer did not close by initialization deadline: {err}"
    );
    assert!(
        !err.to_string().contains("timed out"),
        "peer did not close by initialization deadline: {err}"
    );
    assert!(f.child.try_wait().unwrap().is_none());
}

#[test]
fn initialized_client_outlives_initialization_deadline() {
    let mut f = Fixture::new();
    let mut s = f.initialized();
    std::thread::sleep(Duration::from_secs(6));
    tool(&mut s, 2, "graph_status", json!({"resource":"server"}));
    assert!(f.child.try_wait().unwrap().is_none());
}

#[test]
fn temporary_accept_pressure_does_not_kill_daemon() {
    let mut f = Fixture::new();
    let baseline = f.fd_count();
    let mut held = Vec::new();
    for _ in 0..180 {
        held.push(f.connect());
    }
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        f.child.try_wait().unwrap().is_none(),
        "temporary EMFILE killed daemon"
    );
    drop(held);
    f.released(baseline);
    let mut s = f.initialized();
    tool(&mut s, 2, "graph_status", json!({"resource":"server"}));
}

#[test]
fn submitting_connection_disconnect_does_not_cancel_admitted_run() {
    let mut f = Fixture::new();
    let mut first = f.initialized();
    let spec = json!({"name":"lifetime-owner","entry":"work","output_key":"answer","nodes":[{"id":"work","type":"llm","model":"fixture","prompt":"fixture","config":{"output_key":"answer","timeout_ms":10000}}],"edges":[{"from":"work","to":"END"}]});
    tool(&mut first, 2, "graph_create", json!({"spec":spec}));
    let started = tool(
        &mut first,
        3,
        "graph_run_start",
        json!({"graph_id":"lifetime-owner","budgets":{"max_wall_clock_ms":20000,"max_llm_calls":1}}),
    );
    let id = started["run_id"].as_str().unwrap().to_owned();
    assert_eq!(
        tool(
            &mut first,
            4,
            "graph_run_get",
            json!({"run_id":id,"compact":true})
        )["data"]["status"],
        "running"
    );
    drop(first);
    let mut second = f.initialized();
    let end = Instant::now() + Duration::from_secs(12);
    loop {
        let d = tool(
            &mut second,
            5,
            "graph_run_get",
            json!({"run_id":id,"compact":true}),
        );
        if d["data"]["persistence_status"] == "durable_terminal" {
            assert_eq!(d["data"]["status"], "completed", "{d}");
            assert_eq!(d["data"]["success"], true);
            break;
        }
        assert!(Instant::now() < end, "run did not finish: {d}");
        std::thread::sleep(Duration::from_millis(50));
    }
    let artifact = tool(
        &mut second,
        6,
        "graph_run_artifact",
        json!({"run_id":id,"artifact":"output","offset":0,"limit":16384}),
    );
    assert!(artifact["data"]["data"]
        .as_str()
        .unwrap()
        .contains("fixture output"));
    drop(second);
    assert!(f.child.try_wait().unwrap().is_none());
}
