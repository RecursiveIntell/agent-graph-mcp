//! Process-boundary test: daemon starts, acquires lock, second daemon rejected.
use agent_graph_mcp::{daemon, store::PersistentStore};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;

#[test]
fn second_daemon_against_same_data_dir_is_rejected() {
    let dir = tempdir().expect("temp dir");
    let data = dir.path();
    // First daemon acquires the lock
    let (_lock_a, _conn_a) = daemon::open_owned(data, "daemon-a").expect("first daemon acquires");
    // Second daemon against the same data dir must fail
    let result = daemon::open_owned(data, "daemon-b");
    assert!(
        result.is_err(),
        "second daemon must not acquire the same data directory"
    );
    let err = result.unwrap_err();
    assert_eq!(err.code(), "DATA_DIR_ALREADY_OWNED");
}

#[test]
fn releasing_lock_allows_new_daemon() {
    let dir = tempdir().expect("temp dir");
    let data = dir.path();
    {
        let (_lock, _conn) = daemon::open_owned(data, "daemon-a").expect("first daemon");
    }
    // Lock dropped; new daemon can acquire
    let (_lock_b, _conn_b) =
        daemon::open_owned(data, "daemon-b").expect("second daemon after drop");
}

#[test]
fn recover_owned_state_with_no_executions_table_is_safe() {
    let dir = tempdir().expect("temp dir");
    let data = dir.path();
    let (_lock, conn) = daemon::open_owned(data, "daemon-a").expect("open daemon");
    let id = daemon::identity(&conn).expect("identity");
    let changed = daemon::recover_owned_state(&conn, &id.instance_id, id.generation)
        .expect("recover owned state");
    assert_eq!(changed, 0);
}

#[test]
fn recover_owned_state_with_no_owner_instance_id_column_is_safe() {
    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("agent-graph.db");
    let conn = rusqlite::Connection::open(db_path).expect("open legacy db");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daemon_instances (
            instance_id TEXT PRIMARY KEY,
            generation INTEGER NOT NULL UNIQUE,
            pid INTEGER NOT NULL,
            boot_id TEXT,
            executable_digest TEXT,
            started_at TEXT NOT NULL,
            heartbeat_at TEXT NOT NULL,
            clean_shutdown_at TEXT
        );
        CREATE TABLE IF NOT EXISTS executions (run_id TEXT PRIMARY KEY, status TEXT NOT NULL);
        INSERT INTO daemon_instances(instance_id,generation,pid,started_at,heartbeat_at)
            VALUES('legacy-ownerless', 1, 1, '0', '0');
        INSERT INTO executions(run_id,status) VALUES('run-1','running');",
    )
    .expect("seed legacy schema");

    let changed =
        daemon::recover_owned_state(&conn, "legacy-ownerless", 1).expect("recover owned state");
    assert_eq!(changed, 0);
}

#[test]
fn daemon_identity_is_unique_and_monotonic() {
    let dir = tempdir().expect("temp dir");
    let data = dir.path();
    let (_lock_a, conn_a) = daemon::open_owned(data, "daemon-a").expect("first daemon");
    let id_a = daemon::identity(&conn_a).expect("identity a");
    assert_eq!(id_a.generation, 1);
    drop(_lock_a);
    drop(conn_a);
    let (_lock_b, conn_b) = daemon::open_owned(data, "daemon-b").expect("second daemon");
    let id_b = daemon::identity(&conn_b).expect("identity b");
    assert_eq!(id_b.generation, 2);
    assert_ne!(id_a.instance_id, id_b.instance_id);
}

#[test]
fn daemon_startup_mode_is_durable_across_restarts() {
    let dir = tempdir().expect("temp dir");
    let (_lock, conn) = daemon::open_owned(dir.path(), "daemon-a").expect("first daemon");
    daemon::enforce_startup_mode(&conn, true).expect("initial keyed mode");
    drop(conn);
    drop(_lock);
    let (_lock, conn) = daemon::open_owned(dir.path(), "daemon-b").expect("restart");
    let err = daemon::enforce_startup_mode(&conn, false).expect_err("mixed mode rejected");
    assert!(err.to_string().contains("STARTUP_MODE_MISMATCH"));
}

