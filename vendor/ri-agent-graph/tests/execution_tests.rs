use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_simple_execution() {
    let graph = AgentGraph::builder()
        .add_node(
            "node1",
            node!(|state| async move {
                state.set("step1", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "node2",
            node!(|state| async move {
                state.set("step2", true).await?;
                Ok(())
            }),
        )
        .add_edge("node1", "node2")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("node1", state).await.unwrap();

    assert!(result.get::<bool>("step1").await.unwrap());
    assert!(result.get::<bool>("step2").await.unwrap());
}

#[tokio::test]
async fn test_three_node_chain() {
    let graph = AgentGraph::builder()
        .add_node(
            "a",
            node!(|state| async move {
                state.set("value", 1i32).await?;
                Ok(())
            }),
        )
        .add_node(
            "b",
            node!(|state| async move {
                let v: i32 = state.get("value").await?;
                state.set("value", v * 2).await?;
                Ok(())
            }),
        )
        .add_node(
            "c",
            node!(|state| async move {
                let v: i32 = state.get("value").await?;
                state.set("value", v + 10).await?;
                Ok(())
            }),
        )
        .add_edge("a", "b")
        .add_edge("b", "c")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("a", state).await.unwrap();

    let value: i32 = result.get("value").await.unwrap();
    assert_eq!(value, 12); // (1 * 2) + 10
}

#[tokio::test]
async fn test_single_node_no_edges() {
    let graph = AgentGraph::builder()
        .add_node(
            "solo",
            node!(|state| async move {
                state.set("done", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("solo", state).await.unwrap();

    assert!(result.get::<bool>("done").await.unwrap());
}

#[tokio::test]
async fn test_loop_execution() {
    let graph = AgentGraph::builder()
        .add_node(
            "increment",
            node!(|state| async move {
                let count: u32 = state.get_opt("count").await?.unwrap_or(0);
                state.set("count", count + 1).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "increment",
            router!(|state| async move {
                let count: u32 = state.get("count").await?;
                Ok(if count < 5 {
                    Some("increment".to_string())
                } else {
                    None
                })
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("increment", state).await.unwrap();

    let count: u32 = result.get("count").await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_max_iterations() {
    let graph = AgentGraph::builder()
        .add_node("infinite", node!(|_state| async move { Ok(()) }))
        .add_conditional_edge(
            "infinite",
            router!(|_state| async move {
                Ok(Some("infinite".to_string())) // Always loop
            }),
        )
        .with_max_iterations(10)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("infinite", state).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::MaxIterationsExceeded { current, max } => {
            assert_eq!(max, 10);
            assert!(current >= max);
        }
        other => panic!("Expected MaxIterationsExceeded error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_node_not_found() {
    let graph = AgentGraph::builder()
        .add_node("exists", node!(|_state| async move { Ok(()) }))
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("nonexistent", state).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::NodeNotFound(name) => {
            assert_eq!(name, "nonexistent");
        }
        other => panic!("Expected NodeNotFound error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_build_invalid_edge() {
    let result = AgentGraph::builder()
        .add_node("a", node!(|_state| async move { Ok(()) }))
        .add_edge("a", "nonexistent")
        .build();

    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_propagation() {
    let graph = AgentGraph::builder()
        .add_node(
            "fail",
            node!(|_state| async move {
                Err::<(), _>(AgentGraphError::ExecutionError(
                    "intentional failure".to_string(),
                ))
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("fail", state).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::ExecutionError(msg) => {
            assert_eq!(msg, "intentional failure");
        }
        other => panic!("Expected ExecutionError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_state_mutation_across_nodes() {
    let graph = AgentGraph::builder()
        .add_node(
            "init",
            node!(|state| async move {
                state.set("items", Vec::<String>::new()).await?;
                Ok(())
            }),
        )
        .add_node(
            "add_first",
            node!(|state| async move {
                state
                    .update::<Vec<String>, _>("items", |mut v| {
                        v.push("first".to_string());
                        v
                    })
                    .await?;
                Ok(())
            }),
        )
        .add_node(
            "add_second",
            node!(|state| async move {
                state
                    .update::<Vec<String>, _>("items", |mut v| {
                        v.push("second".to_string());
                        v
                    })
                    .await?;
                Ok(())
            }),
        )
        .add_edge("init", "add_first")
        .add_edge("add_first", "add_second")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("init", state).await.unwrap();

    let items: Vec<String> = result.get("items").await.unwrap();
    assert_eq!(items, vec!["first", "second"]);
}

#[tokio::test]
async fn test_conditional_routing_with_state() {
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("value", 10).await?;
                Ok(())
            }),
        )
        .add_node(
            "high",
            node!(|state| async move {
                state.set("route", "high").await?;
                Ok(())
            }),
        )
        .add_node(
            "low",
            node!(|state| async move {
                state.set("route", "low").await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "start",
            router!(|state| async move {
                let value: i32 = state.get("value").await?;
                Ok(Some(if value > 5 { "high" } else { "low" }.to_string()))
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("start", state).await.unwrap();

    let route: String = result.get("route").await.unwrap();
    assert_eq!(route, "high");
}

#[tokio::test]
async fn test_graph_with_initial_state() {
    let graph = AgentGraph::builder()
        .add_node(
            "process",
            node!(|state| async move {
                let input: String = state.get("input").await?;
                state.set("output", format!("processed: {}", input)).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    state.set("input", "hello world").await.unwrap();

    let result = graph.execute("process", state).await.unwrap();

    let output: String = result.get("output").await.unwrap();
    assert_eq!(output, "processed: hello world");
}
