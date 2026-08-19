use ri_agent_graph::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Human-in-the-Loop Interrupt Example ===\n");

    // ---------------------------------------------------------------
    // Build a three-step workflow: draft -> review -> finalize
    // An interrupt is configured BEFORE the "review" node so that a
    // human can inspect the draft before the review step executes.
    // ---------------------------------------------------------------

    let graph = AgentGraph::builder()
        .add_node(
            "draft",
            node!(|state| async move {
                println!("[draft] Generating initial draft...");
                state
                    .set(
                        "document",
                        "Rust is a systems programming language focused on safety.",
                    )
                    .await?;
                state.set("status", "drafted").await?;
                println!("[draft] Draft complete.");
                Ok(())
            }),
        )
        .add_node(
            "review",
            node!(|state| async move {
                println!("[review] Reviewing document...");
                let doc: String = state.get("document").await?;
                // Simulate a review pass that adds annotations
                let reviewed = format!("{} [Reviewed: looks good]", doc);
                state.set("document", reviewed).await?;
                state.set("status", "reviewed").await?;
                println!("[review] Review complete.");
                Ok(())
            }),
        )
        .add_node(
            "finalize",
            node!(|state| async move {
                println!("[finalize] Finalizing document...");
                let doc: String = state.get("document").await?;
                let finalized = format!("{} [Final version]", doc);
                state.set("document", finalized).await?;
                state.set("status", "finalized").await?;
                println!("[finalize] Document finalized.");
                Ok(())
            }),
        )
        .add_edge("draft", "review")
        .add_edge("review", "finalize")
        // This is the key line: pause execution BEFORE "review" runs.
        .with_interrupt_before(vec!["review".to_string()])
        .build()?;

    // ---------------------------------------------------------------
    // First run: execute from "draft" -- will be interrupted before
    // the "review" node gets a chance to run.
    // ---------------------------------------------------------------

    println!("--- Run 1: Execute from 'draft' (expecting interrupt) ---\n");

    let state = AgentState::new();
    let result = graph
        .execute_with_interrupt("draft", state, GraphConfig::new())
        .await;

    let resumed_state = match result {
        ExecutionResult::Interrupted {
            state,
            node,
            checkpoint_data,
            ..
        } => {
            println!("\nExecution interrupted before node '{}'.", node);

            let status: String = state.get("status").await?;
            let doc: String = state.get("document").await?;
            println!("  Current status : {}", status);
            println!("  Document so far: {}", doc);

            if let Some(ref cp) = checkpoint_data {
                println!("  Resume node    : {}", cp.resume_node);
            }

            println!("\n  (A human would inspect the draft here and decide to proceed.)\n");

            // Optionally modify state before resuming -- simulating a
            // human adding a note.
            state.set("human_note", "Approved by reviewer.").await?;

            state
        }
        ExecutionResult::Complete(_) => {
            println!("ERROR: Expected an interrupt but the graph completed.");
            return Ok(());
        }
        ExecutionResult::Failed { error, .. } => {
            eprintln!("ERROR: Graph execution failed: {error}");
            return Err(error);
        }
    };

    // ---------------------------------------------------------------
    // Second run: resume from the "review" node with the state that
    // was captured at the interrupt point (including human edits).
    // We use execute_with_config to bypass the interrupt on resume.
    // ---------------------------------------------------------------

    println!("--- Run 2: Resume from 'review' (completing the workflow) ---\n");

    // Build a graph without the interrupt so the review node runs.
    let resume_graph = AgentGraph::builder()
        .add_node(
            "review",
            node!(|state| async move {
                println!("[review] Reviewing document...");
                let doc: String = state.get("document").await?;
                let note: String = state
                    .get_opt("human_note")
                    .await?
                    .unwrap_or_else(|| "(none)".to_string());
                println!("[review] Human note: {}", note);
                let reviewed = format!("{} [Reviewed: looks good]", doc);
                state.set("document", reviewed).await?;
                state.set("status", "reviewed").await?;
                println!("[review] Review complete.");
                Ok(())
            }),
        )
        .add_node(
            "finalize",
            node!(|state| async move {
                println!("[finalize] Finalizing document...");
                let doc: String = state.get("document").await?;
                let finalized = format!("{} [Final version]", doc);
                state.set("document", finalized).await?;
                state.set("status", "finalized").await?;
                println!("[finalize] Document finalized.");
                Ok(())
            }),
        )
        .add_edge("review", "finalize")
        .build()?;

    let final_state = resume_graph.execute("review", resumed_state).await?;

    // ---------------------------------------------------------------
    // Print the final results
    // ---------------------------------------------------------------

    println!("\n--- Final Results ---\n");

    let document: String = final_state.get("document").await?;
    let status: String = final_state.get("status").await?;
    let human_note: String = final_state.get("human_note").await?;

    println!("  Status    : {}", status);
    println!("  Human note: {}", human_note);
    println!("  Document  : {}", document);
    println!("\nWorkflow completed successfully.");

    Ok(())
}
