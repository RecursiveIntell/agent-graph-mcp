use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_router_returns_node_name() {
    let state = AgentState::new();
    state.set("choice", "a").await.unwrap();

    let router = router!(|state| async move {
        let choice: String = state.get("choice").await?;
        Ok(Some(choice))
    });

    let config = GraphConfig::default();
    let result = router.route(&state, &config).await.unwrap();
    assert_eq!(result, RouterOutput::Next(Some("a".to_string())));
}

#[tokio::test]
async fn test_router_returns_none() {
    let state = AgentState::new();
    state.set("done", true).await.unwrap();

    let router = router!(|state| async move {
        let done: bool = state.get("done").await?;
        if done {
            Ok(None)
        } else {
            Ok(Some("continue".to_string()))
        }
    });

    let config = GraphConfig::default();
    let result = router.route(&state, &config).await.unwrap();
    assert_eq!(result, RouterOutput::Next(None));
}

#[tokio::test]
async fn test_conditional_routing_high_value() {
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("value", 10i32).await?;
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
async fn test_conditional_routing_low_value() {
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!(|state| async move {
                state.set("value", 2i32).await?;
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
    assert_eq!(route, "low");
}

#[tokio::test]
async fn test_router_end_execution() {
    let graph = AgentGraph::builder()
        .add_node(
            "only_node",
            node!(|state| async move {
                state.set("executed", true).await?;
                Ok(())
            }),
        )
        .add_conditional_edge("only_node", router!(|_state| async move { Ok(None) }))
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("only_node", state).await.unwrap();

    let executed: bool = result.get("executed").await.unwrap();
    assert!(executed);
}

#[tokio::test]
async fn test_multi_stage_routing() {
    let graph = AgentGraph::builder()
        .add_node(
            "stage1",
            node!(|state| async move {
                state.set("stage", 1u32).await?;
                state.set("path", "A").await?;
                Ok(())
            }),
        )
        .add_node(
            "stage2a",
            node!(|state| async move {
                state.set("stage", 2u32).await?;
                let path: String = state.get("path").await?;
                state.set("path", format!("{} -> 2A", path)).await?;
                Ok(())
            }),
        )
        .add_node(
            "stage2b",
            node!(|state| async move {
                state.set("stage", 2u32).await?;
                let path: String = state.get("path").await?;
                state.set("path", format!("{} -> 2B", path)).await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "stage1",
            router!(|state| async move {
                let path: String = state.get("path").await?;
                if path == "A" {
                    Ok(Some("stage2a".to_string()))
                } else {
                    Ok(Some("stage2b".to_string()))
                }
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("stage1", state).await.unwrap();

    let path: String = result.get("path").await.unwrap();
    assert_eq!(path, "A -> 2A");
}
