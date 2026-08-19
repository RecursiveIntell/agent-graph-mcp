use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Fan-Out / Fan-In Parallel Execution Example ===\n");

    // Build a graph with fan-out and fan-in:
    //
    //   init
    //   / \
    //  v   v
    // branch_a  branch_b   (parallel)
    //  \   /
    //   v v
    //  merge
    //
    let graph = AgentGraph::builder()
        .add_node(
            "init",
            node!("init", |state| async move {
                println!("[init] Setting up initial data...");
                state.set("title", "Parallel Workflow").await?;
                state.set("results", Vec::<String>::new()).await?;
                state.set("total_score", 0i64).await?;
                println!("[init] Initial data ready.");
                Ok(())
            }),
        )
        .add_node(
            "branch_a",
            node!("branch_a", |state| async move {
                println!("[branch_a] Starting text analysis...");
                // Simulate some work
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;

                let title: String = state.get("title").await?;
                let finding = format!(
                    "Text analysis of '{}': 42 tokens, positive sentiment",
                    title
                );
                println!("[branch_a] Done: {}", finding);

                // Write results -- the AppendReducer will merge these safely
                state.set("results", vec![finding]).await?;
                state.set("total_score", 85i64).await?;
                Ok(())
            }),
        )
        .add_node(
            "branch_b",
            node!("branch_b", |state| async move {
                println!("[branch_b] Starting keyword extraction...");
                // Simulate some work
                tokio::time::sleep(std::time::Duration::from_millis(60)).await;

                let title: String = state.get("title").await?;
                let finding = format!(
                    "Keyword extraction of '{}': [parallel, workflow, rust]",
                    title
                );
                println!("[branch_b] Done: {}", finding);

                // Write results -- the AppendReducer will merge these safely
                state.set("results", vec![finding]).await?;
                state.set("total_score", 72i64).await?;
                Ok(())
            }),
        )
        .add_node(
            "merge",
            node!("merge", |state| async move {
                println!("[merge] Combining results from parallel branches...");

                let results: Vec<String> = state.get("results").await?;
                let total_score: f64 = state.get("total_score").await?;

                println!("[merge] Received {} results:", results.len());
                for (i, result) in results.iter().enumerate() {
                    println!("  {}. {}", i + 1, result);
                }
                println!("[merge] Combined score: {}", total_score);

                state
                    .set(
                        "summary",
                        format!(
                            "{} analyses completed, combined score: {}",
                            results.len(),
                            total_score
                        ),
                    )
                    .await?;

                Ok(())
            }),
        )
        // Fan-out: init sends to both branches in parallel
        .add_edge("init", "branch_a")
        .add_edge("init", "branch_b")
        // Fan-in: both branches converge at merge
        .add_edge("branch_a", "merge")
        .add_edge("branch_b", "merge")
        // Reducers for safe parallel state merging
        .with_reducer("results", AppendReducer)
        .with_reducer("total_score", AddReducer)
        .build()?;

    // Execute the graph
    let state = AgentState::new();
    let start = std::time::Instant::now();
    let result = graph.execute("init", state).await?;
    let elapsed = start.elapsed();

    // Print final results
    println!("\n=== Final State ===");
    let title: String = result.get("title").await?;
    let results: Vec<String> = result.get("results").await?;
    let total_score: f64 = result.get("total_score").await?;
    let summary: String = result.get("summary").await?;

    println!("  title: {}", title);
    println!("  results: {:?}", results);
    println!("  total_score: {}", total_score);
    println!("  summary: {}", summary);
    println!("  elapsed: {:?}", elapsed);

    // Verify parallel execution was actually parallel
    // Both branches sleep ~60-80ms, so total should be well under 200ms
    assert!(
        elapsed.as_millis() < 200,
        "Expected parallel execution but took {:?}",
        elapsed
    );
    println!("\nParallel execution verified (branches ran concurrently).");

    Ok(())
}
