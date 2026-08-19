#[cfg(feature = "checkpointing")]
use ri_agent_graph::prelude::*;

#[cfg(feature = "checkpointing")]
#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Checkpointing Example ===\n");

    let db_path = "/tmp/ri_agent_graph_checkpoints.db";

    // Create checkpoint manager
    let manager = CheckpointManager::new(db_path)?;

    // Create and execute a graph
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!("step1", |state| async move {
                println!("Step 1: Initializing...");
                state.set("progress", 1u32).await?;
                state.set("data", "initial data").await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!("step2", |state| async move {
                let progress: u32 = state.get("progress").await?;
                println!("Step 2: Processing (progress={})...", progress);
                state.set("progress", progress + 1).await?;
                state.set("data", "processed data").await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .build()?;

    let execution_id = uuid::Uuid::new_v4().to_string();
    println!("Execution ID: {}\n", execution_id);

    // Execute the graph
    let state = AgentState::new();
    let result = graph.execute("step1", state).await?;

    // Save a checkpoint after execution
    let checkpoint = Checkpoint {
        execution_id: execution_id.clone(),
        timestamp: chrono::Utc::now(),
        current_node: "step2".to_string(),
        iteration: 1,
        state: result.snapshot().await,
        step_number: 0,
        active_nodes: Vec::new(),
    };

    manager.save(&checkpoint)?;
    println!("Checkpoint saved!\n");

    // Load the checkpoint back
    if let Some(loaded) = manager.load(&execution_id)? {
        println!("Loaded checkpoint:");
        println!("  Execution ID: {}", loaded.execution_id);
        println!("  Current node: {}", loaded.current_node);
        println!("  Iteration: {}", loaded.iteration);
        println!("  Timestamp: {}", loaded.timestamp);

        // Restore state from checkpoint
        let restored_state = AgentState::new();
        restored_state.restore(&loaded.state).await;

        let progress: u32 = restored_state.get("progress").await?;
        let data: String = restored_state.get("data").await?;
        println!("  State - progress: {}, data: '{}'", progress, data);
    }

    // Clean up
    manager.clear(&execution_id)?;
    println!("\nCheckpoints cleared.");

    // Clean up the temp file
    std::fs::remove_file(db_path).ok();

    Ok(())
}

#[cfg(not(feature = "checkpointing"))]
fn main() {
    println!("This example requires the 'checkpointing' feature.");
    println!("Run with: cargo run --example checkpointing --features checkpointing");
}
