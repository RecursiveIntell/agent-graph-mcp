use ri_agent_graph::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Real-Time Event Streaming Example ===\n");

    // Build a 3-node graph and wrap it in Arc for streaming
    let graph = Arc::new(
        AgentGraph::builder()
            .with_name("streaming_demo")
            .add_node(
                "fetch",
                node!("fetch", |state| async move {
                    state.set("source", "api.example.com").await?;
                    state.set("records", 150i64).await?;
                    Ok(())
                }),
            )
            .add_node(
                "transform",
                node!("transform", |state| async move {
                    let records: i64 = state.get("records").await?;
                    let filtered = records * 80 / 100; // keep 80%
                    state.set("records", filtered).await?;
                    state.set("status", "transformed").await?;
                    Ok(())
                }),
            )
            .add_node(
                "load",
                node!("load", |state| async move {
                    let records: i64 = state.get("records").await?;
                    let source: String = state.get("source").await?;
                    state
                        .set(
                            "summary",
                            format!("Loaded {} records from {}", records, source),
                        )
                        .await?;
                    state.set("status", "complete").await?;
                    Ok(())
                }),
            )
            .add_edge("fetch", "transform")
            .add_edge("transform", "load")
            .build()?,
    );

    // Stream execution events in real time
    let state = AgentState::new();
    let config = GraphConfig::new();
    let (handle, mut rx) = graph.stream("fetch", state, config);

    // Collect and print every event as it arrives
    let mut events: Vec<StreamEvent> = Vec::new();
    while let Some(event) = rx.recv().await {
        match &event {
            StreamEvent::GraphStart { graph_name } => {
                println!("[GraphStart]     graph={:?}", graph_name);
            }
            StreamEvent::SuperstepStart { step, nodes } => {
                println!("[SuperstepStart] step={}, nodes={:?}", step, nodes);
            }
            StreamEvent::NodeStart { node } => {
                println!("[NodeStart]      node={}", node);
            }
            StreamEvent::StateUpdate { node, updates } => {
                let keys: Vec<&String> = updates.keys().collect();
                println!("[StateUpdate]    node={}, keys={:?}", node, keys);
            }
            StreamEvent::NodeEnd { node } => {
                println!("[NodeEnd]        node={}", node);
            }
            StreamEvent::SuperstepEnd { step } => {
                println!("[SuperstepEnd]   step={}", step);
            }
            StreamEvent::GraphEnd { graph_name } => {
                println!("[GraphEnd]       graph={:?}", graph_name);
            }
            StreamEvent::Interrupt { node, value } => {
                println!("[Interrupt]      node={}, value={:?}", node, value);
            }
            StreamEvent::Custom(value) => {
                println!("[Custom]         {:?}", value);
            }
            _ => {
                println!("[Other]          {:?}", event);
            }
        }
        events.push(event);
    }

    // Wait for the execution to complete and get the final state
    let result = handle.await.unwrap()?;

    // Print final state
    println!("\n=== Final State ===");
    let summary: String = result.get("summary").await?;
    let status: String = result.get("status").await?;
    println!("  summary: {}", summary);
    println!("  status: {}", status);

    // Summarize captured events
    println!("\n=== Event Summary ===");
    let graph_starts = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::GraphStart { .. }))
        .count();
    let graph_ends = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::GraphEnd { .. }))
        .count();
    let node_starts = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::NodeStart { .. }))
        .count();
    let node_ends = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::NodeEnd { .. }))
        .count();
    let state_updates = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::StateUpdate { .. }))
        .count();
    let superstep_starts = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::SuperstepStart { .. }))
        .count();
    let superstep_ends = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::SuperstepEnd { .. }))
        .count();

    println!("  GraphStart:     {}", graph_starts);
    println!("  SuperstepStart: {}", superstep_starts);
    println!("  NodeStart:      {}", node_starts);
    println!("  StateUpdate:    {}", state_updates);
    println!("  NodeEnd:        {}", node_ends);
    println!("  SuperstepEnd:   {}", superstep_ends);
    println!("  GraphEnd:       {}", graph_ends);
    println!("  Total events:   {}", events.len());

    Ok(())
}
