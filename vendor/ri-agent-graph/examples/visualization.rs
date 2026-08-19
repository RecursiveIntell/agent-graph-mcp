use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Graph Visualization Example ===\n");

    // Build a complex graph
    let graph = AgentGraph::builder()
        .with_name("research_pipeline")
        .add_node("generate", node!(|_state| async move { Ok(()) }))
        .add_node("search", node!(|_state| async move { Ok(()) }))
        .add_node("evaluate", node!(|_state| async move { Ok(()) }))
        .add_node("summarize", node!(|_state| async move { Ok(()) }))
        .add_node("refine", node!(|_state| async move { Ok(()) }))
        .set_entry_point("generate")
        .add_edge("generate", "search")
        .add_edge("search", "evaluate")
        .add_conditional_edge(
            "evaluate",
            router!(|state| async move {
                let quality: f64 = state.get_opt("quality").await?.unwrap_or(0.0);
                if quality >= 0.8 {
                    Ok(Some("summarize".to_string()))
                } else {
                    Ok(Some("refine".to_string()))
                }
            }),
        )
        .add_edge("refine", "generate")
        .set_finish_point("summarize")
        .build()?;

    // Generate Mermaid diagram
    let mermaid = graph.to_mermaid();

    println!("Mermaid Diagram:");
    println!("```mermaid");
    println!("{}", mermaid);
    println!("```\n");

    println!("Copy the mermaid block above into:");
    println!("  - https://mermaid.live/");
    println!("  - Any Markdown renderer with Mermaid support");
    println!("  - GitHub README (supports Mermaid natively)");

    // Also show graph info
    println!("\nGraph Info:");
    println!("  Name: {:?}", graph.name());
    println!("  Nodes: {:?}", graph.node_names());

    Ok(())
}
