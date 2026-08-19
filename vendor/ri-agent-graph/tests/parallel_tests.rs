use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_parallel_failure_cancels_delayed_side_effect_branch() {
    let graph = AgentGraph::builder()
        .add_node("start", node!(|_state| async move { Ok(()) }))
        .add_node(
            "fail",
            node!(|_state| async move {
                tokio::task::yield_now().await;
                Err::<(), _>(AgentGraphError::ExecutionError("first failure".into()))
            }),
        )
        .add_node(
            "delayed",
            node!(|state| async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                state.set("post_failure_effect", true).await?;
                Ok(())
            }),
        )
        .add_edge("start", "fail")
        .add_edge("start", "delayed")
        .build()
        .unwrap();

    let started = std::time::Instant::now();
    let result = graph.execute("start", AgentState::new()).await;
    assert!(
        matches!(result, Err(AgentGraphError::ExecutionError(message)) if message == "first failure")
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[tokio::test]
async fn test_fan_out_parallel_execution() {
    // A -> B, A -> C (fan-out), both B and C should execute
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("a_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("b_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "c",
            node!(|state| async move {
                state.set("c_done", true).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b")
        .add_edge("a", "c")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("a", state).await.unwrap();

    assert!(result.get::<bool>("a_done").await.unwrap());
    assert!(result.get::<bool>("b_done").await.unwrap());
    assert!(result.get::<bool>("c_done").await.unwrap());
}

#[tokio::test]
async fn test_fan_out_fan_in() {
    // A -> B, A -> C, B -> D, C -> D
    // D should execute after both B and C
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("value", 10i32).await?;
                Ok(())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("b_result", "from_b").await?;
                Ok(())
            }),
        )
        .add_node(
            "c",
            node!(|state| async move {
                state.set("c_result", "from_c").await?;
                Ok(())
            }),
        )
        .add_node(
            "d",
            node!(|state| async move {
                // D should see results from both B and C
                let b: String = state.get("b_result").await?;
                let c: String = state.get("c_result").await?;
                state.set("combined", format!("{} + {}", b, c)).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b")
        .add_edge("a", "c")
        .add_edge("b", "d")
        .add_edge("c", "d")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("a", state).await.unwrap();

    let combined: String = result.get("combined").await.unwrap();
    assert_eq!(combined, "from_b + from_c");
}

#[tokio::test]
async fn test_parallel_execution_actually_parallel() {
    // Verify that parallel branches actually run concurrently
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("started", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_a",
            node!(|state| async move {
                // Simulate some work
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                state.set("a_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_b",
            node!(|state| async move {
                // Simulate some work
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                state.set("b_done", true).await?;
                Ok(())
            }),
        )
        .add_edge("start", "branch_a")
        .add_edge("start", "branch_b")
        .build()
        .unwrap();

    let state = AgentState::new();
    let start = std::time::Instant::now();
    let result = graph.execute("start", state).await.unwrap();
    let elapsed = start.elapsed();

    // If truly parallel, both branches should complete in ~50ms, not ~100ms
    assert!(
        elapsed.as_millis() < 100,
        "Parallel branches took too long: {:?}",
        elapsed
    );

    assert!(result.get::<bool>("a_done").await.unwrap());
    assert!(result.get::<bool>("b_done").await.unwrap());
}

#[tokio::test]
async fn test_parallel_state_with_reducer() {
    // Two parallel branches both increment a counter
    // Without a reducer, last-write-wins; with AddReducer, both are added
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("count", 0i64).await?;
                Ok(())
            }),
        )
        .add_node(
            "add_one",
            node!(|state| async move {
                state.set("count", 1i64).await?;
                Ok(())
            }),
        )
        .add_node(
            "add_two",
            node!(|state| async move {
                state.set("count", 2i64).await?;
                Ok(())
            }),
        )
        .add_edge("start", "add_one")
        .add_edge("start", "add_two")
        .with_reducer("count", AddReducer)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("start", state).await.unwrap();

    // With AddReducer: 0 (snapshot) + 1 (add_one diff) + 2 (add_two diff) = 3
    let count: f64 = result.get("count").await.unwrap();
    assert_eq!(count, 3.0);
}

#[tokio::test]
async fn test_parallel_state_with_append_reducer() {
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("items", Vec::<String>::new()).await?;
                Ok(())
            }),
        )
        .add_node(
            "add_fruits",
            node!(|state| async move {
                state
                    .set("items", vec!["apple".to_string(), "banana".to_string()])
                    .await?;
                Ok(())
            }),
        )
        .add_node(
            "add_vegs",
            node!(|state| async move {
                state
                    .set("items", vec!["carrot".to_string(), "daikon".to_string()])
                    .await?;
                Ok(())
            }),
        )
        .add_edge("start", "add_fruits")
        .add_edge("start", "add_vegs")
        .with_reducer("items", AppendReducer)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("start", state).await.unwrap();

    let items: Vec<String> = result.get("items").await.unwrap();
    // Both branches should have their items appended
    assert_eq!(items.len(), 4);
    assert!(items.contains(&"apple".to_string()));
    assert!(items.contains(&"banana".to_string()));
    assert!(items.contains(&"carrot".to_string()));
    assert!(items.contains(&"daikon".to_string()));
}

#[tokio::test]
async fn test_start_end_constants() {
    let graph = AgentGraph::builder()
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
        .set_entry_point("step1")
        .add_edge("step1", "step2")
        .set_finish_point("step2")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute(START, state).await.unwrap();

    assert!(result.get::<bool>("step1").await.unwrap());
    assert!(result.get::<bool>("step2").await.unwrap());
}

#[tokio::test]
async fn test_command_goto() {
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("visited_a", true).await?;
                // Use command to skip to 'c', bypassing normal edges
                Ok(NodeOutput::goto("c"))
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("visited_b", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "c",
            node!(|state| async move {
                state.set("visited_c", true).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b") // Normal edge would go to b
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("a", state).await.unwrap();

    assert!(result.get::<bool>("visited_a").await.unwrap());
    // b should NOT have been visited (command overrides edge)
    assert!(result.get_opt::<bool>("visited_b").await.unwrap().is_none());
    assert!(result.get::<bool>("visited_c").await.unwrap());
}

#[tokio::test]
async fn test_command_end() {
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("visited_a", true).await?;
                Ok(NodeOutput::end())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                state.set("visited_b", true).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("a", state).await.unwrap();

    assert!(result.get::<bool>("visited_a").await.unwrap());
    assert!(result.get_opt::<bool>("visited_b").await.unwrap().is_none());
}

#[tokio::test]
async fn test_config_access_in_node() {
    let graph = AgentGraph::builder()
        .add_node(
            "read_config",
            node!(|state, config| async move {
                if let Some(thread_id) = &config.thread_id {
                    state.set("thread_id", thread_id.clone()).await?;
                }
                state
                    .set("recursion_limit", config.recursion_limit as i64)
                    .await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let config = GraphConfig::new()
        .with_thread_id("test-thread-42")
        .with_recursion_limit(50);
    let result = graph
        .execute_with_config("read_config", state, config)
        .await
        .unwrap();

    let thread_id: String = result.get("thread_id").await.unwrap();
    assert_eq!(thread_id, "test-thread-42");

    let limit: i64 = result.get("recursion_limit").await.unwrap();
    assert_eq!(limit, 50);
}

#[tokio::test]
async fn test_conditional_fan_out() {
    // Router returns multiple nodes for fan-out
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("started", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_a",
            node!(|state| async move {
                state.set("a_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_b",
            node!(|state| async move {
                state.set("b_done", true).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "start",
            router!(|_state| async move {
                Ok(RouterOutput::FanOut(vec![
                    "branch_a".to_string(),
                    "branch_b".to_string(),
                ]))
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("start", state).await.unwrap();

    assert!(result.get::<bool>("a_done").await.unwrap());
    assert!(result.get::<bool>("b_done").await.unwrap());
}
