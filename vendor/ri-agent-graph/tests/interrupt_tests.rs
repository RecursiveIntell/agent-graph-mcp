use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_interrupt_before() {
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
        .add_edge("step1", "step2")
        .with_interrupt_before(vec!["step2".to_string()])
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step1", state).await;

    // Should be interrupted before step2
    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::InterruptError { node, .. } => {
            assert_eq!(node, "step2");
        }
        other => panic!("Expected InterruptError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_interrupt_after() {
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
        .add_edge("step1", "step2")
        .with_interrupt_after(vec!["step1".to_string()])
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step1", state).await;

    // Should be interrupted after step1
    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::InterruptError { node, .. } => {
            assert_eq!(node, "step1");
        }
        other => panic!("Expected InterruptError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_execute_with_interrupt_complete() {
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("done", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph
        .execute_with_interrupt("step1", state, GraphConfig::default())
        .await;

    match result {
        ExecutionResult::Complete(state) => {
            assert!(state.get::<bool>("done").await.unwrap());
        }
        ExecutionResult::Interrupted { .. } => {
            panic!("Should have completed, not interrupted");
        }
        ExecutionResult::Failed { error, .. } => {
            panic!("Should have completed, but failed: {error}");
        }
    }
}

#[tokio::test]
async fn test_execute_with_interrupt_interrupted() {
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
        .add_edge("step1", "step2")
        .with_interrupt_before(vec!["step2".to_string()])
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph
        .execute_with_interrupt("step1", state, GraphConfig::default())
        .await;

    match result {
        ExecutionResult::Interrupted {
            node,
            checkpoint_data,
            ..
        } => {
            assert_eq!(node, "step2");
            assert!(checkpoint_data.is_some());
            let cp = checkpoint_data.unwrap();
            assert_eq!(cp.resume_node, "step2");
        }
        ExecutionResult::Complete(_) => {
            panic!("Should have been interrupted, not completed");
        }
        ExecutionResult::Failed { error, .. } => {
            panic!("Should have been interrupted, but failed: {error}");
        }
    }
}

#[tokio::test]
async fn test_resume_after_interrupt() {
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
        .add_edge("step1", "step2")
        .with_interrupt_before(vec!["step2".to_string()])
        .build()
        .unwrap();

    // First execution - gets interrupted
    let state = AgentState::new();
    let result = graph.execute("step1", state.clone()).await;
    assert!(result.is_err());

    // Resume from step2 (without interrupt config, step2 would be a standalone builder)
    // For this test, create a graph without the interrupt to resume
    let resume_graph = AgentGraph::builder()
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("step2_done", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let checkpoint = InterruptCheckpoint {
        resume_node: "step2".to_string(),
        resume_before: true,
        iteration: 1,
        active_nodes: vec!["step2".to_string()],
        graph_hash: None,
    };

    let result = resume_graph
        .resume(state, GraphConfig::default(), checkpoint)
        .await
        .unwrap();

    assert!(result.get::<bool>("step2_done").await.unwrap());
}

#[tokio::test]
async fn test_dynamic_interrupt_from_node() {
    // Node can trigger an interrupt by returning the interrupt error
    let graph = AgentGraph::builder()
        .add_node(
            "review",
            node!(|state| async move {
                let needs_review: bool = state.get_opt("needs_review").await?.unwrap_or(false);
                if needs_review {
                    return Err(ri_agent_graph::error::interrupt(
                        "review",
                        Some(serde_json::json!({"reason": "manual review needed"})),
                    ));
                }
                state.set("reviewed", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    // Without needs_review - completes normally
    let state = AgentState::new();
    let result = graph.execute("review", state).await;
    assert!(result.is_ok());

    // With needs_review - triggers interrupt
    let state = AgentState::new();
    state.set("needs_review", true).await.unwrap();
    let result = graph.execute("review", state).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::InterruptError { node, value } => {
            assert_eq!(node, "review");
            assert!(value.is_some());
        }
        other => panic!("Expected InterruptError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_interrupt_config_builder() {
    let config = InterruptConfig::new()
        .before("step1")
        .before("step2")
        .after("step3");

    assert!(config.should_interrupt_before("step1"));
    assert!(config.should_interrupt_before("step2"));
    assert!(!config.should_interrupt_before("step3"));
    assert!(config.should_interrupt_after("step3"));
    assert!(!config.should_interrupt_after("step1"));
    assert!(!config.is_empty());
}
