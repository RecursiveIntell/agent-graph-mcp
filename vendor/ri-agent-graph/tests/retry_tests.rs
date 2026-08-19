use ri_agent_graph::prelude::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn test_retry_on_failure() {
    let graph = AgentGraph::builder()
        .add_node_with_retry(
            "flaky",
            node!(|state| async move {
                // This would need access to the counter...
                // Instead, use state to track attempts
                let attempt: u32 = state.get_opt("attempt").await?.unwrap_or(0);
                state.set("attempt", attempt + 1).await?;

                if attempt < 2 {
                    Err(AgentGraphError::ExecutionError(
                        "transient failure".to_string(),
                    ))
                } else {
                    state.set("success", true).await?;
                    Ok(())
                }
            }),
            RetryPolicy::new()
                .with_max_attempts(5)
                .with_initial_interval(Duration::from_millis(10))
                .with_jitter(false),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("flaky", state).await.unwrap();

    let success: bool = result.get("success").await.unwrap();
    assert!(success);

    let attempts: u32 = result.get("attempt").await.unwrap();
    assert_eq!(attempts, 3); // failed 2 times, succeeded on 3rd
}

#[tokio::test]
async fn test_retry_exhausted() {
    let graph = AgentGraph::builder()
        .add_node_with_retry(
            "always_fail",
            node!(|_state| async move {
                Err::<(), _>(AgentGraphError::ExecutionError(
                    "permanent failure".to_string(),
                ))
            }),
            RetryPolicy::new()
                .with_max_attempts(3)
                .with_initial_interval(Duration::from_millis(10))
                .with_jitter(false),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("always_fail", state).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::ExecutionError(msg) => {
            assert_eq!(msg, "permanent failure");
        }
        other => panic!("Expected ExecutionError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_with_predicate() {
    let graph = AgentGraph::builder()
        .add_node_with_retry(
            "selective_retry",
            node!(|_state| async move {
                // This error should NOT be retried
                Err::<(), _>(AgentGraphError::StateError("not retryable".to_string()))
            }),
            RetryPolicy::new()
                .with_max_attempts(5)
                .with_initial_interval(Duration::from_millis(10))
                .with_retry_on(|e| {
                    // Only retry execution errors, not state errors
                    matches!(e, AgentGraphError::ExecutionError(_))
                }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("selective_retry", state).await;

    // Should fail immediately without retrying
    assert!(result.is_err());
    match result.unwrap_err() {
        AgentGraphError::StateError(msg) => {
            assert_eq!(msg, "not retryable");
        }
        other => panic!("Expected StateError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_retry_policy_builder() {
    let policy = RetryPolicy::new()
        .with_max_attempts(5)
        .with_initial_interval(Duration::from_secs(2))
        .with_backoff_factor(3.0)
        .with_max_interval(Duration::from_secs(120))
        .with_jitter(false);

    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.initial_interval, Duration::from_secs(2));
    assert_eq!(policy.backoff_factor, 3.0);
    assert_eq!(policy.max_interval, Duration::from_secs(120));
    assert!(!policy.jitter);
}

#[tokio::test]
async fn test_retry_delay_calculation() {
    let policy = RetryPolicy::new()
        .with_initial_interval(Duration::from_secs(1))
        .with_backoff_factor(2.0)
        .with_max_interval(Duration::from_secs(10))
        .with_jitter(false);

    // attempt 0: 1 * 2^0 = 1s
    assert_eq!(policy.delay_for_attempt(0), Duration::from_secs(1));
    // attempt 1: 1 * 2^1 = 2s
    assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(2));
    // attempt 2: 1 * 2^2 = 4s
    assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(4));
    // attempt 3: 1 * 2^3 = 8s
    assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(8));
    // attempt 4: 1 * 2^4 = 16s -> capped at 10s
    assert_eq!(policy.delay_for_attempt(4), Duration::from_secs(10));
}

#[tokio::test]
async fn test_no_retry_for_node_without_policy() {
    // Nodes without retry policy should fail immediately
    let graph = AgentGraph::builder()
        .add_node(
            "no_retry",
            node!(|_state| async move {
                Err::<(), _>(AgentGraphError::ExecutionError(
                    "immediate fail".to_string(),
                ))
            }),
        )
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("no_retry", state).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_retry_events_share_trial_ids_per_concrete_attempt() {
    let events: Arc<Mutex<Vec<GraphEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let graph = AgentGraph::builder()
        .with_event_sink(Arc::new(CallbackEventSink::new(move |event| {
            events_clone.lock().unwrap().push(event);
        })))
        .add_node_with_retry(
            "flaky",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("attempt").await?.unwrap_or(0);
                state.set("attempt", attempt + 1).await?;

                if attempt < 2 {
                    Err(AgentGraphError::ExecutionError("transient failure".into()))
                } else {
                    Ok(())
                }
            }),
            RetryPolicy::new()
                .with_max_attempts(3)
                .with_initial_interval(Duration::from_millis(1))
                .with_jitter(false),
        )
        .build()
        .unwrap();

    graph.execute("flaky", AgentState::new()).await.unwrap();

    let captured = events.lock().unwrap();
    let starts: Vec<(String, String)> = captured
        .iter()
        .filter_map(|event| match event {
            GraphEvent::NodeStart {
                node_id,
                attempt_id,
                trial_id,
                ..
            } if node_id == "flaky" => match (attempt_id.as_ref(), trial_id.as_ref()) {
                (Some(attempt_id), Some(trial_id)) => Some((
                    attempt_id.as_str().to_string(),
                    trial_id.as_str().to_string(),
                )),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let ends: Vec<(String, String, &'static str)> = captured
        .iter()
        .filter_map(|event| match event {
            GraphEvent::NodeEnd {
                node_id,
                outcome,
                attempt_id,
                trial_id,
                ..
            } if node_id == "flaky" => match (attempt_id.as_ref(), trial_id.as_ref()) {
                (Some(attempt_id), Some(trial_id)) => Some((
                    attempt_id.as_str().to_string(),
                    trial_id.as_str().to_string(),
                    match outcome {
                        NodeOutcomeKind::Success => "success",
                        NodeOutcomeKind::Failed => "failed",
                        NodeOutcomeKind::Interrupted => "interrupted",
                    },
                )),
                _ => None,
            },
            _ => None,
        })
        .collect();

    assert_eq!(starts.len(), 3, "each retry trial must emit NodeStart");
    assert_eq!(ends.len(), 3, "each retry trial must emit NodeEnd");

    let canonical_attempt_id = &starts[0].0;
    assert!(
        starts
            .iter()
            .all(|(attempt_id, _)| attempt_id == canonical_attempt_id),
        "all retry trials must share one AttemptId"
    );
    assert!(
        ends.iter()
            .all(|(attempt_id, _, _)| attempt_id == canonical_attempt_id),
        "NodeEnd must preserve the retry family's AttemptId"
    );

    let start_trial_ids: Vec<String> = starts
        .iter()
        .map(|(_, trial_id)| trial_id.clone())
        .collect();
    let end_trial_ids: Vec<String> = ends
        .iter()
        .map(|(_, trial_id, _)| trial_id.clone())
        .collect();
    assert_eq!(
        start_trial_ids, end_trial_ids,
        "each concrete trial must reuse the same TrialId in NodeStart and NodeEnd"
    );

    let unique_trial_ids: HashSet<String> = start_trial_ids.into_iter().collect();
    assert_eq!(
        unique_trial_ids.len(),
        3,
        "each concrete retry must mint a fresh TrialId"
    );
    assert_eq!(ends[0].2, "failed");
    assert_eq!(ends[1].2, "failed");
    assert_eq!(ends[2].2, "success");
}

#[tokio::test]
async fn test_checkpoint_store_records_every_retry_trial() {
    let store = Arc::new(InMemoryCheckpointStore::new());

    let graph = AgentGraph::builder()
        .with_checkpoint_store(store.clone())
        .add_node_with_retry(
            "flaky",
            node!(|state| async move {
                let attempt: u32 = state.get_opt("attempt").await?.unwrap_or(0);
                state.set("attempt", attempt + 1).await?;

                if attempt < 2 {
                    Err(AgentGraphError::ExecutionError("transient failure".into()))
                } else {
                    Ok(())
                }
            }),
            RetryPolicy::new()
                .with_max_attempts(3)
                .with_initial_interval(Duration::from_millis(1))
                .with_jitter(false),
        )
        .build()
        .unwrap();

    graph.execute("flaky", AgentState::new()).await.unwrap();

    let runs = store.list_runs().await;
    assert_eq!(runs.len(), 1);
    let attempts = &runs[0].attempts;
    assert_eq!(
        attempts.len(),
        3,
        "checkpoint store must record each concrete retry"
    );
    assert_eq!(attempts[0].attempt, 0);
    assert_eq!(attempts[1].attempt, 1);
    assert_eq!(attempts[2].attempt, 2);
    assert!(matches!(attempts[0].status, AttemptStatus::Failed));
    assert!(matches!(attempts[1].status, AttemptStatus::Failed));
    assert!(matches!(attempts[2].status, AttemptStatus::Completed));
}
