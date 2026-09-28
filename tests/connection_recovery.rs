use agent_graph_mcp::{server::AgentGraphServer, store::PersistentStore};

fn server(data_dir: &std::path::Path) -> AgentGraphServer {
    AgentGraphServer::new_with_max_graphs_and_key(
        "http://127.0.0.1:11434".into(),
        "test-model".into(),
        Some(data_dir.to_path_buf()),
        None,
        64,
        None,
    )
    .expect("server")
}

#[test]
fn a_second_daemon_connection_does_not_recover_a_healthy_active_run() {
    let temp = tempfile::tempdir().expect("temp dir");
    let _startup = server(temp.path());
    let store = PersistentStore::open(temp.path()).expect("store");
    store
        .save_graph("active-graph", "{}", "hash", false)
        .expect("graph");
    store
        .save_execution("run-active", "active-graph", "hash", "running", "{}")
        .expect("active execution");

    // Daemon connections must share their existing owner, not construct one.
    let _connection = _startup.clone();
    let run = store.load_execution("run-active").expect("load execution");
    assert_eq!(run.expect("active run")["status"], "running");
}

#[test]
fn daemon_startup_recovers_work_left_interrupted_by_a_real_restart() {
    let temp = tempfile::tempdir().expect("temp dir");
    {
        let store = PersistentStore::open(temp.path()).expect("store before restart");
        store
            .save_graph("restart-graph", "{}", "hash", false)
            .expect("graph");
        store
            .save_execution("run-restart", "restart-graph", "hash", "running", "{}")
            .expect("running execution");
    }

    let _restarted_daemon = server(temp.path());
    let store = PersistentStore::open(temp.path()).expect("store after restart");
    let run = store
        .load_execution("run-restart")
        .expect("load recovered execution")
        .expect("execution row");
    assert_eq!(run["status"], "interrupted_non_resumable");
}
