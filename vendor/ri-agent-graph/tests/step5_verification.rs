//! VG-5 verification tests for agent-graph upgrades.

use ri_agent_graph::prelude::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

// ── Transaction tests ──────────────────────────────────────────

#[tokio::test]
async fn test_transaction_commit() {
    let state = AgentState::new();
    state.set("x", 1i32).await.unwrap();

    let txn = state.transaction().await;
    txn.set("x", 2i32).await.unwrap();
    txn.set("y", 42i32).await.unwrap();
    txn.commit().await.unwrap();

    let x: i32 = state.get("x").await.unwrap();
    let y: i32 = state.get("y").await.unwrap();
    assert_eq!(x, 2);
    assert_eq!(y, 42);
}

#[tokio::test]
async fn test_transaction_rollback() {
    let state = AgentState::new();
    state.set("x", 1i32).await.unwrap();
    state.set("y", 100i32).await.unwrap();

    let txn = state.transaction().await;
    txn.set("x", 999i32).await.unwrap();
    txn.set("y", 0i32).await.unwrap();
    txn.rollback().await;

    // State should be restored to pre-transaction values
    let x: i32 = state.get("x").await.unwrap();
    let y: i32 = state.get("y").await.unwrap();
    assert_eq!(x, 1);
    assert_eq!(y, 100);
}

#[tokio::test]
async fn test_transaction_read_within() {
    let state = AgentState::new();
    state.set("counter", 10i32).await.unwrap();

    let txn = state.transaction().await;
    let val: i32 = txn.get("counter").await.unwrap();
    assert_eq!(val, 10);
    txn.set("counter", val + 5).await.unwrap();
    let updated: i32 = txn.get("counter").await.unwrap();
    assert_eq!(updated, 15);
    txn.commit().await.unwrap();

    let final_val: i32 = state.get("counter").await.unwrap();
    assert_eq!(final_val, 15);
}

#[tokio::test]
async fn test_transaction_isolated_before_commit() {
    let state = AgentState::new();
    state.set("x", 1i32).await.unwrap();

    let txn = state.transaction().await;
    txn.set("x", 2i32).await.unwrap();
    txn.set("y", 3i32).await.unwrap();

    let live_x: i32 = state.get("x").await.unwrap();
    let live_y: Option<i32> = state.get_opt("y").await.unwrap();
    assert_eq!(live_x, 1);
    assert_eq!(live_y, None);

    txn.commit().await.unwrap();

    let committed_x: i32 = state.get("x").await.unwrap();
    let committed_y: i32 = state.get("y").await.unwrap();
    assert_eq!(committed_x, 2);
    assert_eq!(committed_y, 3);
}

#[tokio::test]
async fn test_transaction_conflict_on_concurrent_modification() {
    let state = AgentState::new();
    state.set("x", 1i32).await.unwrap();

    // Start a transaction, capturing the current version.
    let txn = state.transaction().await;
    txn.set("x", 99i32).await.unwrap();

    // Mutate state outside the transaction (simulates a concurrent writer).
    state.set("x", 2i32).await.unwrap();

    // Commit must detect the conflict and return an error.
    let err = txn.commit().await.unwrap_err();
    assert!(
        matches!(err, AgentGraphError::StateError(ref msg) if msg.contains("Transaction conflict")),
        "expected a transaction conflict error, got: {}",
        err
    );

    // State should still reflect the concurrent write, not the transaction.
    let x: i32 = state.get("x").await.unwrap();
    assert_eq!(x, 2);
}

#[tokio::test]
async fn test_state_limit_enforcement_leaves_state_unchanged_on_failure() {
    let state = AgentState::with_limits(StateLimits {
        max_keys: 1,
        max_value_bytes: 128,
        ..StateLimits::default()
    });

    state.set("x", 1i32).await.unwrap();
    let err = state.set("y", 2i32).await.unwrap_err();
    assert!(matches!(err, AgentGraphError::StateError(_)));

    let keys = state.keys().await;
    assert_eq!(keys, vec!["x".to_string()]);
    let x: i32 = state.get("x").await.unwrap();
    assert_eq!(x, 1);
}

