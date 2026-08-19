use criterion::{criterion_group, criterion_main, Criterion};

fn bench_graph_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("linear_graph_3_nodes", |b| {
        b.iter(|| {
            rt.block_on(async {
                use ri_agent_graph::prelude::*;

                let graph = AgentGraph::builder()
                    .add_node(
                        "a",
                        node!(|state| async move {
                            state.set("count", 1i32).await?;
                            Ok(())
                        }),
                    )
                    .add_node(
                        "b",
                        node!(|state| async move {
                            let c: i32 = state.get("count").await?;
                            state.set("count", c + 1).await?;
                            Ok(())
                        }),
                    )
                    .add_node(
                        "c",
                        node!(|state| async move {
                            let c: i32 = state.get("count").await?;
                            state.set("count", c + 1).await?;
                            Ok(())
                        }),
                    )
                    .add_edge("a", "b")
                    .add_edge("b", "c")
                    .build()
                    .unwrap();

                let state = AgentState::new();
                graph.execute("a", state).await.unwrap();
            });
        });
    });

    c.bench_function("loop_graph_10_iterations", |b| {
        b.iter(|| {
            rt.block_on(async {
                use ri_agent_graph::prelude::*;

                let graph = AgentGraph::builder()
                    .add_node(
                        "increment",
                        node!(|state| async move {
                            let count: u32 = state.get_opt("count").await?.unwrap_or(0);
                            state.set("count", count + 1).await?;
                            Ok(())
                        }),
                    )
                    .add_conditional_edge(
                        "increment",
                        router!(|state| async move {
                            let count: u32 = state.get("count").await?;
                            Ok(if count < 10 {
                                Some("increment".to_string())
                            } else {
                                None
                            })
                        }),
                    )
                    .build()
                    .unwrap();

                let state = AgentState::new();
                graph.execute("increment", state).await.unwrap();
            });
        });
    });

    c.bench_function("state_read_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                use ri_agent_graph::state::AgentState;

                let state = AgentState::new();
                for i in 0..100 {
                    state.set(&format!("key_{}", i), i).await.unwrap();
                }
                for i in 0..100 {
                    let _: i32 = state.get(&format!("key_{}", i)).await.unwrap();
                }
            });
        });
    });
}

criterion_group!(benches, bench_graph_execution);
criterion_main!(benches);
