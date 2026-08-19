use ri_agent_graph::prelude::*;

#[tokio::test]
async fn test_state_set_get() {
    let state = AgentState::new();

    state.set("name", "Alice").await.unwrap();
    state.set("age", 30u32).await.unwrap();

    let name: String = state.get("name").await.unwrap();
    let age: u32 = state.get("age").await.unwrap();

    assert_eq!(name, "Alice");
    assert_eq!(age, 30);
}

#[tokio::test]
async fn test_state_get_missing_key() {
    let state = AgentState::new();

    let result = state.get::<String>("missing").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_state_get_opt() {
    let state = AgentState::new();

    let missing: Option<String> = state.get_opt("missing").await.unwrap();
    assert!(missing.is_none());

    state.set("key", "value").await.unwrap();
    let found: Option<String> = state.get_opt("key").await.unwrap();
    assert_eq!(found, Some("value".to_string()));
}

#[tokio::test]
async fn test_state_update() {
    let state = AgentState::new();
    state.set("counter", 0u32).await.unwrap();

    state.update::<u32, _>("counter", |c| c + 1).await.unwrap();
    state.update::<u32, _>("counter", |c| c + 1).await.unwrap();

    let counter: u32 = state.get("counter").await.unwrap();
    assert_eq!(counter, 2);
}

#[tokio::test]
async fn test_state_contains() {
    let state = AgentState::new();

    assert!(!state.contains("key").await);
    state.set("key", "value").await.unwrap();
    assert!(state.contains("key").await);
}

#[tokio::test]
async fn test_state_remove() {
    let state = AgentState::new();
    state.set("key", "value").await.unwrap();

    let removed = state.remove("key").await;
    assert!(removed.is_some());
    assert!(!state.contains("key").await);
}

#[tokio::test]
async fn test_state_keys() {
    let state = AgentState::new();
    state.set("a", 1).await.unwrap();
    state.set("b", 2).await.unwrap();
    state.set("c", 3).await.unwrap();

    let mut keys = state.keys().await;
    keys.sort();
    assert_eq!(keys, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn test_state_snapshot_restore() {
    let state = AgentState::new();
    state.set("value", 100).await.unwrap();

    let snapshot = state.snapshot().await;

    state.set("value", 200).await.unwrap();

    state.restore(&snapshot).await;
    let value: i32 = state.get("value").await.unwrap();

    assert_eq!(value, 100);
}

#[tokio::test]
async fn test_state_history() {
    let state = AgentState::new();
    state.set("step", 1).await.unwrap();
    state.save_to_history().await;

    state.set("step", 2).await.unwrap();
    state.save_to_history().await;

    let history = state.get_history().await;
    assert_eq!(history.len(), 2);
}

#[tokio::test]
async fn test_state_with_data() {
    use serde_json::json;
    use std::collections::HashMap;

    let mut data = HashMap::new();
    data.insert("name".to_string(), json!("Bob"));
    data.insert("score".to_string(), json!(42));

    let state = AgentState::with_data(data);

    let name: String = state.get("name").await.unwrap();
    let score: i32 = state.get("score").await.unwrap();

    assert_eq!(name, "Bob");
    assert_eq!(score, 42);
}

#[tokio::test]
async fn test_state_export() {
    let state = AgentState::new();
    state.set("x", 10).await.unwrap();
    state.set("y", 20).await.unwrap();

    let exported = state.export().await;
    assert_eq!(exported.len(), 2);
    assert!(exported.contains_key("x"));
    assert!(exported.contains_key("y"));
}

#[tokio::test]
async fn test_state_concurrent_access() {
    let state = AgentState::new();
    state.set("counter", 0i32).await.unwrap();

    let mut handles = vec![];

    for _ in 0..10 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                s.update::<i32, _>("counter", |c| c + 1).await.unwrap();
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let counter: i32 = state.get("counter").await.unwrap();
    assert_eq!(counter, 1000);
}

#[tokio::test]
async fn test_state_complex_types() {
    let state = AgentState::new();

    // Vec
    state.set("list", vec![1, 2, 3]).await.unwrap();
    let list: Vec<i32> = state.get("list").await.unwrap();
    assert_eq!(list, vec![1, 2, 3]);

    // Nested struct via serde_json::Value
    use serde_json::json;
    state
        .set(
            "config",
            json!({
                "name": "test",
                "values": [1, 2, 3]
            }),
        )
        .await
        .unwrap();

    let config: serde_json::Value = state.get("config").await.unwrap();
    assert_eq!(config["name"], "test");
}