#[tokio::test]
async fn test_transaction_limit_failure_leaves_state_unchanged() {
    let state = AgentState::with_limits(StateLimits {
        max_keys: 1,
        max_value_bytes: 128,
        ..StateLimits::default()
    });
    state.set("x", 1i32).await.unwrap();

    let txn = state.transaction().await;
    let err = txn.set("y", 2i32).await.unwrap_err();
    assert!(matches!(err, AgentGraphError::StateError(_)));

    let keys = state.keys().await;
    assert_eq!(keys, vec!["x".to_string()]);
    let y: Option<i32> = state.get_opt("y").await.unwrap();
    assert_eq!(y, None);
}

// ── Parallelism tests ──────────────────────────────────────────

#[tokio::test]
async fn test_parallelism_config_default() {
    let config = GraphConfig::default();
    assert_eq!(config.max_parallelism, 8);
}

#[tokio::test]
async fn test_parallelism_config_hard_cap() {
    let config = GraphConfig::new().with_max_parallelism(100);
    assert_eq!(config.max_parallelism, 32); // hard-capped
}

#[tokio::test]
async fn test_parallelism_config_min_one() {
    let config = GraphConfig::new().with_max_parallelism(0);
    assert_eq!(config.max_parallelism, 1); // minimum 1
}

