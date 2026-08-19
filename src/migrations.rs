//! Versioned durable-daemon migrations. Kept independent so the store can call it.
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

pub const CURRENT_VERSION: i64 = 6;
pub const LEGACY_OWNER_UNKNOWN: &str = "legacy_owner_unknown";

#[allow(dead_code)]
pub trait MigrationStore {
    fn connection(&self) -> &Connection;
}

pub fn apply(conn: &mut Connection, binary_digest: &str) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, migration_digest TEXT NOT NULL);")?;
    let exists: Option<i64> = tx
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = ?1",
            [CURRENT_VERSION],
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_none() {
        tx.execute_batch("CREATE TABLE IF NOT EXISTS daemon_instances (instance_id TEXT PRIMARY KEY, generation INTEGER NOT NULL UNIQUE, pid INTEGER NOT NULL, boot_id TEXT, executable_digest TEXT, started_at TEXT NOT NULL, heartbeat_at TEXT NOT NULL, clean_shutdown_at TEXT); CREATE TABLE IF NOT EXISTS run_publication_state (run_id TEXT PRIMARY KEY, state TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, reason TEXT); CREATE TABLE IF NOT EXISTS operator_receipts (receipt_id TEXT PRIMARY KEY, request_digest TEXT NOT NULL, action TEXT NOT NULL, resource_kind TEXT NOT NULL, resource_id TEXT NOT NULL, state_digest TEXT NOT NULL, operator_uid INTEGER NOT NULL, daemon_instance_id TEXT NOT NULL, nonce TEXT NOT NULL UNIQUE, issued_at TEXT NOT NULL, expires_at TEXT NOT NULL, consumed_at TEXT); CREATE INDEX IF NOT EXISTS idx_operator_receipts_nonce ON operator_receipts(nonce);")?;
        let has_owner: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM pragma_table_info('executions') WHERE name='owner_instance_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if has_owner.is_none() {
            let has_executions: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='executions'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if has_executions.is_some() {
                tx.execute_batch("ALTER TABLE executions ADD COLUMN owner_instance_id TEXT;")?;
                tx.execute("UPDATE executions SET owner_instance_id = ?1 WHERE owner_instance_id IS NULL AND status IN ('accepted','running')", [LEGACY_OWNER_UNKNOWN])?;
            }
        }
        // v5: authenticate the independently persisted terminal bundle.
        let has_terminal_receipts: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='terminal_receipts'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if has_terminal_receipts.is_some() {
            let has_bundle_digest: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM pragma_table_info('terminal_receipts') WHERE name='bundle_digest'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if has_bundle_digest.is_none() {
                tx.execute_batch("ALTER TABLE terminal_receipts ADD COLUMN bundle_digest TEXT;")?;
            }
        }

        // v4: deletion governance — phantom remediation + retention lifecycle
        let has_executions: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='executions'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if has_executions.is_some() {
            let has_superseded: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM pragma_table_info('executions') WHERE name='superseded_by'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if has_superseded.is_none() {
                tx.execute_batch(
                    "ALTER TABLE executions ADD COLUMN superseded_by TEXT DEFAULT NULL;",
                )?;
            }
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS legal_holds (\
                hold_id TEXT PRIMARY KEY,\
                graph_name TEXT NOT NULL,\
                reason TEXT,\
                issued_at TEXT NOT NULL DEFAULT (datetime('now')),\
                expires_at TEXT,\
                issued_by TEXT\
            );\
            CREATE TABLE IF NOT EXISTS archive_manifests (\
                graph_name TEXT PRIMARY KEY,\
                graph_version TEXT,\
                spec_digest TEXT,\
                version_count INTEGER,\
                last_execution_at TEXT,\
                execution_count INTEGER,\
                content_digest TEXT,\
                created_at TEXT NOT NULL DEFAULT (datetime('now'))\
            );\
            CREATE TABLE IF NOT EXISTS gc_policy (\
                id INTEGER PRIMARY KEY CHECK (id = 1),\
                enabled INTEGER NOT NULL DEFAULT 1,\
                idle_archive_days INTEGER NOT NULL DEFAULT 30,\
                review_expire_days INTEGER NOT NULL DEFAULT 60,\
                min_executions INTEGER NOT NULL DEFAULT 5,\
                storage_flag_mb INTEGER NOT NULL DEFAULT 10,\
                last_run TEXT\
            );\
            INSERT OR IGNORE INTO gc_policy (id) VALUES (1);",
        )?;

        let digest = migration_digest(binary_digest);
        tx.execute(
            "INSERT INTO schema_migrations(version,migration_digest) VALUES (?1,?2)",
            params![CURRENT_VERSION, digest],
        )?;
    }
    tx.commit()
}

pub fn migration_digest(binary_digest: &str) -> String {
    let mut h = Sha256::new();
    h.update(format!(
        "agent-graph-mcp:migration:{CURRENT_VERSION}:{binary_digest}"
    ));
    format!("{:x}", h.finalize())
}

#[allow(dead_code)]
pub fn owner_for_new_run(owner: &str) -> rusqlite::Result<&str> {
    if owner.is_empty() {
        Err(rusqlite::Error::InvalidParameterName(
            "owner_instance_id".into(),
        ))
    } else {
        Ok(owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_active_rows_are_quarantined() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE executions(run_id TEXT PRIMARY KEY,status TEXT NOT NULL); INSERT INTO executions VALUES ('a','running'),('b','completed');").unwrap();
        apply(&mut c, "bin").unwrap();
        let owner: String = c
            .query_row(
                "SELECT owner_instance_id FROM executions WHERE run_id='a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(owner, LEGACY_OWNER_UNKNOWN);
        apply(&mut c, "bin").unwrap();
        assert_eq!(
            c.query_row::<i64, _, _>("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn bundle_digest_column_is_added_by_version_five() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE terminal_receipts(
                run_id TEXT PRIMARY KEY,
                receipt_json TEXT NOT NULL,
                bundle_json TEXT NOT NULL,
                receipt_digest TEXT NOT NULL
            );
            CREATE TABLE schema_migrations(
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                migration_digest TEXT NOT NULL
            );
            INSERT INTO schema_migrations(version,migration_digest)
                VALUES (4,'legacy');",
        )
        .unwrap();
        apply(&mut c, "bin").unwrap();
        let has_bundle_digest: i64 = c
            .query_row(
                "SELECT 1 FROM pragma_table_info('terminal_receipts') WHERE name='bundle_digest'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has_bundle_digest, 1);
        assert_eq!(
            c.query_row::<i64, _, _>(
                "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap(),
            CURRENT_VERSION
        );
    }
}
