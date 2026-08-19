use ri_agent_graph::prelude::*;

/// Demonstrates the map-reduce pattern using fan-out with parallel execution.
/// A "distribute" node fans out to multiple "worker" nodes,
/// which all write to a shared "results" key using AppendReducer,
/// then a "collect" node summarizes the results.
#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Map-Reduce Example ===\n");

    let graph = AgentGraph::builder()
        .add_node(
            "distribute",
            node!(|state| async move {
                let items: Vec<String> = state.get("items").await?;
                println!("Distributing {} items across workers...", items.len());
                state.set("total_items", items.len() as i64).await?;
                // Items are already in state; each worker will process a subset
                Ok(())
            }),
        )
        .add_node(
            "worker_a",
            node!(|state| async move {
                let items: Vec<String> = state.get("items").await?;
                let batch: Vec<String> = items.into_iter().filter(|i| i.starts_with('a')).collect();
                println!("  Worker A processing {} items: {:?}", batch.len(), batch);
                let results: Vec<String> =
                    batch.iter().map(|i| format!("{}_processed", i)).collect();
                state.set("results", results).await?;
                Ok(())
            }),
        )
        .add_node(
            "worker_b",
            node!(|state| async move {
                let items: Vec<String> = state.get("items").await?;
                let batch: Vec<String> = items.into_iter().filter(|i| i.starts_with('b')).collect();
                println!("  Worker B processing {} items: {:?}", batch.len(), batch);
                let results: Vec<String> =
                    batch.iter().map(|i| format!("{}_processed", i)).collect();
                state.set("results", results).await?;
                Ok(())
            }),
        )
        .add_node(
            "worker_c",
            node!(|state| async move {
                let items: Vec<String> = state.get("items").await?;
                let batch: Vec<String> = items
                    .into_iter()
                    .filter(|i| !i.starts_with('a') && !i.starts_with('b'))
                    .collect();
                println!("  Worker C processing {} items: {:?}", batch.len(), batch);
                let results: Vec<String> =
                    batch.iter().map(|i| format!("{}_processed", i)).collect();
                state.set("results", results).await?;
                Ok(())
            }),
        )
        .add_node(
            "collect",
            node!(|state| async move {
                let results: Vec<String> = state.get("results").await?;
                println!("\nCollecting {} processed results:", results.len());
                for r in &results {
                    println!("  - {}", r);
                }
                state
                    .set("summary", format!("Processed {} items", results.len()))
                    .await?;
                Ok(())
            }),
        )
        // Fan-out: distribute -> worker_a, worker_b, worker_c
        .add_edge("distribute", "worker_a")
        .add_edge("distribute", "worker_b")
        .add_edge("distribute", "worker_c")
        // Fan-in: all workers -> collect
        .add_edge("worker_a", "collect")
        .add_edge("worker_b", "collect")
        .add_edge("worker_c", "collect")
        // Use AppendReducer to merge results from parallel workers
        .with_reducer("results", AppendReducer)
        .build()?;

    // Set up input data
    let state = AgentState::new();
    state
        .set(
            "items",
            vec![
                "alpha".to_string(),
                "bravo".to_string(),
                "charlie".to_string(),
                "apple".to_string(),
                "banana".to_string(),
                "delta".to_string(),
            ],
        )
        .await?;

    let result = graph.execute("distribute", state).await?;

    let summary: String = result.get("summary").await?;
    println!("\n{}", summary);

    Ok(())
}