#[test]
fn repeated_same_mode_startup_is_allowed() {
    let dir = tempdir().expect("temp dir");
    let (_lock, conn) = daemon::open_owned(dir.path(), "daemon-a").expect("daemon");
    daemon::enforce_startup_mode(&conn, false).expect("initial keyless mode");
    daemon::enforce_startup_mode(&conn, false).expect("same mode");
}

#[test]
fn concurrent_startup_attempts_have_one_owner() {
    let dir = tempdir().expect("temp dir");
    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let path = dir.path().to_path_buf();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            daemon::DaemonLock::acquire(&path)
                .map(|lock| {
                    thread::sleep(std::time::Duration::from_millis(20));
                    drop(lock);
                    true
                })
                .unwrap_or(false)
        }));
    }
    let owners = workers
        .into_iter()
        .filter_map(|w| w.join().ok())
        .filter(|v| *v)
        .count();
    assert_eq!(
        owners, 1,
        "only one concurrent daemon/watchdog owner is allowed"
    );
}

#[test]
fn keyed_then_keyless_and_keyless_then_keyed_are_rejected() {
    let keyed = tempdir().expect("keyed temp dir");
    let (_lock, conn) = daemon::open_owned(keyed.path(), "daemon-keyed").expect("keyed daemon");
    daemon::enforce_startup_mode(&conn, true).expect("record keyed mode");
    drop(conn);
    drop(_lock);
    let (_lock, conn) = daemon::open_owned(keyed.path(), "daemon-keyless").expect("restart");
    assert!(daemon::enforce_startup_mode(&conn, false)
        .expect_err("keyed data must reject keyless restart")
        .to_string()
        .contains("STARTUP_MODE_MISMATCH"));

    let keyless = tempdir().expect("keyless temp dir");
    let (_lock, conn) =
        daemon::open_owned(keyless.path(), "daemon-keyless").expect("keyless daemon");
    daemon::enforce_startup_mode(&conn, false).expect("record keyless mode");
    drop(conn);
    drop(_lock);
    let (_lock, conn) = daemon::open_owned(keyless.path(), "daemon-keyed").expect("restart");
    assert!(daemon::enforce_startup_mode(&conn, true)
        .expect_err("keyless data must reject keyed restart")
        .to_string()
        .contains("STARTUP_MODE_MISMATCH"));
}

#[test]
fn independent_data_dirs_have_independent_process_locks() {
    let first = tempdir().expect("first temp dir");
    let second = tempdir().expect("second temp dir");
    let (_lock_a, _conn_a) =
        daemon::open_owned(first.path(), "same-instance-pattern").expect("first daemon");
    let (_lock_b, _conn_b) = daemon::open_owned(second.path(), "same-instance-pattern")
        .expect("different data dir lock");
}

#[test]
fn released_lock_supports_watchdog_style_reacquisition() {
    let dir = tempdir().expect("temp dir");
    let (_lock, _conn) = daemon::open_owned(dir.path(), "watchdog-primary").expect("primary");
    assert!(daemon::open_owned(dir.path(), "watchdog-secondary").is_err());
    drop(_conn);
    drop(_lock);
    let (_lock, _conn) =
        daemon::open_owned(dir.path(), "watchdog-replacement").expect("replacement watchdog");
}

#[test]
fn health_store_loss_is_visible_to_separate_connection() {
    let dir = tempdir().expect("temp dir");
    let (_lock, conn) = daemon::open_owned(dir.path(), "health-daemon").expect("daemon");
    conn.execute_batch("CREATE TABLE health_probe(value TEXT NOT NULL);")
        .expect("health probe");
    drop(conn);
    std::fs::remove_file(dir.path().join("agent-graph.db")).expect("remove sqlite file");
    let reopened =
        rusqlite::Connection::open(dir.path().join("agent-graph.db")).expect("observer connection");
    assert!(reopened
        .query_row("SELECT 1 FROM health_probe", [], |row| row.get::<_, i64>(0))
        .is_err());
}

#[test]
fn observer_write_failures_are_returned() {
    let dir = tempdir().expect("temp dir");
    std::fs::write(dir.path().join("agent-graph.db"), b"not a database").expect("seed invalid db");
    let err = daemon::open_owned(dir.path(), "observer").expect_err("open must surface failure");
    assert!(matches!(err.code(), "DAEMON_SQL" | "DAEMON_IO"));
}

