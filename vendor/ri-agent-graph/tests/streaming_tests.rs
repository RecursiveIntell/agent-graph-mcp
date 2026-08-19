use ri_agent_graph::prelude::*;
use std::sync::Arc;

#[tokio::test]
async fn test_stream_events() {
    let graph = Arc::new(
        AgentGraph::builder()
            .with_name("test_graph")
            .add_node(
                "step1",
                node!(|state| async move {
                    state.set("value", 1).await?;
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
            .build()
            .unwrap(),
    );

    let state = AgentState::new();
    let config = GraphConfig::default();
    let (handle, mut rx) = graph.stream("step1", state, config);

    // Collect all events
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    let result = handle.await.unwrap().unwrap();
    let value: i32 = result.get("value").await.unwrap();
    assert_eq!(value, 2);

    // Check that we got the expected events
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::GraphStart { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::GraphEnd { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::NodeStart { node } if node == "step1")));
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::NodeEnd { node } if node == "step1")));
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::NodeStart { node } if node == "step2")));
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::NodeEnd { node } if node == "step2")));
}

#[tokio::test]
async fn test_stream_state_updates() {
    let graph = Arc::new(
        AgentGraph::builder()
            .add_node(
                "setter",
                node!(|state| async move {
                    state.set("key", "value").await?;
                    Ok(())
                }),
            )
            .build()
            .unwrap(),
    );

    let state = AgentState::new();
    let config = GraphConfig::default();
    let (handle, mut rx) = graph.stream("setter", state, config);

    let mut has_state_update = false;
    while let Some(event) = rx.recv().await {
        if let StreamEvent::StateUpdate { node, updates } = &event {
            if node == "setter" && updates.contains_key("key") {
                has_state_update = true;
            }
        }
    }

    handle.await.unwrap().unwrap();
    assert!(has_state_update, "Expected a StateUpdate event for 'key'");
}

#[tokio::test]
async fn test_stream_superstep_events() {
    let graph = Arc::new(
        AgentGraph::builder()
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
            .add_edge("a", "b")
            .build()
            .unwrap(),
    );

    let state = AgentState::new();
    let config = GraphConfig::default();
    let (handle, mut rx) = graph.stream("a", state, config);

    let mut superstep_starts = 0;
    let mut superstep_ends = 0;
    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::SuperstepStart { .. } => superstep_starts += 1,
            StreamEvent::SuperstepEnd { .. } => superstep_ends += 1,
            _ => {}
        }
    }

    handle.await.unwrap().unwrap();
    assert_eq!(superstep_starts, 2); // step1 and step2
    assert_eq!(superstep_ends, 2);
}
