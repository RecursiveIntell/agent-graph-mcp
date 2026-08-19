use ri_agent_graph::prelude::*;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== State Reducers Example ===\n");

    // --- Example 1: AppendReducer for collecting items ---
    println!("--- AppendReducer: Collecting Items ---");
    {
        let graph = AgentGraph::builder()
            .add_node(
                "add_fruits",
                node!(|state| async move {
                    state
                        .set("items", vec!["apple".to_string(), "banana".to_string()])
                        .await?;
                    println!("  Added fruits: [apple, banana]");
                    Ok(())
                }),
            )
            .add_node(
                "add_vegs",
                node!(|state| async move {
                    state
                        .set("items", vec!["carrot".to_string(), "daikon".to_string()])
                        .await?;
                    println!("  Added vegetables: [carrot, daikon]");
                    Ok(())
                }),
            )
            .add_edge("add_fruits", "add_vegs")
            .with_reducer("items", AppendReducer)
            .build()?;

        let state = AgentState::new();
        let result = graph.execute("add_fruits", state).await?;

        let items: Vec<String> = result.get("items").await?;
        println!("  Final items: {:?}\n", items);
    }

    // --- Example 2: AddReducer for accumulating scores ---
    println!("--- AddReducer: Accumulating Scores ---");
    {
        let graph = AgentGraph::builder()
            .add_node(
                "quiz_1",
                node!(|state| async move {
                    state.set("score", 25i64).await?;
                    println!("  Quiz 1 score: 25");
                    Ok(())
                }),
            )
            .add_node(
                "quiz_2",
                node!(|state| async move {
                    state.set("score", 30i64).await?;
                    println!("  Quiz 2 score: 30");
                    Ok(())
                }),
            )
            .add_node(
                "quiz_3",
                node!(|state| async move {
                    state.set("score", 45i64).await?;
                    println!("  Quiz 3 score: 45");
                    Ok(())
                }),
            )
            .add_edge("quiz_1", "quiz_2")
            .add_edge("quiz_2", "quiz_3")
            .with_reducer("score", AddReducer)
            .build()?;

        let state = AgentState::new();
        let result = graph.execute("quiz_1", state).await?;

        let score: f64 = result.get("score").await?;
        println!("  Total score: {}\n", score);
    }

    // --- Example 3: MergeReducer for combining configs ---
    println!("--- MergeReducer: Deep Merging Objects ---");
    {
        let state = AgentState::new();
        state.register_reducer("config", MergeReducer).await;

        state
            .set(
                "config",
                json!({"database": {"host": "localhost", "port": 5432}}),
            )
            .await?;
        println!("  Set: database.host=localhost, database.port=5432");

        state
            .set(
                "config",
                json!({"database": {"name": "mydb"}, "cache": {"enabled": true}}),
            )
            .await?;
        println!("  Merged: database.name=mydb, cache.enabled=true");

        let config: serde_json::Value = state.get("config").await?;
        println!(
            "  Final config: {}\n",
            serde_json::to_string_pretty(&config)?
        );
    }

    // --- Example 4: Custom FnReducer ---
    println!("--- FnReducer: Custom Max Reducer ---");
    {
        let state = AgentState::new();
        state
            .register_reducer(
                "max_score",
                FnReducer::new(|current, update| {
                    let a = current.as_f64().unwrap_or(0.0);
                    let b = update.as_f64().unwrap_or(0.0);
                    Ok(json!(a.max(b)))
                }),
            )
            .await;

        let scores = [72.5, 95.0, 88.3, 91.2, 85.0];
        for score in scores {
            state.set("max_score", score).await?;
            println!("  Checked score: {}", score);
        }

        let max: f64 = state.get("max_score").await?;
        println!("  Highest score: {}", max);

        Ok(())
    }
}
