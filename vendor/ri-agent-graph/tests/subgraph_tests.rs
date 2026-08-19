use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_subgraph_basic() {
    // Create a subgraph
    let subgraph = AgentGraph::builder()
        .add_node(
            "sub_step1",
            node!(|state| async move {
                let val: i32 = state.get_opt("value").await?.unwrap_or(0);
                state.set("value", val + 10).await?;
                Ok(())
            }),
        )
        .add_node(
            "sub_step2",
            node!(|state| async move {
                let val: i32 = state.get("value").await?;
                state.set("value", val * 2).await?;
                Ok(())
            }),
        )
        .set_entry_point("sub_step1")
        .add_edge("sub_step1", "sub_step2")
        .set_finish_point("sub_step2")
        .build()
        .unwrap();

    // Create parent graph that uses the subgraph as a node
    let graph = AgentGraph::builder()
        .add_node(
            "init",
            node!(|state| async move {
                state.set("value", 5i32).await?;
                Ok(())
            }),
        )
        .add_subgraph("process", subgraph)
        .add_node(
            "finalize",
            node!(|state| async move {
                let val: i32 = state.get("value").await?;
                state.set("result", format!("Final: {}", val)).await?;
                Ok(())
            }),
        )
        .add_edge("init", "process")
        .add_edge("process", "finalize")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("init", state).await.unwrap();

    // value = (5 + 10) * 2 = 30
    let val: i32 = result.get("value").await.unwrap();
    assert_eq!(val, 30);

    let result_str: String = result.get("result").await.unwrap();
    assert_eq!(result_str, "Final: 30");
}

#[tokio::test]
async fn test_subgraph_state_isolation() {
    // Subgraph should not see parent-only state changes after fork
    let subgraph = AgentGraph::builder()
        .add_node(
            "sub_node",
            node!(|state| async move {
                // Read parent state that was passed
                let parent_val: i32 = state.get("shared").await?;
                state.set("sub_result", parent_val * 3).await?;
                // Modify shared key
                state.set("shared", 999i32).await?;
                Ok(())
            }),
        )
        .set_entry_point("sub_node")
        .set_finish_point("sub_node")
        .build()
        .unwrap();

    let graph = AgentGraph::builder()
        .add_node(
            "setup",
            node!(|state| async move {
                state.set("shared", 7i32).await?;
                Ok(())
            }),
        )
        .add_subgraph("sub", subgraph)
        .add_edge("setup", "sub")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("setup", state).await.unwrap();

    // Subgraph result should be merged back
    let sub_result: i32 = result.get("sub_result").await.unwrap();
    assert_eq!(sub_result, 21); // 7 * 3

    // Subgraph's modification to "shared" should be merged back
    let shared: i32 = result.get("shared").await.unwrap();
    assert_eq!(shared, 999);
}

#[tokio::test]
async fn test_subgraph_with_conditional_routing() {
    let subgraph = AgentGraph::builder()
        .add_node(
            "check",
            node!(|state| async move {
                let val: i32 = state.get("input").await?;
                state.set("checked", val > 10).await?;
                Ok(())
            }),
        )
        .add_node(
            "high",
            node!(|state| async move {
                state.set("category", "high").await?;
                Ok(())
            }),
        )
        .add_node(
            "low",
            node!(|state| async move {
                state.set("category", "low").await?;
                Ok(())
            }),
        )
        .set_entry_point("check")
        .add_conditional_edge(
            "check",
            router!(|state| async move {
                let checked: bool = state.get("checked").await?;
                Ok(Some(if checked { "high" } else { "low" }.to_string()))
            }),
        )
        .build()
        .unwrap();

    let graph = AgentGraph::builder()
        .add_node(
            "prep",
            node!(|state| async move {
                state.set("input", 15i32).await?;
                Ok(())
            }),
        )
        .add_subgraph("classify", subgraph)
        .add_edge("prep", "classify")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("prep", state).await.unwrap();

    let category: String = result.get("category").await.unwrap();
    assert_eq!(category, "high");
}

#[tokio::test]
async fn test_nested_subgraphs() {
    // Inner subgraph
    let inner = AgentGraph::builder()
        .add_node(
            "inner_node",
            node!(|state| async move {
                let val: i32 = state.get_opt("depth").await?.unwrap_or(0);
                state.set("depth", val + 1).await?;
                state.set("inner_done", true).await?;
                Ok(())
            }),
        )
        .set_entry_point("inner_node")
        .set_finish_point("inner_node")
        .build()
        .unwrap();

    // Outer subgraph containing inner
    let outer = AgentGraph::builder()
        .add_node(
            "outer_pre",
            node!(|state| async move {
                state.set("outer_started", true).await?;
                Ok(())
            }),
        )
        .add_subgraph("inner_sub", inner)
        .add_edge("outer_pre", "inner_sub")
        .set_entry_point("outer_pre")
        .set_finish_point("inner_sub")
        .build()
        .unwrap();

    // Main graph containing outer
    let graph = AgentGraph::builder()
        .add_node(
            "main_init",
            node!(|state| async move {
                state.set("depth", 0i32).await?;
                Ok(())
            }),
        )
        .add_subgraph("outer_sub", outer)
        .add_edge("main_init", "outer_sub")
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("main_init", state).await.unwrap();

    assert!(result.get::<bool>("outer_started").await.unwrap());
    assert!(result.get::<bool>("inner_done").await.unwrap());
    let depth: i32 = result.get("depth").await.unwrap();
    assert_eq!(depth, 1);
}
