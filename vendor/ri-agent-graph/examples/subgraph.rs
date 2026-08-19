use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Subgraph Example ===\n");

    // Create a reusable "validation" subgraph
    let validation_subgraph = AgentGraph::builder()
        .with_name("validation")
        .add_node(
            "check_length",
            node!(|state| async move {
                let text: String = state.get("text").await?;
                let valid = text.len() >= 5;
                state.set("length_ok", valid).await?;
                println!(
                    "  [validation] Length check: {} (len={})",
                    valid,
                    text.len()
                );
                Ok(())
            }),
        )
        .add_node(
            "check_content",
            node!(|state| async move {
                let text: String = state.get("text").await?;
                let valid = !text.contains("bad");
                state.set("content_ok", valid).await?;
                println!("  [validation] Content check: {}", valid);
                Ok(())
            }),
        )
        .add_node(
            "summarize",
            node!(|state| async move {
                let length_ok: bool = state.get("length_ok").await?;
                let content_ok: bool = state.get("content_ok").await?;
                state.set("valid", length_ok && content_ok).await?;
                println!(
                    "  [validation] Overall: {}",
                    if length_ok && content_ok {
                        "VALID"
                    } else {
                        "INVALID"
                    }
                );
                Ok(())
            }),
        )
        .set_entry_point("check_length")
        .add_edge("check_length", "check_content")
        .add_edge("check_content", "summarize")
        .set_finish_point("summarize")
        .build()?;

    // Create the main graph that uses the validation subgraph
    let graph = AgentGraph::builder()
        .with_name("main_pipeline")
        .add_node(
            "prepare",
            node!(|state| async move {
                let input: String = state.get("input").await?;
                println!("Preparing input: '{}'", input);
                state.set("text", input).await?;
                Ok(())
            }),
        )
        .add_subgraph("validate", validation_subgraph)
        .add_node(
            "process",
            node!(|state| async move {
                let valid: bool = state.get("valid").await?;
                if valid {
                    let text: String = state.get("text").await?;
                    let result = format!("Processed: {}", text.to_uppercase());
                    state.set("output", result).await?;
                    println!("Processing: success");
                } else {
                    state
                        .set("output", "Rejected: validation failed".to_string())
                        .await?;
                    println!("Processing: rejected");
                }
                Ok(())
            }),
        )
        .add_edge("prepare", "validate")
        .add_edge("validate", "process")
        .build()?;

    // Test with valid input
    println!("--- Test 1: Valid input ---");
    let state = AgentState::new();
    state.set("input", "hello world").await?;
    let result = graph.execute("prepare", state).await?;
    let output: String = result.get("output").await?;
    println!("Result: {}\n", output);

    // Test with invalid input (too short)
    println!("--- Test 2: Too short ---");
    let state = AgentState::new();
    state.set("input", "hi").await?;
    let result = graph.execute("prepare", state).await?;
    let output: String = result.get("output").await?;
    println!("Result: {}\n", output);

    // Test with bad content
    println!("--- Test 3: Bad content ---");
    let state = AgentState::new();
    state.set("input", "this is bad content").await?;
    let result = graph.execute("prepare", state).await?;
    let output: String = result.get("output").await?;
    println!("Result: {}", output);

    Ok(())
}
