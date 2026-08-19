use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_research_pattern() {
    // Simulates the research agent pattern: generate -> search -> evaluate -> loop
    let graph = AgentGraph::builder()
        .add_node(
            "generate",
            node!(|state| async move {
                let iteration: usize = state.get_opt("iteration").await?.unwrap_or(0);
                let queries = vec![format!("query_{}", iteration)];
                state.set("queries", queries).await?;
                state.set("iteration", iteration + 1).await?;
                Ok(())
            }),
        )
        .add_node(
            "search",
            node!(|state| async move {
                let queries: Vec<String> = state.get("queries").await?;
                let mut findings: Vec<String> =
                    state.get_opt("findings").await?.unwrap_or_default();
                findings.extend(queries);
                state.set("findings", findings).await?;
                Ok(())
            }),
        )
        .add_node(
            "evaluate",
            node!(|state| async move {
                let findings: Vec<String> = state.get("findings").await?;
                state.set("sufficient", findings.len() >= 3).await?;
                Ok(())
            }),
        )
        .add_edge("generate", "search")
        .add_edge("search", "evaluate")
        .add_conditional_edge(
            "evaluate",
            router!(|state| async move {
                let sufficient: bool = state.get("sufficient").await?;
                if sufficient {
                    Ok(None)
                } else {
                    Ok(Some("generate".to_string()))
                }
            }),
        )
        .with_max_iterations(20)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("generate", state).await.unwrap();

    let findings: Vec<String> = result.get("findings").await.unwrap();
    assert!(findings.len() >= 3);

    let iterations: usize = result.get("iteration").await.unwrap();
    assert_eq!(iterations, 3);
}

#[tokio::test]
async fn test_quality_loop_pattern() {
    // Simulates quality refinement: generate -> evaluate -> loop until good
    let graph = AgentGraph::builder()
        .add_node(
            "generate",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("attempt").await?.unwrap_or(0);
                let quality = 0.3 + (attempt as f64 * 0.25);
                state.set("quality", quality).await?;
                state.set("attempt", attempt + 1).await?;
                Ok(())
            }),
        )
        .add_node(
            "evaluate",
            node!(|state| async move {
                let quality: f64 = state.get("quality").await?;
                state.set("approved", quality >= 0.8).await?;
                Ok(())
            }),
        )
        .add_edge("generate", "evaluate")
        .add_conditional_edge(
            "evaluate",
            router!(|state| async move {
                let approved: bool = state.get("approved").await?;
                if approved {
                    Ok(None)
                } else {
                    Ok(Some("generate".to_string()))
                }
            }),
        )
        .with_max_iterations(20)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("generate", state).await.unwrap();

    let quality: f64 = result.get("quality").await.unwrap();
    assert!(quality >= 0.8);

    let approved: bool = result.get("approved").await.unwrap();
    assert!(approved);
}

