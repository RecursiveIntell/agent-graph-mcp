use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Conditional Routing Example ===\n");

    let graph = AgentGraph::builder()
        .add_node(
            "classifier",
            node!("classifier", |state| async move {
                let input: String = state.get("input").await?;
                let confidence: f32 = if input.contains("math") { 0.9 } else { 0.3 };

                println!("Classifier: input='{}', confidence={}", input, confidence);
                state.set("confidence", confidence).await?;
                Ok(())
            }),
        )
        .add_node(
            "high_confidence",
            node!("high_confidence", |state| async move {
                println!("High confidence handler executed");
                state.set("result", "Handled with confidence").await?;
                Ok(())
            }),
        )
        .add_node(
            "low_confidence",
            node!("low_confidence", |state| async move {
                println!("Low confidence handler executed");
                state.set("result", "Needs review").await?;
                Ok(())
            }),
        )
        .add_conditional_edge(
            "classifier",
            router!(|state| async move {
                let confidence: f32 = state.get("confidence").await?;

                if confidence > 0.8 {
                    Ok(Some("high_confidence".to_string()))
                } else {
                    Ok(Some("low_confidence".to_string()))
                }
            }),
        )
        .build()?;

    // Test with high confidence input
    println!("--- Test 1: High Confidence ---");
    let state = AgentState::new();
    state.set("input", "solve this math problem").await?;
    let result = graph.execute("classifier", state).await?;
    let output: String = result.get("result").await?;
    println!("Result: {}\n", output);

    // Test with low confidence input
    println!("--- Test 2: Low Confidence ---");
    let state2 = AgentState::new();
    state2.set("input", "tell me a story").await?;
    let result2 = graph.execute("classifier", state2).await?;
    let output2: String = result2.get("result").await?;
    println!("Result: {}\n", output2);

    Ok(())
}
