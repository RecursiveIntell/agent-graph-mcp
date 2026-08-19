use ri_agent_graph::prelude::*;
use serde_json::json;

#[tokio::test]
async fn test_last_write_wins_reducer() {
    let reducer = LastWriteWins;
    let result = reducer.reduce(&json!(1), &json!(2)).unwrap();
    assert_eq!(result, json!(2));
}

#[tokio::test]
async fn test_append_reducer_arrays() {
    let reducer = AppendReducer;
    let result = reducer.reduce(&json!([1, 2]), &json!([3, 4])).unwrap();
    assert_eq!(result, json!([1, 2, 3, 4]));
}

#[tokio::test]
async fn test_append_reducer_array_and_scalar() {
    let reducer = AppendReducer;
    let result = reducer.reduce(&json!([1, 2]), &json!(3)).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[tokio::test]
async fn test_append_reducer_scalar_and_array() {
    let reducer = AppendReducer;
    let result = reducer.reduce(&json!(1), &json!([2, 3])).unwrap();
    assert_eq!(result, json!([1, 2, 3]));
}

#[tokio::test]
async fn test_append_reducer_two_scalars() {
    let reducer = AppendReducer;
    let result = reducer.reduce(&json!(1), &json!(2)).unwrap();
    assert_eq!(result, json!([1, 2]));
}

#[tokio::test]
async fn test_add_reducer() {
    let reducer = AddReducer;
    let result = reducer.reduce(&json!(10), &json!(5)).unwrap();
    assert_eq!(result, json!(15.0));
}

#[tokio::test]
async fn test_add_reducer_floats() {
    let reducer = AddReducer;
    let result = reducer.reduce(&json!(1.5), &json!(2.5)).unwrap();
    assert_eq!(result, json!(4.0));
}

#[tokio::test]
async fn test_add_reducer_error_on_non_numbers() {
    let reducer = AddReducer;
    let result = reducer.reduce(&json!("hello"), &json!(5));
    assert!(result.is_err());
}

#[tokio::test]
async fn test_merge_reducer_objects() {
    let reducer = MergeReducer;
    let result = reducer
        .reduce(&json!({"a": 1, "b": 2}), &json!({"b": 3, "c": 4}))
        .unwrap();
    assert_eq!(result, json!({"a": 1, "b": 3, "c": 4}));
}

#[tokio::test]
async fn test_merge_reducer_deep() {
    let reducer = MergeReducer;
    let result = reducer
        .reduce(
            &json!({"nested": {"a": 1, "b": 2}}),
            &json!({"nested": {"b": 3, "c": 4}}),
        )
        .unwrap();
    assert_eq!(result, json!({"nested": {"a": 1, "b": 3, "c": 4}}));
}

#[tokio::test]
async fn test_merge_reducer_non_object_replaces() {
    let reducer = MergeReducer;
    let result = reducer.reduce(&json!(1), &json!(2)).unwrap();
    assert_eq!(result, json!(2));
}

#[tokio::test]
async fn test_fn_reducer() {
    // Custom reducer: always takes the maximum
    let reducer = FnReducer::new(|current, update| {
        let a = current.as_f64().unwrap_or(0.0);
        let b = update.as_f64().unwrap_or(0.0);
        Ok(json!(a.max(b)))
    });
    let result = reducer.reduce(&json!(5), &json!(3)).unwrap();
    assert_eq!(result, json!(5.0));

    let result = reducer.reduce(&json!(3), &json!(5)).unwrap();
    assert_eq!(result, json!(5.0));
}

#[tokio::test]
async fn test_state_with_reducer() {
    let state = AgentState::new();
    state.register_reducer("items", AppendReducer).await;

    // First set: no existing value, just stores
    state.set("items", vec![1, 2]).await.unwrap();
    let items: Vec<i32> = state.get("items").await.unwrap();
    assert_eq!(items, vec![1, 2]);

    // Second set: uses reducer to append
    state.set("items", vec![3, 4]).await.unwrap();
    let items: Vec<i32> = state.get("items").await.unwrap();
    assert_eq!(items, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn test_state_add_reducer() {
    let state = AgentState::new();
    state.register_reducer("score", AddReducer).await;

    state.set("score", 10).await.unwrap();
    state.set("score", 5).await.unwrap();

    let score: f64 = state.get("score").await.unwrap();
    assert_eq!(score, 15.0);
}

#[tokio::test]
async fn test_graph_builder_with_reducer() {
    let graph = AgentGraph::builder()
        .add_node(
            "step1",
            node!(|state| async move {
                state.set("log", vec!["step1".to_string()]).await?;
                Ok(())
            }),
        )
        .add_node(
            "step2",
            node!(|state| async move {
                state.set("log", vec!["step2".to_string()]).await?;
                Ok(())
            }),
        )
        .add_edge("step1", "step2")
        .with_reducer("log", AppendReducer)
        .build()
        .unwrap();

    let state = AgentState::new();
    let result = graph.execute("step1", state).await.unwrap();

    let log: Vec<String> = result.get("log").await.unwrap();
    assert_eq!(log, vec!["step1", "step2"]);
}
