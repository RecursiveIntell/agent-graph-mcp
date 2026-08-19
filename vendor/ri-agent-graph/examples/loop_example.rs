use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Loop Example (Quality Refinement) ===\n");

    let graph = AgentGraph::builder()
        .add_node(
            "generate",
            node!("generate", |state| async move {
                let iteration: usize = state.get_opt("iteration").await?.unwrap_or(0);
                println!("Generating content (iteration {})...", iteration + 1);

                // Simulate improving quality each iteration
                let quality = 0.5 + (iteration as f32 * 0.15);
                state.set("quality", quality).await?;
                state.set("iteration", iteration + 1).await?;

                println!("  Generated with quality: {:.2}", quality);
                Ok(())
            }),
        )
        .add_node(
            "evaluate",
            node!("evaluate", |state| async move {
                let quality: f32 = state.get("quality").await?;
                let is_good = quality >= 0.8;

                println!("Evaluating: quality={:.2}, good={}", quality, is_good);
                state.set("is_good", is_good).await?;
                Ok(())
            }),
        )
        .add_edge("generate", "evaluate")
        .add_conditional_edge(
            "evaluate",
            router!(|state| async move {
                let is_good: bool = state.get("is_good").await?;

                if is_good {
                    println!("Quality threshold met! Finishing.\n");
                    Ok(None) // End execution
                } else {
                    println!("Quality too low, regenerating...\n");
                    Ok(Some("generate".to_string())) // Loop back
                }
            }),
        )
        .with_max_iterations(10)
        .build()?;

    let state = AgentState::new();
    let result = graph.execute("generate", state).await?;

    let final_quality: f32 = result.get("quality").await?;
    let iterations: usize = result.get("iteration").await?;

    println!("=== Final Results ===");
    println!("Iterations: {}", iterations);
    println!("Final quality: {:.2}", final_quality);

    Ok(())
}