#[tokio::test]
async fn test_multi_branch_graph() {
    // A graph that branches and processes differently based on type
    let graph = AgentGraph::builder()
        .add_node(
            "classify",
            node!(|state| async move {
                let input: String = state.get("input").await?;
                let category = if input.contains("error") {
                    "error"
                } else if input.contains("warn") {
                    "warning"
                } else {
                    "info"
                };
                state.set("category", category).await?;
                Ok(())
            }),
        )
        .add_node(
            "handle_error",
            node!(|state| async move {
                state.set("action", "escalate").await?;
                state.set("priority", "high").await?;
                Ok(())
            }),
        )
        .add_node(
            "handle_warning",
            node!(|state| async move {
                state.set("action", "log").await?;
                state.set("priority", "medium").await?;
                Ok(())
            }),
        )
        .add_node(
            "handle_info",
            node!(|state| async move {
                state.set("action", "ignore").await?;
                state.set("priority", "low").await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "classify",
            router!(|state| async move {
                let category: String = state.get("category").await?;
                Ok(Some(
                    match category.as_str() {
                        "error" => "handle_error",
                        "warning" => "handle_warning",
                        _ => "handle_info",
                    }
                    .to_string(),
                ))
            }),
        )
        .build()
        .unwrap();

    // Test error path
    let state = AgentState::new();
    state.set("input", "critical error occurred").await.unwrap();
    let result = graph.execute("classify", state).await.unwrap();
    assert_eq!(result.get::<String>("action").await.unwrap(), "escalate");
    assert_eq!(result.get::<String>("priority").await.unwrap(), "high");

    // Test warning path
    let state = AgentState::new();
    state.set("input", "disk space warn level").await.unwrap();
    let result = graph.execute("classify", state).await.unwrap();
    assert_eq!(result.get::<String>("action").await.unwrap(), "log");
    assert_eq!(result.get::<String>("priority").await.unwrap(), "medium");

    // Test info path
    let state = AgentState::new();
    state.set("input", "system healthy").await.unwrap();
    let result = graph.execute("classify", state).await.unwrap();
    assert_eq!(result.get::<String>("action").await.unwrap(), "ignore");
    assert_eq!(result.get::<String>("priority").await.unwrap(), "low");
}

#[tokio::test]
async fn test_state_accumulation() {
    // Test that state accumulates correctly across many nodes
    let graph = AgentGraph::builder()
        .add_node(
            "step",
            node!(|state| async move {
                let mut log: Vec<String> = state.get_opt("log").await?.unwrap_or_default();
                let count = log.len();
                log.push(format!("step_{}", count));
                state.set("log", log).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "step",
            router!(|state| async move {
                let log: Vec<String> = state.get("log").await?;
                if log.len() >= 5 {
                    Ok(None)
                } else {
                    Ok(Some("step".to_string()))
                }
            }),
        )
        .with_max_iterations(10)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step", state).await.unwrap();

    let log: Vec<String> = result.get("log").await.unwrap();
    assert_eq!(log, vec!["step_0", "step_1", "step_2", "step_3", "step_4"]);
}

#[tokio::test]
async fn test_snapshot_during_execution() {
    // Test taking snapshots during graph execution
    let graph = AgentGraph::builder()
        .add_node(
            "node1",
            node!(|state| async move {
                state.set("value", 1i32).await?;
                state.save_to_history().await;
                Ok(())
            }),
        )
        .add_node(
            "node2",
            node!(|state| async move {
                state.set("value", 2i32).await?;
                state.save_to_history().await;
                Ok(())
            }),
        )
        .add_edge("node1", "node2")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("node1", state).await.unwrap();

    let history = result.get_history().await;
    assert_eq!(history.len(), 2);

    // First snapshot should have value=1
    let snap1_value: i32 =
        serde_json::from_value(history[0].data.get("value").unwrap().clone()).unwrap();
    assert_eq!(snap1_value, 1);

    // Second snapshot should have value=2
    let snap2_value: i32 =
        serde_json::from_value(history[1].data.get("value").unwrap().clone()).unwrap();
    assert_eq!(snap2_value, 2);
}

#[cfg(feature = "checkpointing")]
#[tokio::test]
async fn test_checkpoint_save_load() {
    let db_path = "/tmp/test_ri_agent_graph_checkpoint.db";

    // Clean up first
    std::fs::remove_file(db_path).ok();

    let manager = CheckpointManager::new(db_path).unwrap();

    // Create a state and checkpoint
    let state = AgentState::new();
    state.set("key", "value").await.unwrap();
    state.set("count", 42i32).await.unwrap();

    let checkpoint = Checkpoint {
        execution_id: "test-exec-1".to_string(),
        timestamp: chrono::Utc::now(),
        current_node: "node_a".to_string(),
        iteration: 5,
        state: state.snapshot().await,
        step_number: 0,
        active_nodes: Vec::new(),
    };

    manager.save(&checkpoint).unwrap();

    // Load it back
    let loaded = manager.load("test-exec-1").unwrap().unwrap();
    assert_eq!(loaded.execution_id, "test-exec-1");
    assert_eq!(loaded.current_node, "node_a");
    assert_eq!(loaded.iteration, 5);

    // Restore state from checkpoint
    let restored_state = AgentState::new();
    restored_state.restore(&loaded.state).await;

    let key: String = restored_state.get("key").await.unwrap();
    let count: i32 = restored_state.get("count").await.unwrap();
    assert_eq!(key, "value");
    assert_eq!(count, 42);

    // Clean up
    manager.clear("test-exec-1").unwrap();
    let empty = manager.load("test-exec-1").unwrap();
    assert!(empty.is_none());

    std::fs::remove_file(db_path).ok();
}
