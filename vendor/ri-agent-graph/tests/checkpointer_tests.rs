use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_memory_saver_basic() {
    let saver = MemorySaver::new();

    let state = AgentState::new();
    state.set("key", "value").await.unwrap();

    let checkpoint = ri_agent_graph::checkpoint::Checkpoint {
        execution_id: "thread-1".to_string(),
        timestamp: chrono::Utc::now(),
        current_node: "step1".to_string(),
        iteration: 0,
        state: state.snapshot().await,
        step_number: 0,
        active_nodes: vec!["step1".to_string()],
    };

    saver.save(&checkpoint).await.unwrap();

    let loaded = saver.load("thread-1").await.unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.execution_id, "thread-1");
    assert_eq!(loaded.current_node, "step1");
}

#[tokio::test]
async fn test_memory_saver_history() {
    let saver = MemorySaver::new();

    let state = AgentState::new();
    for i in 0..3 {
        state.set("step", i as i32).await.unwrap();
        let checkpoint = ri_agent_graph::checkpoint::Checkpoint {
            execution_id: "thread-1".to_string(),
            timestamp: chrono::Utc::now(),
            current_node: format!("step{}", i),
            iteration: i,
            state: state.snapshot().await,
            step_number: i,
            active_nodes: Vec::new(),
        };
        saver.save(&checkpoint).await.unwrap();
    }

    let history = saver.load_history("thread-1").await.unwrap();
    assert_eq!(history.len(), 3);

    // Latest should be step2
    let latest = saver.load("thread-1").await.unwrap().unwrap();
    assert_eq!(latest.current_node, "step2");
}

#[tokio::test]
async fn test_memory_saver_clear() {
    let saver = MemorySaver::new();

    let state = AgentState::new();
    let checkpoint = ri_agent_graph::checkpoint::Checkpoint {
        execution_id: "thread-1".to_string(),
        timestamp: chrono::Utc::now(),
        current_node: "step1".to_string(),
        iteration: 0,
        state: state.snapshot().await,
        step_number: 0,
        active_nodes: Vec::new(),
    };
    saver.save(&checkpoint).await.unwrap();

    saver.clear("thread-1").await.unwrap();
    let loaded = saver.load("thread-1").await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn test_memory_saver_multiple_threads() {
    let saver = MemorySaver::new();

    let state = AgentState::new();
    for thread_id in &["thread-1", "thread-2"] {
        state.set("thread", *thread_id).await.unwrap();
        let checkpoint = ri_agent_graph::checkpoint::Checkpoint {
            execution_id: thread_id.to_string(),
            timestamp: chrono::Utc::now(),
            current_node: "step1".to_string(),
            iteration: 0,
            state: state.snapshot().await,
            step_number: 0,
            active_nodes: Vec::new(),
        };
        saver.save(&checkpoint).await.unwrap();
    }

    let thread1 = saver.load("thread-1").await.unwrap();
    let thread2 = saver.load("thread-2").await.unwrap();
    assert!(thread1.is_some());
    assert!(thread2.is_some());

    // Clear thread-1 only
    saver.clear("thread-1").await.unwrap();
    assert!(saver.load("thread-1").await.unwrap().is_none());
    assert!(saver.load("thread-2").await.unwrap().is_some());
}

#[tokio::test]
async fn test_graph_with_checkpointer() {
    let saver = MemorySaver::new();

    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("value", 1i32).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                let v: i32 = state.get("value").await?;
                state.set("value", v + 1).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .with_checkpointer(saver)
        .build()
        .unwrap();

    let state = AgentState::new();
    let config = GraphConfig::new().with_thread_id("test-thread");
    let result = graph
        .execute_with_config("step1", state, config.clone())
        .await
        .unwrap();

    let value: i32 = result.get("value").await.unwrap();
    assert_eq!(value, 2);

    // Verify checkpoints were saved
    let history = graph.get_state_history(&config).await.unwrap();
    assert!(!history.is_empty());
}

#[tokio::test]
async fn test_graph_get_state() {
    let saver = MemorySaver::new();

    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("value", 42i32).await?;
                Ok(())
            }),
        )
        .with_checkpointer(saver)
        .build()
        .unwrap();

    let state = AgentState::new();
    let config = GraphConfig::new().with_thread_id("retrieve-test");
    graph
        .execute_with_config("step1", state, config.clone())
        .await
        .unwrap();

    // Retrieve state from checkpointer
    let saved_state = graph.get_state(&config).await.unwrap();
    assert!(saved_state.is_some());
    let saved = saved_state.unwrap();
    let value: i32 = saved.get("value").await.unwrap();
    assert_eq!(value, 42);
}

#[cfg(feature = "checkpointing")]
#[tokio::test]
async fn test_sqlite_saver_basic() {
    let db_path = "/tmp/test_sqlite_saver.db";
    std::fs::remove_file(db_path).ok();

    let saver = SqliteSaver::new(db_path).unwrap();

    let state = AgentState::new();
    state.set("key", "value").await.unwrap();

    let checkpoint = ri_agent_graph::checkpoint::Checkpoint {
        execution_id: "sqlite-thread-1".to_string(),
        timestamp: chrono::Utc::now(),
        current_node: "step1".to_string(),
        iteration: 0,
        state: state.snapshot().await,
        step_number: 0,
        active_nodes: Vec::new(),
    };

    saver.save(&checkpoint).await.unwrap();

    let loaded = saver.load("sqlite-thread-1").await.unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.current_node, "step1");

    saver.clear("sqlite-thread-1").await.unwrap();
    let empty = saver.load("sqlite-thread-1").await.unwrap();
    assert!(empty.is_none());

    std::fs::remove_file(db_path).ok();
}