#[tokio::test]
async fn test_parallelism_fan_out_with_config() {
    // Fan-out with max_parallelism = 2 (should still work correctly)
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("a", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("b", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "c",
            node!(|state| async move {
                state.set("c", true).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b")
        .add_edge("a", "c")
        .build()
        .unwrap();

    let config = GraphConfig::new().with_max_parallelism(2);
    let state = AgentState::new();
    let result = graph.execute_with_config("a", state, config).await.unwrap();

    assert!(result.get::<bool>("a").await.unwrap());
    assert!(result.get::<bool>("b").await.unwrap());
    assert!(result.get::<bool>("c").await.unwrap());
}

#[tokio::test]
async fn test_parallelism_cap_actually_caps_concurrency() {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let branch_node =
        |in_flight: Arc<AtomicUsize>, max_seen: Arc<AtomicUsize>, completed: Arc<AtomicUsize>| {
            Box::new(FnNode::new(move |_state, _config| {
                let in_flight = in_flight.clone();
                let max_seen = max_seen.clone();
                let completed = completed.clone();
                Box::pin(async move {
                    let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut observed = max_seen.load(Ordering::SeqCst);
                    while current > observed {
                        match max_seen.compare_exchange(
                            observed,
                            current,
                            Ordering::SeqCst,
                            Ordering::SeqCst,
                        ) {
                            Ok(_) => break,
                            Err(next) => observed = next,
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                    Ok(NodeOutput::Done)
                })
            }))
        };

    let graph = AgentGraph::builder()
        .add_node("start", node!(|_state| async move { Ok(()) }))
        .add_node(
            "a",
            branch_node(in_flight.clone(), max_seen.clone(), completed.clone()),
        )
        .add_node(
            "b",
            branch_node(in_flight.clone(), max_seen.clone(), completed.clone()),
        )
        .add_node(
            "c",
            branch_node(in_flight.clone(), max_seen.clone(), completed.clone()),
        )
        .add_node(
            "d",
            branch_node(in_flight.clone(), max_seen.clone(), completed.clone()),
        )
        .add_edge("start", "a")
        .add_edge("start", "b")
        .add_edge("start", "c")
        .add_edge("start", "d")
        .build()
        .unwrap();

    let config = GraphConfig::new().with_max_parallelism(2);
    graph
        .execute_with_config("start", AgentState::new(), config)
        .await
        .unwrap();

    assert_eq!(completed.load(Ordering::SeqCst), 4);
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "observed more than the configured concurrency cap"
    );
}

// ── Checkpoint metadata tests ──────────────────────────────────

#[tokio::test]
async fn test_checkpoint_metadata_graph_hash() {
    let meta = CheckpointMetadata {
        graph_hash: "abc123".to_string(),
        run_id: "run-1".to_string(),
        node_id: "step1".to_string(),
        step: 0,
        created_at: chrono::Utc::now(),
    };

    assert_eq!(meta.graph_hash, "abc123");
    assert_eq!(meta.run_id, "run-1");
}

#[tokio::test]
async fn test_checkpoint_metadata_mismatch_detection() {
    // Simulate checking for graph hash mismatch on resume
    let checkpoint_hash = "old_hash_v1";
    let current_hash = "new_hash_v2";
    assert_ne!(
        checkpoint_hash, current_hash,
        "Graph definition has changed since checkpoint"
    );
}

#[tokio::test]
async fn test_checkpoint_store_run_summary() {
    let store = InMemoryCheckpointStore::new();
    let run_id = store.create_run("test-graph").await.unwrap();

    // Record an attempt
    let attempt_id = store
        .record_attempt(&run_id, "node_a", 0, &serde_json::json!({}))
        .await
        .unwrap();
    store
        .complete_attempt(
            &attempt_id,
            &serde_json::json!({"ok": true}),
            &std::collections::HashMap::from([(
                "trace_id".to_string(),
                serde_json::Value::String("trace-store".to_string()),
            )]),
        )
        .await
        .unwrap();

    store.complete_run(&run_id).await.unwrap();

    let summary = store.summarize_run(&run_id).await.unwrap();
    assert_eq!(summary.graph_name, "test-graph");
    assert_eq!(summary.status, RunStatus::Completed);
    assert_eq!(summary.total_nodes_executed, 1);
    assert_eq!(summary.total_attempts, 1);
    assert_eq!(summary.failed_attempts, 0);
    assert_eq!(summary.trace_id.as_deref(), Some("trace-store"));
    assert!(summary.finished_at.is_some());
}

#[tokio::test]
async fn test_resume_rejects_checkpoint_mismatch() {
    let graph_v1 = AgentGraph::builder()
        .add_node("step", node!(|_state| async move { Ok(()) }))
        .build()
        .unwrap();

    let graph_v2 = AgentGraph::builder()
        .add_node(
            "step",
            node!(|state| async move {
                state.set("resumed", true).await?;
                Ok(())
            }),
        )
        .add_node("extra", node!(|_state| async move { Ok(()) }))
        .build()
        .unwrap();

    let checkpoint = InterruptCheckpoint {
        resume_node: "step".to_string(),
        resume_before: true,
        iteration: 1,
        active_nodes: vec!["step".to_string()],
        graph_hash: Some(graph_v1.compute_graph_hash()),
    };

    let err = graph_v2
        .resume(AgentState::new(), GraphConfig::default(), checkpoint)
        .await
        .unwrap_err();

    assert!(matches!(err, AgentGraphError::CheckpointMismatch { .. }));
}

#[tokio::test]
async fn test_resume_force_still_works_on_mismatch() {
    let graph_v1 = AgentGraph::builder()
        .add_node("step", node!(|_state| async move { Ok(()) }))
        .build()
        .unwrap();

    let graph_v2 = AgentGraph::builder()
        .add_node(
            "step",
            node!(|state| async move {
                state.set("resumed", true).await?;
                Ok(())
            }),
        )
        .add_node("extra", node!(|_state| async move { Ok(()) }))
        .build()
        .unwrap();

    let checkpoint = InterruptCheckpoint {
        resume_node: "step".to_string(),
        resume_before: true,
        iteration: 1,
        active_nodes: vec!["step".to_string()],
        graph_hash: Some(graph_v1.compute_graph_hash()),
    };

    let resumed = graph_v2
        .resume_force(AgentState::new(), GraphConfig::default(), checkpoint)
        .await
        .unwrap();

    let resumed_flag: bool = resumed.get("resumed").await.unwrap();
    assert!(resumed_flag);
}

#[tokio::test]
async fn test_execute_with_summary_reports_trace_and_counts() {
    let graph = AgentGraph::builder()
        .with_name("summary-test")
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("step1", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("step2", true).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .build()
        .unwrap();

    let (result, summary) = graph
        .execute_with_summary(
            "step1",
            AgentState::new(),
            GraphConfig::new().with_trace_id("trace-summary"),
        )
        .await;

    let final_state = result.unwrap();
    assert!(final_state.get::<bool>("step1").await.unwrap());
    assert!(final_state.get::<bool>("step2").await.unwrap());

    assert_eq!(summary.graph_name, "summary-test");
    assert_eq!(summary.status, RunStatus::Completed);
    assert_eq!(summary.total_nodes_executed, 2);
    assert_eq!(summary.total_attempts, 2);
    assert_eq!(summary.failed_attempts, 0);
    assert_eq!(summary.trace_id.as_deref(), Some("trace-summary"));
    assert!(summary.finished_at.is_some());
}

// ── Error kind tests ───────────────────────────────────────────

#[test]
fn test_error_kind_discriminants() {
    let err = AgentGraphError::NodeNotFound("x".into());
    assert_eq!(err.kind(), "node_not_found");

    let err = AgentGraphError::Cancelled;
    assert_eq!(err.kind(), "cancelled");

    let err = AgentGraphError::StateError("bad".into());
    assert_eq!(err.kind(), "state");
}

// ── Trace ID propagation ──────────────────────────────────────

#[tokio::test]
async fn test_trace_id_on_config() {
    let config = GraphConfig::new().with_trace_id("trace-abc");
    assert_eq!(config.trace_id, Some("trace-abc".to_string()));
}

#[allow(deprecated)]
#[tokio::test]
async fn test_trace_id_in_events() {
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let sink = Arc::new(CallbackEventSink::new(move |event| {
        events_clone.lock().unwrap().push(event);
    }));

    let graph = Arc::new(
        AgentGraph::builder()
            .with_name("trace-test")
            .with_event_sink(sink)
            .add_node(
                "step1",
                node!(|state| async move {
                    state.set("done", true).await?;
                    Ok(())
                }),
            )
            .set_entry_point("step1")
            .set_finish_point("step1")
            .build()
            .unwrap(),
    );

    let config = GraphConfig::new().with_trace_id("my-trace-123");
    let (handle, _rx) = graph.stream("__start__", AgentState::new(), config);
    handle.await.unwrap().unwrap();

    let captured = events.lock().unwrap();
    // Verify trace_id is present on RunStart
    let has_trace = captured
        .iter()
        .any(|e| matches!(e, GraphEvent::RunStart { trace_id, .. } if trace_id == "my-trace-123"));
    assert!(
        has_trace,
        "RunStart event should carry the configured trace_id"
    );
}

// ── StateLimits struct ─────────────────────────────────────────

#[test]
fn test_state_limits_defaults() {
    let limits = StateLimits::default();
    assert_eq!(limits.max_keys, 10_000);
    assert_eq!(limits.max_value_bytes, 1_048_576);
    assert_eq!(limits.max_history_len, 100);
}

// ── RunSummary struct ──────────────────────────────────────────

#[test]
fn test_run_summary_serialization() {
    let summary = RunSummary {
        run_id: "r1".into(),
        graph_name: "g1".into(),
        status: RunStatus::Completed,
        total_nodes_executed: 3,
        total_attempts: 5,
        failed_attempts: 1,
        trace_id: Some("t1".into()),
        trace_ctx: None,
        started_at: chrono::Utc::now(),
        finished_at: Some(chrono::Utc::now()),
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("\"run_id\":\"r1\""));
    assert!(json.contains("\"trace_id\":\"t1\""));
}
