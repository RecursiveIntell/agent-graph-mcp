#[path = "../src/migrations.rs"]
mod migrations;
#[test]
fn legacy_rows_are_rewritten_and_migration_is_idempotent() {
    let mut c = rusqlite::Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE executions(run_id TEXT PRIMARY KEY,status TEXT NOT NULL); INSERT INTO executions VALUES('r','running');").unwrap();
    migrations::apply(&mut c, "test").unwrap();
    migrations::apply(&mut c, "test").unwrap();
    assert_eq!(
        c.query_row::<String, _, _>("SELECT owner_instance_id FROM executions", [], |r| r.get(0))
            .unwrap(),
        migrations::LEGACY_OWNER_UNKNOWN
    );
    assert_eq!(
        c.query_row::<i64, _, _>("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap(),
        1
    );
}
