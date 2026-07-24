use rusqlite::Connection;
#[test]
fn terminal_projection_rolls_back_as_one_unit() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE terminal(id TEXT PRIMARY KEY, receipt TEXT NOT NULL, digest TEXT NOT NULL);",
    )
    .unwrap();
    let tx = c.unchecked_transaction().unwrap();
    tx.execute("INSERT INTO terminal VALUES('run','receipt','digest')", [])
        .unwrap();
    assert!(tx
        .execute(
            "INSERT INTO terminal VALUES('run','receipt2','digest2')",
            []
        )
        .is_err());
    tx.rollback().unwrap();
    assert_eq!(
        c.query_row::<i64, _, _>("SELECT count(*) FROM terminal", [], |r| r.get(0))
            .unwrap(),
        0
    );
}
