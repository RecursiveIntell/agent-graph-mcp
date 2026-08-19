// This example shows how to use llm-pipeline as a node in agent-graph.
// Demonstrates the complementary nature of the two libraries.

use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Using Pipeline as Graph Node ===\n");

    // This demonstrates the concept - actual implementation would import llm-pipeline.
    // For now, we simulate a pipeline with a simple node.

    let graph = AgentGraph::builder()
        .add_node(
            "draft_pipeline",
            node!("draft_pipeline", |state| async move {
                let topic: String = state.get("topic").await?;

                println!("Running multi-stage draft pipeline on '{}'...", topic);

                // In real usage, this would be:
                // let pipeline = Pipeline::builder()
                //     .add_stage(Stage::new("ideate", "..."))
                //     .add_stage(Stage::new("draft", "..."))
                //     .add_stage(Stage::new("polish", "..."))
                //     .build()?;
                // let result = pipeline.execute(topic).await?;

                // Simulated output - quality improves each iteration
                let iteration: u32 = state.get_opt("iteration").await?.unwrap_or(0);
                let draft = format!(
                    "High-quality content about {} (rev {})",
                    topic,
                    iteration + 1
                );
                let quality = 0.75 + (iteration as f32 * 0.1);
                state.set("draft", draft).await?;
                state.set("quality", quality).await?;
                state.set("iteration", iteration + 1).await?;

                println!("Draft pipeline complete");
                Ok(())
            }),
        )
        .add_node(
            "review",
            node!("review", |state| async move {
                let quality: f32 = state.get("quality").await?;
                println!("\nReviewing draft (quality: {:.2})...", quality);

                let needs_revision = quality < 0.9;
                state.set("needs_revision", needs_revision).await?;

                if needs_revision {
                    println!("Needs revision - will regenerate");
                } else {
                    println!("Quality acceptable!");
                }

                Ok(())
            }),
        )
        .add_edge("draft_pipeline", "review")
        .add_conditional_edge(
            "review",
            router!(|state| async move {
                let needs_revision: bool = state.get("needs_revision").await?;

                if needs_revision {
                    Ok(Some("draft_pipeline".to_string()))
                } else {
                    Ok(None)
                }
            }),
        )
        .build()?;

    let state = AgentState::new();
    state.set("topic", "Rust async programming").await?;

    let result = graph.execute("draft_pipeline", state).await?;

    let draft: String = result.get("draft").await?;
    println!("\n=== Final Draft ===");
    println!("{}", draft);

    Ok(())
}
