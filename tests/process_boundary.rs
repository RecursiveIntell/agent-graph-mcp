#[path = "../src/daemon.rs"]
mod daemon;
#[path = "../src/lifecycle.rs"]
mod lifecycle;
#[path = "../src/migrations.rs"]
mod migrations;
#[test]
fn second_daemon_fails_before_opening_database() {
    let d = tempfile::tempdir().unwrap();
    let (_lock, _db) = daemon::open_owned(d.path(), "a").unwrap();
    let e = daemon::DaemonLock::acquire(d.path()).unwrap_err();
    assert_eq!(e.code(), "DATA_DIR_ALREADY_OWNED");
}
#[test]
fn timeout_is_completion_unknown_and_requests_cancel() {
    let d = lifecycle::synchronous_timeout();
    assert!(d.completion_unknown && d.cancellation_requested);
}
