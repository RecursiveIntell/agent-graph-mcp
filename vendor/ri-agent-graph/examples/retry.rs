use ri_agent_graph::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Retry Policy Example ===\n");

    // ---------------------------------------------------------------
    // Part 1: A flaky API node that fails the first 2 attempts, then
    // succeeds on the 3rd. Retry policy allows up to 5 attempts.
    // ---------------------------------------------------------------

    println!("--- Part 1: Flaky API with automatic retries ---\n");

    let graph = AgentGraph::builder()
        .add_node_with_retry(
            "flaky_api",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("flaky_attempt").await?.unwrap_or(0);
                let attempt = attempt + 1;
                state.set("flaky_attempt", attempt).await?;

                println!("  [flaky_api] Attempt {} ...", attempt);

                if attempt < 3 {
                    println!("  [flaky_api] Transient failure on attempt {}!", attempt);
                    return Err(AgentGraphError::ExecutionError(format!(
                        "API timeout on attempt {}",
                        attempt
                    )));
                }

                println!("  [flaky_api] Success on attempt {}!", attempt);
                state.set("flaky_result", "data from API").await?;
                Ok(())
            }),
            RetryPolicy::new()
                .with_max_attempts(5)
                .with_initial_interval(Duration::from_millis(100))
                .with_jitter(false),
        )
        .add_node(
            "process",
            node!(|state| async move {
                let data: String = state.get("flaky_result").await?;
                println!("  [process] Processing result: '{}'", data);
                state
                    .set("final_output", format!("processed: {}", data))
                    .await?;
                Ok(())
            }),
        )
        .add_edge("flaky_api", "process")
        .build()?;

    let state = AgentState::new();
    let result = graph.execute("flaky_api", state).await?;

    let attempts: u32 = result.get("flaky_attempt").await?;
    let output: String = result.get("final_output").await?;
    println!("\n  Total attempts: {}", attempts);
    println!("  Final output  : {}", output);

    // ---------------------------------------------------------------
    // Part 2: A picky API node with a retry predicate that only
    // retries ExecutionError -- other error types fail immediately.
    // ---------------------------------------------------------------

    println!("\n--- Part 2: Selective retry with predicate ---\n");

    let picky_graph = AgentGraph::builder()
        .add_node_with_retry(
            "picky_api",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("picky_attempt").await?.unwrap_or(0);
                let attempt = attempt + 1;
                state.set("picky_attempt", attempt).await?;

                println!("  [picky_api] Attempt {} ...", attempt);

                match attempt {
                    1 => {
                        // First attempt: retryable ExecutionError
                        println!("  [picky_api] ExecutionError (retryable) on attempt 1");
                        Err(AgentGraphError::ExecutionError(
                            "connection reset".to_string(),
                        ))
                    }
                    2 => {
                        // Second attempt: retryable ExecutionError again
                        println!("  [picky_api] ExecutionError (retryable) on attempt 2");
                        Err(AgentGraphError::ExecutionError("read timeout".to_string()))
                    }
                    _ => {
                        // Third attempt: success
                        println!("  [picky_api] Success on attempt {}!", attempt);
                        state.set("picky_result", "picky data").await?;
                        Ok(())
                    }
                }
            }),
            RetryPolicy::new()
                .with_max_attempts(5)
                .with_initial_interval(Duration::from_millis(100))
                .with_jitter(false)
                .with_retry_on(|err| {
                    // Only retry on ExecutionError; all other errors fail fast
                    matches!(err, AgentGraphError::ExecutionError(_))
                }),
        )
        .build()?;

    let state2 = AgentState::new();
    let result2 = picky_graph.execute("picky_api", state2).await?;

    let picky_attempts: u32 = result2.get("picky_attempt").await?;
    let picky_result: String = result2.get("picky_result").await?;
    println!("\n  Total attempts: {}", picky_attempts);
    println!("  Result        : {}", picky_result);

    // ---------------------------------------------------------------
    // Part 3: Show that a non-retryable error type fails immediately
    // when a retry predicate is configured.
    // ---------------------------------------------------------------

    println!("\n--- Part 3: Non-retryable error fails immediately ---\n");

    let non_retryable_graph = AgentGraph::builder()
        .add_node_with_retry(
            "strict_api",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("strict_attempt").await?.unwrap_or(0);
                let attempt = attempt + 1;
                state.set("strict_attempt", attempt).await?;

                println!("  [strict_api] Attempt {} ...", attempt);

                // Return a StateError, which the predicate does NOT retry
                println!(
                    "  [strict_api] StateError (non-retryable) on attempt {}",
                    attempt
                );
                Err::<(), _>(AgentGraphError::StateError(
                    "invalid input format".to_string(),
                ))
            }),
            RetryPolicy::new()
                .with_max_attempts(5)
                .with_initial_interval(Duration::from_millis(100))
                .with_jitter(false)
                .with_retry_on(|err| matches!(err, AgentGraphError::ExecutionError(_))),
        )
        .build()?;

    let state3 = AgentState::new();
    let result3 = non_retryable_graph.execute("strict_api", state3).await;

    match result3 {
        Err(AgentGraphError::StateError(msg)) => {
            println!("  Correctly failed without retrying: {}", msg);
        }
        Err(other) => {
            println!("  Unexpected error type: {:?}", other);
        }
        Ok(s) => {
            let attempts: u32 = s.get("strict_attempt").await?;
            println!("  Unexpectedly succeeded after {} attempts", attempts);
        }
    }

    println!("\nAll retry examples complete.");

    Ok(())
}
