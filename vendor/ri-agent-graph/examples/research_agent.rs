use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Research Agent Example ===\n");

    let graph = AgentGraph::builder()
        .add_node(
            "generate_queries",
            node!("generate_queries", |state| async move {
                let topic: String = state.get("topic").await?;
                let iteration: usize = state.get_opt("iteration").await?.unwrap_or(0);

                println!(
                    "Iteration {}: Generating search queries for '{}'",
                    iteration + 1,
                    topic
                );

                // Simulate query generation
                let queries = vec![
                    format!("{} overview", topic),
                    format!("{} latest developments", topic),
                    format!("{} use cases", topic),
                ];

                state.set("queries", queries.clone()).await?;
                state.set("iteration", iteration + 1).await?;

                for (i, q) in queries.iter().enumerate() {
                    println!("  Query {}: {}", i + 1, q);
                }

                Ok(())
            }),
        )
        .add_node(
            "search",
            node!("search", |state| async move {
                let queries: Vec<String> = state.get("queries").await?;
                println!("\nSearching...");

                // Simulate search results
                let mut findings: Vec<String> =
                    state.get_opt("findings").await?.unwrap_or_default();
                findings.extend(queries.iter().map(|q| format!("Result for: {}", q)));

                state.set("findings", findings.clone()).await?;
                println!("Total findings: {}", findings.len());

                Ok(())
            }),
        )
        .add_node(
            "evaluate",
            node!("evaluate", |state| async move {
                let findings: Vec<String> = state.get("findings").await?;
                let iteration: usize = state.get("iteration").await?;

                // Stop if we have enough findings or reached max iterations
                let is_sufficient = findings.len() >= 9 || iteration >= 3;

                println!(
                    "\nEvaluating: {} findings, iteration {}",
                    findings.len(),
                    iteration
                );
                println!("Sufficient: {}", is_sufficient);

                state.set("is_sufficient", is_sufficient).await?;
                Ok(())
            }),
        )
        .add_edge("generate_queries", "search")
        .add_edge("search", "evaluate")
        .add_conditional_edge(
            "evaluate",
            router!(|state| async move {
                let is_sufficient: bool = state.get("is_sufficient").await?;

                if is_sufficient {
                    println!("\nResearch complete!\n");
                    Ok(None)
                } else {
                    println!("\nNeed more research, continuing...\n");
                    Ok(Some("generate_queries".to_string()))
                }
            }),
        )
        .with_max_iterations(10)
        .build()?;

    let state = AgentState::new();
    state.set("topic", "Rust for AI").await?;

    let result = graph.execute("generate_queries", state).await?;

    let findings: Vec<String> = result.get("findings").await?;
    let iterations: usize = result.get("iteration").await?;

    println!("=== Research Summary ===");
    println!("Iterations: {}", iterations);
    println!("Total findings: {}", findings.len());
    println!("\nFindings:");
    for (i, finding) in findings.iter().enumerate() {
        println!("  {}. {}", i + 1, finding);
    }

    Ok(())
}
