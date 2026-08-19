//! AG-001 regression: ordinary execution failures must NOT be mapped to
//! `ExecutionResult::Complete`. They must be preserved as `ExecutionResult::Failed`.
//!
//! The audited bug was in `execute_with_interrupt`:
//! ```ignore
//! Err(_) => ExecutionResult::Complete(state_clone),  // false success
//! ```
//! This test ensures that a node returning an error results in `Failed`,
//! not `Complete` or `Interrupted`.

use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_ordinary_error_is_failed_not_complete() {
    // A graph with a node that always fails.
    let graph = AgentGraph::builder()
        .add_node(
            "fail_node",
            node!(|_state| async move {
                Err::<(), AgentGraphError>(AgentGraphError::ExecutionError(
                    "intentional failure".to_string(),
                ))
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph
        .execute_with_interrupt("fail_node", state, GraphConfig::default())
        .await;

    // AG-001: The error must be preserved as Failed, not silently mapped to Complete.
    match result {
        ExecutionResult::Failed { error, .. } => {
            // Verify the original error is preserved.
            let msg = error.to_string();
            assert!(
                msg.contains("intentional failure"),
                "error message should contain the original failure, got: {msg}"
            );
        }
        ExecutionResult::Complete(_) => {
            panic!("AG-001: ordinary error was silently mapped to Complete (false success)");
        }
        ExecutionResult::Interrupted { .. } => {
            panic!("AG-001: ordinary error was misclassified as Interrupted");
        }
    }
}

#[tokio::test]
async fn test_cancellation_is_failed_not_complete() {
    // Cancellation is an ordinary error (AgentGraphError::Cancelled),
    // not an interrupt. It must be preserved as Failed.
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("step1_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("step2_done", true).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .build()
        .unwrap();

    // Use a config that will cause MaxIterationsExceeded by setting recursion_limit to 0.
    let config = GraphConfig {
        recursion_limit: 0,
        ..Default::default()
    };

    let result = graph
        .execute_with_interrupt("step1", AgentState::new(), config)
        .await;

    match result {
        ExecutionResult::Failed { error, .. } => {
            // The error should be MaxIterationsExceeded, not Complete.
            assert!(
                error.to_string().contains("iterations") || error.to_string().contains("cycle"),
                "expected max iterations or cycle error, got: {error}"
            );
        }
        ExecutionResult::Complete(_) => {
            panic!("AG-001: max-iterations error was silently mapped to Complete");
        }
        ExecutionResult::Interrupted { .. } => {
            panic!("AG-001: max-iterations error was misclassified as Interrupted");
        }
    }
}

#[tokio::test]
async fn test_successful_execution_still_completes() {
    // Positive test: successful execution should still return Complete.
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("done", true).await?;
                Ok(())
            }),
        )
        .build()
        .unwrap();

    let result = graph
        .execute_with_interrupt("step1", AgentState::new(), GraphConfig::default())
        .await;

    match result {
        ExecutionResult::Complete(state) => {
            assert!(state.get::<bool>("done").await.unwrap());
        }
        ExecutionResult::Failed { error, .. } => {
            panic!("successful execution should return Complete, not Failed: {error}");
        }
        ExecutionResult::Interrupted { .. } => {
            panic!("successful execution should return Complete, not Interrupted");
        }
    }
}

#[tokio::test]
async fn test_interrupt_still_returns_interrupted() {
    // Interrupts should still be preserved as Interrupted, not Failed.
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("step1_done", true).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("step2_done", true).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .with_interrupt_before(vec!["step2".to_string()])
        .build()
        .unwrap();

    let result = graph
        .execute_with_interrupt("step1", AgentState::new(), GraphConfig::default())
        .await;

    match result {
        ExecutionResult::Interrupted { node, .. } => {
            assert_eq!(node, "step2");
        }
        ExecutionResult::Complete(_) => {
            panic!("should have been interrupted, not completed");
        }
        ExecutionResult::Failed { error, .. } => {
            panic!("interrupt should return Interrupted, not Failed: {error}");
        }
    }
}
