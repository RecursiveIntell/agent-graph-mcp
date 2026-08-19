use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Basic Agent Graph Example ===\n");

    // Build a simple linear graph
    let graph = AgentGraph::builder()
        .add_node(
            "start",
            node!("start", |state| async move {
                println!("Node 1: Starting...");
                state.set("message", "Hello").await?;
                Ok(())
            }),
        )
        .add_node(
            "middle",
            node!("middle", |state| async move {
                let msg: String = state.get("message").await?;
                println!("Node 2: Received '{}'", msg);
                state.set("message", format!("{} from", msg)).await?;
                Ok(())
            }),
        )
        .add_node(
            "end",
            node!("end", |state| async move {
                let msg: String = state.get("message").await?;
                println!("Node 3: Final message: '{} agent-graph!'", msg);
                state.set("complete", true).await?;
                Ok(())
            }),
        )
        .add_edge("start", "middle")
        .add_edge("middle", "end")
        .build()?;

    // Execute
    let state = AgentState::new();
    let result = graph.execute("start", state).await?;

    // Check results
    let message: String = result.get("message").await?;
    let complete: bool = result.get("complete").await?;

    println!("\nFinal state:");
    println!("  message: {}", message);
    println!("  complete: {}", complete);

    Ok(())
}
