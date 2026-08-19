//! Tests for the new runtime features:
//! - PayloadNode execution
//! - JoinNode fan-in
//! - EventSink events
//! - CheckpointStore recording
//! - Executor abstraction
//! - Interrupt/resume roundtrip
//! - Cancellation
//! - Router branching
//! - Loop termination

use ri_agent_graph::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ---------- Mock Payload ----------

struct MockPayload {
    response: Value,
}

impl MockPayload {
    fn new(response: Value) -> Self {
        Self { response }
    }
}

impl Payload for MockPayload {
    fn invoke(
        &self,
        _input: Value,
        _ctx: &PayloadContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<PayloadOutput, PayloadError>> + Send + '_>>
    {
        let response = self.response.clone();
        Box::pin(async move {
            Ok(PayloadOutput {
                value: response,
                meta: HashMap::new(),
            })
        })
    }
}

struct StreamingPayload {
    tokens: Vec<String>,
    final_value: Value,
}

impl Payload for StreamingPayload {
    fn invoke(
        &self,
        _input: Value,
        ctx: &PayloadContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<PayloadOutput, PayloadError>> + Send + '_>>
    {
        let tokens = self.tokens.clone();
        let final_value = self.final_value.clone();
        let on_token = ctx.on_token.clone();
        Box::pin(async move {
            for token in &tokens {
                if let Some(ref callback) = on_token {
                    callback(token);
                }
            }
            Ok(PayloadOutput {
                value: final_value,
                meta: HashMap::new(),
            })
        })
    }
}

struct FailingPayload {
    message: String,
}

impl Payload for FailingPayload {
    fn invoke(
        &self,
        _input: Value,
        _ctx: &PayloadContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<PayloadOutput, PayloadError>> + Send + '_>>
    {
        let msg = self.message.clone();
        Box::pin(async move { Err(msg.into()) })
    }
}

// ---------- PayloadNode Tests ----------

#[tokio::test]
async fn test_payload_node_basic() {
    let payload = MockPayload::new(json!({"result": "hello", "count": 42}));
    let node = PayloadNode::new(Box::new(payload));

    let graph = AgentGraph::builder()
        .add_node("payload", Box::new(node))
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("payload", state).await.unwrap();

    let result_val: String = result.get("result").await.unwrap();
    assert_eq!(result_val, "hello");
    let count: i64 = result.get("count").await.unwrap();
    assert_eq!(count, 42);
}

#[tokio::test]
async fn test_payload_node_with_input_selector() {
    // Payload that echoes input
    struct EchoPayload;
    impl Payload for EchoPayload {
        fn invoke(
            &self,
            input: Value,
            _ctx: &PayloadContext,
        ) -> Pin<
            Box<dyn Future<Output = std::result::Result<PayloadOutput, PayloadError>> + Send + '_>,
        > {
            Box::pin(async move {
                Ok(PayloadOutput {
                    value: json!({"echo": input}),
                    meta: HashMap::new(),
                })
            })
        }
    }

    let node = PayloadNode::new(Box::new(EchoPayload))
        .with_input_selector(|state| {
            // Only pass the "query" key
            state
                .get("query")
                .cloned()
                .unwrap_or(Value::String("no query".into()))
        })
        .with_name("echo");

    let graph = AgentGraph::builder()
        .add_node("echo", Box::new(node))
        .build()
        .unwrap();

    let state = AgentState::new();
    state.set("query", "test query").await.unwrap();
    state
        .set("irrelevant", "should not be passed")
        .await
        .unwrap();

    let result = graph.execute("echo", state).await.unwrap();
    let echo: Value = result.get("echo").await.unwrap();
    assert_eq!(echo, json!("test query"));
}

#[tokio::test]
async fn test_payload_node_with_output_mapper() {
    let payload = MockPayload::new(json!({"generated": "content"}));
    let node = PayloadNode::new(Box::new(payload)).with_output_mapper(|_state, output| {
        // Only keep the "generated" key under "result"
        json!({"result": output.value.get("generated").cloned().unwrap_or(Value::Null)})
    });

    let graph = AgentGraph::builder()
        .add_node("mapped", Box::new(node))
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("mapped", state).await.unwrap();

    let result_val: String = result.get("result").await.unwrap();
    assert_eq!(result_val, "content");
    // "generated" should not be in state (mapper doesn't include it directly)
    assert!(result
        .get_opt::<Value>("generated")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_payload_node_error_propagation() {
    let payload = FailingPayload {
        message: "LLM API error".into(),
    };
    let node = PayloadNode::new(Box::new(payload));

    let graph = AgentGraph::builder()
        .add_node("failing", Box::new(node))
        .build()
        .unwrap();

    let state = AgentState::new();
    let err = graph.execute("failing", state).await.unwrap_err();
    assert!(matches!(err, AgentGraphError::PayloadError(_)));
    assert!(err.to_string().contains("LLM API error"));
}

#[tokio::test]
async fn test_payload_node_in_chain() {
    // Test: fn_node -> payload_node -> fn_node
    let payload = MockPayload::new(json!({"step2_done": true}));

    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("step1_done", true).await?;
                Ok(())
            }),
        )
        .add_node("step2", Box::new(PayloadNode::new(Box::new(payload))))
        .add_node(
            "step3",
            node!(|state| async move {
                let s1: bool = state.get("step1_done").await?;
                let s2: bool = state.get("step2_done").await?;
                state.set("all_done", s1 && s2).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .add_edge("step2", "step3")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step1", state).await.unwrap();

    let all_done: bool = result.get("all_done").await.unwrap();
    assert!(all_done);
}

// ---------- JoinNode Tests ----------

#[tokio::test]
async fn test_join_node_collect_array() {
    let join = JoinNode::collect_array(vec!["branch_a".into(), "branch_b".into()], "merged");

    let graph = AgentGraph::builder()
        .add_node(
            "branch_a_node",
            node!(|state| async move {
                state.set("branch_a", json!({"result": "from_a"})).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_b_node",
            node!(|state| async move {
                state.set("branch_b", json!({"result": "from_b"})).await?;
                Ok(())
            }),
        )
        .add_node("join", Box::new(join))
        // Fan-out from start
        .add_edge(START, "branch_a_node")
        .add_edge(START, "branch_b_node")
        // Fan-in to join
        .add_edge("branch_a_node", "join")
        .add_edge("branch_b_node", "join")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute(START, state).await.unwrap();

    let merged: Vec<Value> = result.get("merged").await.unwrap();
    assert_eq!(merged.len(), 2);
    // Check both values are present (order may vary due to parallel execution)
    let has_a = merged
        .iter()
        .any(|v| v.get("result") == Some(&json!("from_a")));
    let has_b = merged
        .iter()
        .any(|v| v.get("result") == Some(&json!("from_b")));
    assert!(has_a, "Missing branch_a result");
    assert!(has_b, "Missing branch_b result");
}

#[tokio::test]
async fn test_join_node_merge_objects() {
    let join = JoinNode::merge_objects(vec!["result_x".into(), "result_y".into()], "combined");

    let graph = AgentGraph::builder()
        .add_node(
            "x",
            node!(|state| async move {
                state.set("result_x", json!({"x_key": "x_val"})).await?;
                Ok(())
            }),
        )
        .add_node(
            "y",
            node!(|state| async move {
                state.set("result_y", json!({"y_key": "y_val"})).await?;
                Ok(())
            }),
        )
        .add_node("join", Box::new(join))
        .add_edge(START, "x")
        .add_edge(START, "y")
        .add_edge("x", "join")
        .add_edge("y", "join")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute(START, state).await.unwrap();

    let combined: Value = result.get("combined").await.unwrap();
    assert_eq!(combined.get("x_key"), Some(&json!("x_val")));
    assert_eq!(combined.get("y_key"), Some(&json!("y_val")));
}

#[tokio::test]
async fn test_join_node_custom_merge() {
    let join = JoinNode::new(
        vec!["count_a".into(), "count_b".into()],
        "total",
        |values| {
            let sum: i64 = values.into_iter().filter_map(|(_, v)| v.as_i64()).sum();
            Ok(json!(sum))
        },
    );

    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("count_a", 10i64).await?;
                Ok(())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("count_b", 20i64).await?;
                Ok(())
            }),
        )
        .add_node("join", Box::new(join))
        .add_edge(START, "a")
        .add_edge(START, "b")
        .add_edge("a", "join")
        .add_edge("b", "join")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute(START, state).await.unwrap();

    let total: i64 = result.get("total").await.unwrap();
    assert_eq!(total, 30);
}

// ---------- EventSink Tests ----------

#[tokio::test]
async fn test_event_sink_captures_events() {
    let events: Arc<Mutex<Vec<GraphEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let sink = CallbackEventSink::new(move |event| {
        events_clone.lock().unwrap().push(event);
    });

    let graph = AgentGraph::builder()
        .with_event_sink(Arc::new(sink))
        .add_node(
            "step",
            node!(|state| async move {
                state.set("done", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    graph.execute("step", state).await.unwrap();

    let captured = events.lock().unwrap();
    // Should have RunStart, SuperstepStart, NodeStart, StateUpdate, NodeEnd, SuperstepEnd, RunEnd
    assert!(
        captured
            .iter()
            .any(|e| matches!(e, GraphEvent::RunStart { .. })),
        "Missing RunStart"
    );
    assert!(
        captured
            .iter()
            .any(|e| matches!(e, GraphEvent::NodeStart { .. })),
        "Missing NodeStart"
    );
    assert!(
        captured
            .iter()
            .any(|e| matches!(e, GraphEvent::NodeEnd { .. })),
        "Missing NodeEnd"
    );
    assert!(
        captured
            .iter()
            .any(|e| matches!(e, GraphEvent::RunEnd { .. })),
        "Missing RunEnd"
    );
    assert!(
        captured
            .iter()
            .any(|e| matches!(e, GraphEvent::StateUpdate { .. })),
        "Missing StateUpdate"
    );
}

#[tokio::test]
async fn test_event_sink_with_stream_compat() {
    // Using stream() still works and events flow through
    let graph = Arc::new(
        AgentGraph::builder()
            .add_node(
                "step",
                node!(|state| async move {
                    state.set("v", 1).await?;
                    Ok(())
                }),
            )
            .build()
            .unwrap(),
    );

    let (handle, mut rx) = graph.stream("step", AgentState::new(), GraphConfig::default());

    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    handle.await.unwrap().unwrap();

    // Legacy StreamEvent format should still work
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::NodeStart { node } if node == "step")));
}

// ---------- CheckpointStore Tests ----------

#[tokio::test]
async fn test_checkpoint_store_records_attempts() {
    let store = Arc::new(InMemoryCheckpointStore::new());
    let store_clone = store.clone();

    let graph = AgentGraph::builder()
        .with_checkpoint_store(store.clone())
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("a", 1).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("b", 2).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .build()
        .unwrap();

    let state = AgentState::new();
    graph.execute("step1", state).await.unwrap();

    // Find the run (there should be exactly one)
    let runs = store_clone.list_runs().await;
    assert_eq!(runs.len(), 1);

    let run_state = &runs[0];
    assert_eq!(run_state.status, RunStatus::Completed);
    assert!(!run_state.run_id.is_empty());

    // Should have 2 attempts (step1 and step2)
    assert_eq!(run_state.attempts.len(), 2);
    assert!(run_state
        .attempts
        .iter()
        .all(|a| a.status == AttemptStatus::Completed));
}

#[tokio::test]
async fn test_checkpoint_store_state_snapshots() {
    let store = Arc::new(InMemoryCheckpointStore::new());
    let store_clone = store.clone();

    let graph = AgentGraph::builder()
        .with_checkpoint_store(store.clone())
        .add_node(
            "step",
            node!(|state| async move {
                state.set("key", "value").await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    graph.execute("step", AgentState::new()).await.unwrap();

    let runs = store_clone.list_runs().await;
    let run_state = &runs[0];
    // State snapshot should include the key set by the node
    assert!(run_state.state_snapshot.contains_key("key"));
    assert_eq!(run_state.state_snapshot["key"], json!("value"));
}

// ---------- Executor Tests ----------

#[tokio::test]
async fn test_custom_executor() {
    // Track how many times execute_node is called
    let call_count = Arc::new(AtomicUsize::new(0));

    struct CountingExecutor {
        count: Arc<AtomicUsize>,
    }

    impl Executor for CountingExecutor {
        fn execute_node(
            &self,
            node: Arc<dyn Node>,
            state: AgentState,
            config: GraphConfig,
        ) -> Pin<Box<dyn Future<Output = Result<NodeOutput>> + Send>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move { node.execute(&state, &config).await })
        }
    }

    let executor = Arc::new(CountingExecutor {
        count: call_count.clone(),
    });

    let graph = AgentGraph::builder()
        .with_executor(executor)
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("a", 1).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("b", 2).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step1", state).await.unwrap();

    // Executor was called for both nodes
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    let a: i32 = result.get("a").await.unwrap();
    let b: i32 = result.get("b").await.unwrap();
    assert_eq!(a, 1);
    assert_eq!(b, 2);
}

// ---------- Router Branching Tests ----------

#[tokio::test]
async fn test_router_branching_with_payload() {
    let high_payload = MockPayload::new(json!({"branch": "high"}));
    let low_payload = MockPayload::new(json!({"branch": "low"}));

    let graph = AgentGraph::builder()
        .add_node(
            "classify",
            node!(|state| async move {
                state.set("score", 80).await?;
                Ok(())
            }),
        )
        .add_node(
            "high_handler",
            Box::new(PayloadNode::new(Box::new(high_payload))),
        )
        .add_node(
            "low_handler",
            Box::new(PayloadNode::new(Box::new(low_payload))),
        )
        .add_conditional_edge(
            "classify",
            router!(|state| async move {
                let score: i32 = state.get("score").await?;
                Ok(Some(
                    if score >= 50 {
                        "high_handler"
                    } else {
                        "low_handler"
                    }
                    .to_string(),
                ))
            }),
        )
        .build()
        .unwrap();

    let result = graph.execute("classify", AgentState::new()).await.unwrap();
    let branch: String = result.get("branch").await.unwrap();
    assert_eq!(branch, "high");
}

// ---------- Loop Termination Tests ----------

#[tokio::test]
async fn test_loop_with_max_steps() {
    let graph = AgentGraph::builder()
        .with_max_iterations(5)
        .add_node(
            "counter",
            node!(|state| async move {
                let count: i32 = state.get_opt("count").await?.unwrap_or(0);
                state.set("count", count + 1).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "counter",
            router!(|state| async move {
                let count: i32 = state.get("count").await?;
                if count >= 100 {
                    Ok(None) // will never trigger, max_iterations catches first
                } else {
                    Ok(Some("counter".to_string()))
                }
            }),
        )
        .build()
        .unwrap();

    let err = graph
        .execute("counter", AgentState::new())
        .await
        .unwrap_err();
    assert!(matches!(err, AgentGraphError::MaxIterationsExceeded { .. }));
}

#[tokio::test]
async fn test_loop_with_explicit_termination() {
    let graph = AgentGraph::builder()
        .add_node(
            "counter",
            node!(|state| async move {
                let count: i32 = state.get_opt("count").await?.unwrap_or(0);
                state.set("count", count + 1).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "counter",
            router!(|state| async move {
                let count: i32 = state.get("count").await?;
                if count >= 3 {
                    Ok(None) // end
                } else {
                    Ok(Some("counter".to_string()))
                }
            }),
        )
        .build()
        .unwrap();

    let result = graph.execute("counter", AgentState::new()).await.unwrap();
    let count: i32 = result.get("count").await.unwrap();
    assert_eq!(count, 3);
}

// ---------- Fan-out / Fan-in Deterministic Join ----------

#[tokio::test]
async fn test_fan_out_fan_in_deterministic() {
    // Run this multiple times to check determinism
    for _ in 0..5 {
        let join = JoinNode::collect_array(vec!["r1".into(), "r2".into(), "r3".into()], "results");

        let graph = AgentGraph::builder()
            .add_node(
                "n1",
                node!(|state| async move {
                    state.set("r1", json!("result_1")).await?;
                    Ok(())
                }),
            )
            .add_node(
                "n2",
                node!(|state| async move {
                    state.set("r2", json!("result_2")).await?;
                    Ok(())
                }),
            )
            .add_node(
                "n3",
                node!(|state| async move {
                    state.set("r3", json!("result_3")).await?;
                    Ok(())
                }),
            )
            .add_node("join", Box::new(join))
            .add_edge(START, "n1")
            .add_edge(START, "n2")
            .add_edge(START, "n3")
            .add_edge("n1", "join")
            .add_edge("n2", "join")
            .add_edge("n3", "join")
            .build()
            .unwrap();

        let result = graph.execute(START, AgentState::new()).await.unwrap();
        let results: Vec<Value> = result.get("results").await.unwrap();
        assert_eq!(results.len(), 3);
        // All three results should be present
        assert!(results.contains(&json!("result_1")));
        assert!(results.contains(&json!("result_2")));
        assert!(results.contains(&json!("result_3")));
    }
}

// ---------- Interrupt / Resume Roundtrip ----------

#[tokio::test]
async fn test_interrupt_resume_roundtrip() {
    // Phase 1: Build a graph with interrupt_before on step3.
    // This means: step1 runs, step2 runs, then before step3 execution is interrupted.
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("step1_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("step2_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "step3",
            node!(|state| async move {
                let human_input: String = state.get("human_input").await?;
                state
                    .set("final", format!("completed with: {}", human_input))
                    .await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .add_edge("step2", "step3")
        .with_interrupt_before(vec!["step3".to_string()])
        .build()
        .unwrap();

    let config = GraphConfig::new().with_thread_id("test-thread");

    // Execute until interrupt (should interrupt before step3)
    let exec_result = graph
        .execute_with_interrupt("step1", AgentState::new(), config.clone())
        .await;

    let (interrupted_state, checkpoint) = match exec_result {
        ExecutionResult::Interrupted {
            state,
            checkpoint_data,
            node,
            ..
        } => {
            assert_eq!(node, "step3");
            (state, checkpoint_data.unwrap())
        }
        ExecutionResult::Complete(_) => panic!("Expected interrupt"),
        ExecutionResult::Failed { error, .. } => panic!("Expected interrupt, got failure: {error}"),
    };

    // Verify partial execution: step1 and step2 should have run
    let step1_done: bool = interrupted_state.get("step1_done").await.unwrap();
    assert!(step1_done);

    // Phase 2: inject user input and resume using a graph WITHOUT the interrupt config
    // (to avoid re-triggering the same interrupt).
    interrupted_state
        .set("human_input", "approved")
        .await
        .unwrap();

    let resume_graph = AgentGraph::builder()
        .add_node(
            "step3",
            node!(|state| async move {
                let human_input: String = state.get("human_input").await?;
                state
                    .set("final", format!("completed with: {}", human_input))
                    .await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let final_state = resume_graph
        .resume_force(interrupted_state, config, checkpoint)
        .await
        .unwrap();

    let final_val: String = final_state.get("final").await.unwrap();
    assert_eq!(final_val, "completed with: approved");
}

// ---------- Cancellation ----------

#[tokio::test]
async fn test_cancellation_stops_execution() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let exec_count_clone = execution_count.clone();

    let graph = Arc::new(
        AgentGraph::builder()
            .add_node(
                "slow",
                node!(|state| async move {
                    state.set("started", true).await?;
                    // Simulate work
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    Ok(())
                }),
            )
            .add_conditional_edge(
                "slow",
                router!(|_state| async move { Ok(Some("slow".to_string())) }),
            )
            .with_max_iterations(100)
            .build()
            .unwrap(),
    );

    let (handle, cancel_flag) =
        graph.execute_cancellable("slow", AgentState::new(), GraphConfig::default());

    // Let it run for a bit
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Cancel
    cancel_flag.store(true, Ordering::Relaxed);

    let result = handle.await.unwrap();
    assert!(
        matches!(result, Err(AgentGraphError::Cancelled)),
        "Expected Cancelled error, got: {:?}",
        result
    );
    let _ = exec_count_clone;
}

// ---------- Composite Event Sink ----------

#[tokio::test]
async fn test_composite_event_sink() {
    let events1: Arc<Mutex<Vec<GraphEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events2: Arc<Mutex<Vec<GraphEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let e1 = events1.clone();
    let e2 = events2.clone();

    let sink1 = Arc::new(CallbackEventSink::new(move |event| {
        e1.lock().unwrap().push(event);
    }));
    let sink2 = Arc::new(CallbackEventSink::new(move |event| {
        e2.lock().unwrap().push(event);
    }));
    let composite = Arc::new(CompositeEventSink::new(vec![sink1, sink2]));

    let graph = AgentGraph::builder()
        .with_event_sink(composite)
        .add_node(
            "step",
            node!(|state| async move {
                state.set("v", 1).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    graph.execute("step", AgentState::new()).await.unwrap();

    // Both sinks should have received the same events
    let len1 = events1.lock().unwrap().len();
    let len2 = events2.lock().unwrap().len();
    assert!(len1 > 0);
    assert_eq!(len1, len2);
}

// ---------- Token Streaming via PayloadContext ----------

#[tokio::test]
async fn test_payload_token_streaming() {
    let payload = StreamingPayload {
        tokens: vec!["Hello".into(), " ".into(), "world".into()],
        final_value: json!({"text": "Hello world"}),
    };

    // Test that the payload receives and calls the token sink
    let collected_tokens: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tokens_clone = collected_tokens.clone();

    let on_token: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |token: &str| {
        tokens_clone.lock().unwrap().push(token.to_string());
    });

    let ctx = PayloadContext {
        on_token: Some(on_token),
        run_id: "test-run".into(),
        node_id: "test-node".into(),
    };

    let result = payload.invoke(json!({}), &ctx).await.unwrap();
    assert_eq!(result.value, json!({"text": "Hello world"}));

    let tokens = collected_tokens.lock().unwrap();
    assert_eq!(*tokens, vec!["Hello", " ", "world"]);
}

// ---------- InProcessExecutor ----------

#[tokio::test]
async fn test_in_process_executor() {
    let executor = Arc::new(InProcessExecutor::new());

    let graph = AgentGraph::builder()
        .with_executor(executor)
        .add_node(
            "step",
            node!(|state| async move {
                state.set("executed", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let result = graph.execute("step", AgentState::new()).await.unwrap();
    let executed: bool = result.get("executed").await.unwrap();
    assert!(executed);
}

// ---------- CheckpointStore on failure ----------

#[tokio::test]
async fn test_checkpoint_store_records_failure() {
    let store = Arc::new(InMemoryCheckpointStore::new());
    let store_clone = store.clone();

    let graph = AgentGraph::builder()
        .with_checkpoint_store(store.clone())
        .add_node(
            "failing",
            node!(
                |_state| async move { Err::<(), _>(AgentGraphError::ExecutionError("boom".into())) }
            ),
        )
        .build()
        .unwrap();

    let _ = graph.execute("failing", AgentState::new()).await;

    let runs = store_clone.list_runs().await;
    let run_state = &runs[0];
    assert_eq!(run_state.status, RunStatus::Failed);
    assert!(run_state
        .attempts
        .iter()
        .any(|a| a.status == AttemptStatus::Failed));
}