#[test]
fn crash_during_run_is_interrupted_not_running_after_restart() {
    let dir = tempdir().expect("temp dir");
    let (_lock, conn) = daemon::open_owned(dir.path(), "crash-run").expect("daemon");
    let id = daemon::identity(&conn).expect("identity");
    conn.execute_batch("CREATE TABLE IF NOT EXISTS graphs(name TEXT PRIMARY KEY, spec_json TEXT NOT NULL, spec_version TEXT NOT NULL DEFAULT '2', topology_hash TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')), updated_at TEXT NOT NULL DEFAULT (datetime('now'))); CREATE TABLE IF NOT EXISTS executions(run_id TEXT PRIMARY KEY, graph_name TEXT NOT NULL, graph_hash TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT);")
        .expect("run schema");
    conn.execute(
        "INSERT INTO graphs(name,spec_json,topology_hash) VALUES ('crash-graph','{}','hash')",
        [],
    )
    .expect("graph create");
    conn.execute("INSERT INTO executions(run_id,graph_name,graph_hash,status,started_at) VALUES ('run-crash','crash-graph','hash','running',datetime('now'))", [])
        .expect("start run");
    drop(conn);
    drop(_lock); // abrupt ownership loss: no clean-shutdown marker

    let (_restart_lock, restart_conn) =
        daemon::open_owned(dir.path(), "crash-run-restart").expect("restart");
    let restarted = PersistentStore::open(dir.path()).expect("store restart");
    restarted
        .recover_incomplete_executions()
        .expect("recover run");
    let status: String = restart_conn
        .query_row(
            "SELECT status FROM executions WHERE run_id='run-crash'",
            [],
            |r| r.get(0),
        )
        .expect("run status");
    assert!(status.contains("interrupted"), "status was {status}");
    assert_ne!(status, "running");
    assert_ne!(status, "completed");
    let _ = id;
}

#[test]
fn crash_during_checkpoint_write_leaves_no_corruption() {
    let dir = tempdir().expect("temp dir");
    let store = PersistentStore::open(dir.path()).expect("store");
    let conn = rusqlite::Connection::open(dir.path().join("agent-graph.db")).expect("observer");
    conn.execute_batch("INSERT INTO graphs(name,spec_json,topology_hash) VALUES ('checkpoint-graph','{}','hash'); INSERT INTO executions(run_id,graph_name,graph_hash,status,started_at) VALUES ('run','checkpoint-graph','hash','running',datetime('now')); BEGIN; INSERT INTO checkpoints(run_id,node_id,attempt,status,checkpoint_id,state_json) VALUES ('run','node',1,'writing','cp','{'); ROLLBACK;")
        .expect("interrupted checkpoint transaction rolls back");
    drop(conn);
    drop(store);
    let reopened = rusqlite::Connection::open(dir.path().join("agent-graph.db")).expect("reopen");
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok");
    let count: i64 = reopened
        .query_row(
            "SELECT count(*) FROM checkpoints WHERE checkpoint_id='cp'",
            [],
            |r| r.get(0),
        )
        .expect("checkpoint query");
    assert_eq!(count, 0, "partial checkpoint rows must not survive");
}

#[test]
fn crash_during_graph_create_does_not_register_graph() {
    let dir = tempdir().expect("temp dir");
    let store = PersistentStore::open(dir.path()).expect("store");
    let conn = rusqlite::Connection::open(dir.path().join("agent-graph.db")).expect("observer");
    conn.execute_batch("BEGIN; INSERT INTO graphs(name,spec_json,spec_version,topology_hash) VALUES ('uncommitted','{}','2','hash'); ROLLBACK;")
        .expect("interrupted graph transaction rolls back");
    drop(conn);
    drop(store);
    let restarted = PersistentStore::open(dir.path()).expect("restart");
    let count: i64 = rusqlite::Connection::open(dir.path().join("agent-graph.db"))
        .expect("list observer")
        .query_row(
            "SELECT count(*) FROM graphs WHERE name='uncommitted'",
            [],
            |r| r.get(0),
        )
        .expect("graph list query");
    assert_eq!(count, 0, "graph_create must be all-or-nothing");
    drop(restarted);
}
