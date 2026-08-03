//! SQLite-backed persistent storage for graphs, executions, checkpoints, and events.
//!
//! Enabled when `--data-dir` is passed to the server binary. Without it,
//! everything is in-memory and lost on restart.

use crate::evidence::{
    digest, hmac_sha256, validate_witness_capture_with_key, verify_witness_record_with_key,
    WitnessCapture, WitnessError, WitnessRecord,
};
use crate::operator_auth::OperatorAction;
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Persistent store wrapping a SQLite connection.
pub struct PersistentStore {
    conn: std::sync::Arc<Mutex<Connection>>,
    integrity_key: Option<std::sync::Arc<[u8]>>,
    #[cfg(test)]
    terminal_projection_fault: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    checkpoint_persistence_fault: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    graph_delete_fault: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointRecord {
    pub checkpoint_id: String,
    pub run_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub next_node_cursor: String,
    pub state: Value,
    pub state_digest: String,
    pub budgets: Value,
    pub budget_counters: Value,
    pub dependency_summary: Value,
    pub dependency_digest: String,
    pub terminal_cursor: u64,
    pub event_cursor: u64,
    pub checkpoint_digest: String,
    pub created_at: String,
    pub consumed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    NotFound,
    Consumed,
    Integrity,
    Persistence,
    IntegrityKeyRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    NotFound,
    Conflict,
    AlreadyDecided,
    Expired,
    DecisionNotAllowed,
    Integrity,
    Checkpoint(CheckpointError),
    Persistence,
    IntegrityKeyRequired,
}

impl ApprovalError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "APPROVAL_NOT_FOUND",
            Self::Conflict => "APPROVAL_REQUEST_CONFLICT",
            Self::AlreadyDecided => "APPROVAL_ALREADY_DECIDED",
            Self::Expired => "APPROVAL_EXPIRED",
            Self::DecisionNotAllowed => "APPROVAL_DECISION_NOT_ALLOWED",
            Self::Integrity => "APPROVAL_INTEGRITY_FAILURE",
            Self::Checkpoint(error) => error.code(),
            Self::Persistence => "APPROVAL_PERSISTENCE_FAILURE",
            Self::IntegrityKeyRequired => "INTEGRITY_KEY_REQUIRED",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::NotFound => "approval was not found".into(),
            Self::Conflict => {
                "a conflicting pending approval already exists for this checkpoint and audience"
                    .into()
            }
            Self::AlreadyDecided => "approval has already been decided".into(),
            Self::Expired => "approval has expired".into(),
            Self::DecisionNotAllowed => "decision is not allowed by this approval".into(),
            Self::Integrity => "approval integrity validation failed".into(),
            Self::Checkpoint(error) => error.message().into(),
            Self::Persistence => "approval persistence failed".into(),
            Self::IntegrityKeyRequired => {
                "integrity key is required for durable approval operations".into()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub checkpoint_id: String,
    pub run_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub checkpoint_digest: String,
    pub audience: String,
    pub prompt_digest: String,
    pub allowed_decisions: Vec<String>,
    pub approval_digest: String,
    pub status: String,
    pub decision: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ApprovedCheckpoint {
    pub approval: ApprovalRecord,
    pub checkpoint: CheckpointRecord,
}

impl CheckpointError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "CHECKPOINT_NOT_FOUND",
            Self::Consumed => "CHECKPOINT_CONSUMED",
            Self::Integrity => "CHECKPOINT_INTEGRITY_FAILURE",
            Self::Persistence => "CHECKPOINT_PERSISTENCE_FAILURE",
            Self::IntegrityKeyRequired => "INTEGRITY_KEY_REQUIRED",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::NotFound => "checkpoint was not found",
            Self::Consumed => "checkpoint has already been consumed",
            Self::Integrity => "checkpoint integrity validation failed",
            Self::Persistence => "checkpoint persistence failed; resumability was not advertised",
            Self::IntegrityKeyRequired => {
                "integrity key is required for durable checkpoint operations"
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionContract {
    pub graph_id: String,
    pub graph_version: String,
    pub input: Value,
    pub budgets: Value,
}

type CheckpointParts = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CheckpointParts> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
    ))
}

fn checkpoint_from_parts(parts: CheckpointParts) -> Option<CheckpointRecord> {
    let (
        run_id,
        graph_id,
        graph_version,
        next_node_cursor,
        state_json,
        state_digest,
        budgets_json,
        counters_json,
        dependency_json,
        dependency_digest,
        terminal_cursor,
        event_cursor,
        checkpoint_id,
        checkpoint_digest,
        created_at,
        consumed_at,
    ) = parts;
    Some(CheckpointRecord {
        checkpoint_id: checkpoint_id?,
        run_id,
        graph_id,
        graph_version,
        next_node_cursor: next_node_cursor?,
        state: serde_json::from_str(&state_json?).ok()?,
        state_digest: state_digest?,
        budgets: serde_json::from_str(&budgets_json?).ok()?,
        budget_counters: serde_json::from_str(&counters_json?).ok()?,
        dependency_summary: serde_json::from_str(&dependency_json?).ok()?,
        dependency_digest: dependency_digest?,
        terminal_cursor: u64::try_from(terminal_cursor?).ok()?,
        event_cursor: u64::try_from(event_cursor?).ok()?,
        checkpoint_digest: checkpoint_digest?,
        created_at: created_at?,
        consumed_at,
    })
}

fn checkpoint_digest(record: &CheckpointRecord, key: &[u8]) -> String {
    hmac_sha256(
        &serde_json::json!({
            "checkpoint_id": record.checkpoint_id,
            "run_id": record.run_id,
            "graph_id": record.graph_id,
            "graph_version": record.graph_version,
            "next_node_cursor": record.next_node_cursor,
            "state": record.state,
            "state_digest": record.state_digest,
            "budgets": record.budgets,
            "budget_counters": record.budget_counters,
            "dependency_summary": record.dependency_summary,
            "dependency_digest": record.dependency_digest,
            "terminal_cursor": record.terminal_cursor,
            "event_cursor": record.event_cursor,
            "created_at": record.created_at,
        }),
        key,
    )
}

fn validate_checkpoint_record(
    record: &CheckpointRecord,
    key: &[u8],
) -> Result<(), CheckpointError> {
    if record.checkpoint_id != format!("checkpoint-{}-{}", record.run_id, record.next_node_cursor)
        || record.state_digest != digest(&record.state)
        || record.dependency_digest != digest(&record.dependency_summary)
        || record.checkpoint_digest != checkpoint_digest(record, key)
    {
        return Err(CheckpointError::Integrity);
    }
    Ok(())
}

type ApprovalParts = (
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
);

fn approval_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApprovalParts> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
    ))
}

fn approval_from_parts(parts: ApprovalParts) -> Option<ApprovalRecord> {
    let (
        approval_id,
        checkpoint_id,
        run_id,
        graph_id,
        graph_version,
        checkpoint_digest,
        audience,
        prompt_digest,
        allowed_decisions,
        approval_digest,
        status,
        decision,
        decided_by,
        decided_at,
        expires_at,
        created_at,
        _legacy,
    ) = parts;
    let allowed_decisions = serde_json::from_str(&allowed_decisions?).ok()?;
    Some(ApprovalRecord {
        approval_id,
        checkpoint_id: checkpoint_id?,
        run_id,
        graph_id: graph_id?,
        graph_version: graph_version?,
        checkpoint_digest: checkpoint_digest?,
        audience,
        prompt_digest,
        allowed_decisions,
        approval_digest: approval_digest?,
        status,
        decision,
        decided_by,
        decided_at,
        expires_at,
        created_at,
    })
}

const APPROVAL_COLUMNS: &str = "approval_id, checkpoint_id, run_id, graph_id,
    graph_version, checkpoint_digest, audience, prompt_digest,
    allowed_decisions, approval_digest, status, decision, decided_by,
    decided_at, expires_at, created_at, prompt";

fn approval_digest(record: &ApprovalRecord, key: &[u8]) -> String {
    hmac_sha256(
        &serde_json::json!({
            "approval_id": record.approval_id,
            "checkpoint_id": record.checkpoint_id,
            "run_id": record.run_id,
            "graph_id": record.graph_id,
            "graph_version": record.graph_version,
            "checkpoint_digest": record.checkpoint_digest,
            "audience": record.audience,
            "prompt_digest": record.prompt_digest,
            "allowed_decisions": record.allowed_decisions,
            "status": record.status,
            "decision": record.decision,
            "decided_by": record.decided_by,
            "decided_at": record.decided_at,
            "expires_at": record.expires_at,
            "created_at": record.created_at,
        }),
        key,
    )
}

fn parse_approval_parts(parts: ApprovalParts, key: &[u8]) -> Result<ApprovalRecord, ApprovalError> {
    let record = approval_from_parts(parts).ok_or(ApprovalError::Integrity)?;
    if record.approval_digest != approval_digest(&record, key) {
        return Err(ApprovalError::Integrity);
    }
    Ok(record)
}

fn uuid_like() -> String {
    digest(&Value::String(
        Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true),
    ))
    .trim_start_matches("sha256:")
    .to_owned()
}

fn load_checkpoint_from_tx(
    tx: &rusqlite::Transaction<'_>,
    checkpoint_id: &str,
    key: &[u8],
) -> Result<CheckpointRecord, CheckpointError> {
    let row = tx.query_row(
        "SELECT run_id, graph_id, graph_version, next_cursor, state_json,
                state_digest, budgets_json, budget_counters_json,
                dependency_json, dependency_digest, terminal_cursor,
                event_cursor, checkpoint_id, checkpoint_digest, created_at,
                consumed_at
         FROM checkpoints WHERE checkpoint_id = ?1",
        params![checkpoint_id],
        checkpoint_row,
    );
    let parts = match row {
        Ok(parts) => parts,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Err(CheckpointError::NotFound),
        Err(_) => return Err(CheckpointError::Persistence),
    };
    let record = checkpoint_from_parts(parts).ok_or(CheckpointError::Integrity)?;
    validate_checkpoint_record(&record, key)?;
    Ok(record)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDeleteResult {
    Deleted,
    NotFound,
    Referenced,
    RetentionApprovalRequired,
    ReferencedBySubgraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphRetentionError {
    NotFound,
    InvalidState,
    InvalidTransition,
    Referenced,
    ReferencedBySubgraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRetentionRecord {
    pub graph_id: String,
    pub state: String,
    pub reason: String,
    pub actor: String,
    pub review_after: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRetentionReport {
    pub graph_id: String,
    pub state: String,
    pub reason: Option<String>,
    pub actor: Option<String>,
    pub review_after: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version_count: u64,
    pub execution_count: u64,
    pub last_execution_at: Option<String>,
    pub inbound_subgraph_refs: Vec<String>,
    pub deletion_eligible: bool,
    pub state_digest: String,
    pub tombstoned: bool,
}

#[derive(Debug, Clone)]
pub struct OperatorRetentionRequest {
    pub request_digest: String,
    pub action: OperatorAction,
    pub graph_id: String,
    pub expected_state_digest: String,
    pub nonce: String,
    pub operator_uid: u32,
    pub daemon_instance_id: String,
    pub issued_at: String,
    pub expires_at: String,
    pub state: Option<String>,
    pub reason: Option<String>,
    pub review_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorRetentionResult {
    Applied { receipt_id: String },
    Replayed { receipt_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorRetentionError {
    NotFound,
    StaleState,
    NonceReplayed,
    InvalidAction,
    InvalidState,
    InvalidTransition,
    Referenced,
    ReferencedBySubgraph,
    Tombstoned,
    Persistence,
}

impl Clone for PersistentStore {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            integrity_key: self.integrity_key.clone(),
            #[cfg(test)]
            terminal_projection_fault: self.terminal_projection_fault.clone(),
            #[cfg(test)]
            checkpoint_persistence_fault: self.checkpoint_persistence_fault.clone(),
            #[cfg(test)]
            graph_delete_fault: self.graph_delete_fault.clone(),
        }
    }
}

impl PersistentStore {
    /// Open (or create) the SQLite database at `{data_dir}/agent-graph.db`.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        Self::open_with_integrity_key(data_dir, None)
    }

    /// Open (or create) the SQLite database with an explicit integrity key path.
    /// If `integrity_key_path` is None, falls back to the
    /// `AGENT_GRAPH_INTEGRITY_KEY_PATH` environment variable.
    pub fn open_with_integrity_key(
        data_dir: &Path,
        integrity_key_path: Option<&Path>,
    ) -> Result<Self, String> {
        crate::fs_security::validate_data_store(data_dir, integrity_key_path)
            .map_err(|e| format!("filesystem security check failed: {e}"))?;
        std::fs::create_dir_all(data_dir).map_err(|e| format!("failed to create data dir: {e}"))?;
        let db_path = data_dir.join("agent-graph.db");
        let conn =
            Connection::open(&db_path).map_err(|e| format!("failed to open database: {e}"))?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| format!("pragma error: {e}"))?;
        crate::fs_security::validate_data_store(data_dir, integrity_key_path)
            .map_err(|e| format!("filesystem security check failed after WAL init: {e}"))?;

        let store = Self {
            conn: std::sync::Arc::new(Mutex::new(conn)),
            integrity_key: Self::load_integrity_key_from(integrity_key_path),
            #[cfg(test)]
            terminal_projection_fault: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            )),
            #[cfg(test)]
            checkpoint_persistence_fault: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            )),
            #[cfg(test)]
            graph_delete_fault: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        store.migrate()?;
        Ok(store)
    }

    #[allow(dead_code)]
    fn load_integrity_key() -> Option<std::sync::Arc<[u8]>> {
        Self::load_integrity_key_from(None)
    }

    /// Load the integrity key from an explicit path, or fall back to the
    /// `AGENT_GRAPH_INTEGRITY_KEY_PATH` environment variable.
    fn load_integrity_key_from(explicit: Option<&Path>) -> Option<std::sync::Arc<[u8]>> {
        let path = if let Some(p) = explicit {
            std::ffi::OsStr::new(p).to_owned()
        } else {
            std::env::var_os("AGENT_GRAPH_INTEGRITY_KEY_PATH")?
        };
        let key = std::fs::read(path).ok()?;
        (key.len() >= 32).then(|| std::sync::Arc::from(key))
    }

    fn require_integrity_key(&self) -> Result<&[u8], ()> {
        self.integrity_key.as_deref().ok_or(())
    }

    pub fn has_integrity_key(&self) -> bool {
        self.integrity_key.is_some()
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graphs (
                name TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                spec_version TEXT NOT NULL DEFAULT '2',
                topology_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS graph_retention (
                graph_name TEXT PRIMARY KEY,
                state TEXT NOT NULL CHECK(state IN ('active', 'active_phantom_contaminated', 'pinned', 'archived', 'expired_pending_review', 'clear_approved', 'lineage_cleared', 'purged', 'delete_candidate', 'delete_approved')),
                reason TEXT NOT NULL,
                actor TEXT NOT NULL,
                review_after TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS graph_tombstones (
                graph_name TEXT PRIMARY KEY,
                topology_hash TEXT NOT NULL,
                reason TEXT NOT NULL,
                actor TEXT NOT NULL,
                deleted_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS operator_receipts (
                receipt_id TEXT PRIMARY KEY,
                request_digest TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_kind TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                state_digest TEXT NOT NULL,
                operator_uid INTEGER NOT NULL,
                daemon_instance_id TEXT NOT NULL,
                nonce TEXT NOT NULL UNIQUE,
                issued_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                consumed_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_operator_receipts_nonce ON operator_receipts(nonce);

            CREATE TABLE IF NOT EXISTS graph_versions (
                graph_name TEXT NOT NULL,
                topology_hash TEXT NOT NULL,
                spec_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (graph_name, topology_hash)
            );

            CREATE TABLE IF NOT EXISTS executions (
                run_id TEXT PRIMARY KEY,
                graph_name TEXT NOT NULL,
                graph_hash TEXT NOT NULL,
                thread_id TEXT,
                status TEXT NOT NULL,
                input_json TEXT,
                budgets_json TEXT,
                final_state_json TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                total_nodes INTEGER DEFAULT 0,
                failed_attempts INTEGER DEFAULT 0,
                idempotency_key TEXT UNIQUE,
                FOREIGN KEY (graph_name) REFERENCES graphs(name)
            );

            CREATE TABLE IF NOT EXISTS checkpoints (
                run_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                attempt INTEGER NOT NULL,
                input_json TEXT,
                output_json TEXT,
                status TEXT NOT NULL,
                error TEXT,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                checkpoint_id TEXT,
                graph_id TEXT,
                graph_version TEXT,
                next_cursor TEXT,
                state_json TEXT,
                state_digest TEXT,
                budgets_json TEXT,
                budget_counters_json TEXT,
                dependency_json TEXT,
                dependency_digest TEXT,
                terminal_cursor INTEGER,
                event_cursor INTEGER,
                checkpoint_digest TEXT,
                created_at TEXT,
                consumed_at TEXT,
                PRIMARY KEY (run_id, node_id, attempt),
                FOREIGN KEY (run_id) REFERENCES executions(run_id)
            );

            CREATE TABLE IF NOT EXISTS events (
                run_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_json TEXT NOT NULL,
                emitted_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (run_id, seq),
                FOREIGN KEY (run_id) REFERENCES executions(run_id)
            );

            CREATE TABLE IF NOT EXISTS terminal_receipts (
                run_id TEXT PRIMARY KEY,
                receipt_json TEXT NOT NULL,
                bundle_json TEXT NOT NULL,
                receipt_digest TEXT NOT NULL,
                persisted_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (run_id) REFERENCES executions(run_id)
            );

            CREATE TABLE IF NOT EXISTS template_candidates (
                template_id TEXT PRIMARY KEY, spec_digest TEXT NOT NULL,
                graph_id TEXT NOT NULL, graph_version TEXT NOT NULL,
                source_ref TEXT NOT NULL, state TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS template_outcome_links (
                template_id TEXT NOT NULL, run_id TEXT NOT NULL,
                terminal_receipt_id TEXT NOT NULL, receipt_digest TEXT NOT NULL,
                disposition TEXT NOT NULL, evidence_digest TEXT NOT NULL,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (template_id, run_id),
                FOREIGN KEY (template_id) REFERENCES template_candidates(template_id),
                FOREIGN KEY (run_id) REFERENCES executions(run_id),
                FOREIGN KEY (run_id) REFERENCES terminal_receipts(run_id)
            );
            CREATE TABLE IF NOT EXISTS template_promotion_decisions (
                template_id TEXT NOT NULL, from_state TEXT NOT NULL,
                to_state TEXT NOT NULL, evidence_set_digest TEXT NOT NULL,
                operator_receipt_id TEXT NOT NULL, decision_digest TEXT NOT NULL,
                decided_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (template_id) REFERENCES template_candidates(template_id)
            );

            CREATE TABLE IF NOT EXISTS approval_requests (
                approval_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                checkpoint_id TEXT,
                graph_id TEXT,
                graph_version TEXT,
                checkpoint_digest TEXT,
                audience TEXT NOT NULL,
                prompt TEXT NOT NULL,
                prompt_digest TEXT,
                allowed_decisions TEXT NOT NULL,
                approval_digest TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT,
                decided_by TEXT,
                decided_at TEXT,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (run_id) REFERENCES executions(run_id)
            );

            CREATE TABLE IF NOT EXISTS idempotency_keys (
                key TEXT PRIMARY KEY,
                request_digest TEXT NOT NULL,
                result_json TEXT NOT NULL,
                valid INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS source_witnesses (
                witness_id TEXT PRIMARY KEY,
                locator TEXT NOT NULL,
                content TEXT NOT NULL,
                media_type TEXT NOT NULL,
                authority_class TEXT NOT NULL,
                retrieved_at TEXT NOT NULL,
                digest TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .map_err(|e| format!("migration error: {e}"))?;
        let has_request_digest = conn
            .prepare("PRAGMA table_info(idempotency_keys)")
            .map_err(|e| format!("idempotency schema inspection error: {e}"))?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| format!("idempotency schema inspection error: {e}"))?
            .filter_map(Result::ok)
            .any(|column| column == "request_digest");
        if !has_request_digest {
            conn.execute(
                "ALTER TABLE idempotency_keys ADD COLUMN request_digest TEXT",
                [],
            )
            .map_err(|e| format!("idempotency migration error: {e}"))?;
        }
        let has_idempotency_valid = conn
            .prepare("PRAGMA table_info(idempotency_keys)")
            .map_err(|e| format!("idempotency schema inspection error: {e}"))?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| format!("idempotency schema inspection error: {e}"))?
            .filter_map(Result::ok)
            .any(|column| column == "valid");
        if !has_idempotency_valid {
            conn.execute(
                "ALTER TABLE idempotency_keys ADD COLUMN valid INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .map_err(|e| format!("idempotency migration error: {e}"))?;
        }
        conn.execute(
            "UPDATE idempotency_keys SET valid = 0 WHERE request_digest IS NULL",
            [],
        )
        .map_err(|e| format!("idempotency quarantine error: {e}"))?;
        for (table, column, definition) in [
            ("executions", "budgets_json", "TEXT"),
            ("checkpoints", "checkpoint_id", "TEXT"),
            ("checkpoints", "graph_id", "TEXT"),
            ("checkpoints", "graph_version", "TEXT"),
            ("checkpoints", "next_cursor", "TEXT"),
            ("checkpoints", "state_json", "TEXT"),
            ("checkpoints", "state_digest", "TEXT"),
            ("checkpoints", "budgets_json", "TEXT"),
            ("checkpoints", "budget_counters_json", "TEXT"),
            ("checkpoints", "dependency_json", "TEXT"),
            ("checkpoints", "dependency_digest", "TEXT"),
            ("checkpoints", "terminal_cursor", "INTEGER"),
            ("checkpoints", "event_cursor", "INTEGER"),
            ("checkpoints", "checkpoint_digest", "TEXT"),
            ("checkpoints", "created_at", "TEXT"),
            ("checkpoints", "consumed_at", "TEXT"),
            ("approval_requests", "checkpoint_id", "TEXT"),
            ("approval_requests", "graph_id", "TEXT"),
            ("approval_requests", "graph_version", "TEXT"),
            ("approval_requests", "checkpoint_digest", "TEXT"),
            ("approval_requests", "prompt_digest", "TEXT"),
            ("approval_requests", "approval_digest", "TEXT"),
        ] {
            let has_column = conn
                .prepare(&format!("PRAGMA table_info({table})"))
                .map_err(|e| format!("{table} schema inspection error: {e}"))?
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| format!("{table} schema inspection error: {e}"))?
                .filter_map(Result::ok)
                .any(|name| name == column);
            if !has_column {
                conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                    [],
                )
                .map_err(|e| format!("{table} migration error: {e}"))?;
            }
        }
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS checkpoints_checkpoint_id_idx
             ON checkpoints(checkpoint_id) WHERE checkpoint_id IS NOT NULL",
            [],
        )
        .map_err(|e| format!("checkpoint index migration error: {e}"))?;
        Ok(())
    }

    // ── Local source witnesses ───────────────────────────────────────

    pub fn capture_witness(&self, capture: WitnessCapture) -> Result<WitnessRecord, WitnessError> {
        let key = self.require_integrity_key().map_err(|_| {
            WitnessError::new(
                "INTEGRITY_KEY_REQUIRED",
                "an external integrity key is required for durable witness capture",
            )
        })?;
        let expected = validate_witness_capture_with_key(capture, Some(key))
            .map_err(|error| WitnessError::new(error.code, error.message))?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| WitnessError::new("WITNESS_STORE_ERROR", "witness SQLite lock failed"))?;
        conn.execute(
            "INSERT OR IGNORE INTO source_witnesses
             (witness_id, locator, content, media_type, authority_class, retrieved_at, digest)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                expected.witness_id,
                expected.locator,
                expected.content,
                expected.media_type,
                expected.authority_class,
                expected.retrieved_at,
                expected.digest
            ],
        )
        .map_err(|_| WitnessError::new("WITNESS_STORE_ERROR", "witness SQLite write failed"))?;
        drop(conn);
        self.get_witness(&expected.witness_id)?.ok_or_else(|| {
            WitnessError::new(
                "WITNESS_STORE_ERROR",
                "witness SQLite write did not produce a row",
            )
        })
    }

    pub fn get_witness(&self, witness_id: &str) -> Result<Option<WitnessRecord>, WitnessError> {
        let key = self.require_integrity_key().map_err(|_| {
            WitnessError::new(
                "INTEGRITY_KEY_REQUIRED",
                "an external integrity key is required for durable witness reads",
            )
        })?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| WitnessError::new("WITNESS_STORE_ERROR", "witness SQLite lock failed"))?;
        let row = conn.query_row(
            "SELECT witness_id, locator, content, media_type, authority_class, retrieved_at, digest
             FROM source_witnesses WHERE witness_id = ?1",
            params![witness_id],
            |row| {
                Ok(WitnessRecord {
                    witness_id: row.get(0)?,
                    locator: row.get(1)?,
                    content: row.get(2)?,
                    media_type: row.get(3)?,
                    authority_class: row.get(4)?,
                    retrieved_at: row.get(5)?,
                    digest: row.get(6)?,
                })
            },
        );
        match row {
            Ok(record) => {
                verify_witness_record_with_key(&record, Some(key))?;
                Ok(Some(record))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(rusqlite::Error::FromSqlConversionFailure(..))
            | Err(rusqlite::Error::InvalidColumnType(..))
            | Err(rusqlite::Error::InvalidColumnName(_)) => Err(WitnessError::new(
                "WITNESS_INTEGRITY_FAILURE",
                "stored witness integrity validation failed",
            )),
            Err(_) => Err(WitnessError::new(
                "WITNESS_STORE_ERROR",
                "witness SQLite read failed",
            )),
        }
    }

    // ── Graphs ──────────────────────────────────────────────────────────

    pub fn save_graph(
        &self,
        name: &str,
        spec_json: &str,
        topology_hash: &str,
        overwrite: bool,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let tombstoned: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_tombstones WHERE graph_name = ?1)",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| format!("graph tombstone check error: {e}"))?;
        if tombstoned {
            return Err(format!(
                "graph '{name}' has a retention tombstone; choose a new graph ID"
            ));
        }
        let current: Option<String> = conn
            .query_row(
                "SELECT topology_hash FROM graphs WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .ok();
        if let Some(current) = current
            .as_deref()
            .filter(|current| *current != topology_hash)
        {
            let referenced: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM executions WHERE graph_name = ?1 AND graph_hash = ?2)",
                    params![name, current],
                    |row| row.get(0),
                )
                .map_err(|e| format!("graph version reference check error: {e}"))?;
            if referenced && !overwrite {
                return Err(format!("graph '{name}' current version {current} is referenced by a durable execution; explicit overwrite is required"));
            }
        }
        conn.execute(
            "INSERT OR IGNORE INTO graph_versions (graph_name, topology_hash, spec_json) VALUES (?1, ?2, ?3)",
            params![name, topology_hash, spec_json],
        )
        .map_err(|e| format!("save graph version error: {e}"))?;
        conn.execute(
            "INSERT INTO graphs (name, spec_json, topology_hash)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET
                spec_json = excluded.spec_json,
                topology_hash = excluded.topology_hash,
                updated_at = datetime('now')",
            params![name, spec_json, topology_hash],
        )
        .map_err(|e| format!("save_graph error: {e}"))?;
        conn.execute(
            "INSERT INTO graph_retention (graph_name, state, reason, actor)
             VALUES (?1, 'active', 'created', 'system')
             ON CONFLICT(graph_name) DO NOTHING",
            params![name],
        )
        .map_err(|e| format!("save graph retention state error: {e}"))?;
        Ok(())
    }

    pub fn load_graph(&self, name: &str) -> Result<Option<(String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT spec_json, topology_hash FROM graphs WHERE name = ?1")
            .map_err(|e| format!("load_graph error: {e}"))?;
        let result = stmt
            .query_row(params![name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .ok();
        Ok(result)
    }

    pub fn list_graphs(&self) -> Result<Vec<(String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT g.name, g.topology_hash, g.created_at FROM graphs AS g
                      WHERE NOT EXISTS (SELECT 1 FROM graph_tombstones AS t WHERE t.graph_name = g.name)
                      ORDER BY g.name")
            .map_err(|e| format!("list_graphs error: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("list_graphs error: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn graph_is_tombstoned(&self, graph_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM graph_tombstones WHERE graph_name = ?1)",
            params![graph_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("graph tombstone query error: {e}"))
    }

    fn inbound_subgraph_refs(conn: &Connection, name: &str) -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT name, spec_json FROM graphs WHERE name != ?1")
            .map_err(|e| format!("subgraph reference query error: {e}"))?;
        let rows = stmt
            .query_map(params![name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("subgraph reference query error: {e}"))?;
        let mut refs = Vec::new();
        for row in rows {
            let (graph_name, spec_json) =
                row.map_err(|e| format!("subgraph reference row error: {e}"))?;
            let Ok(spec) = serde_json::from_str::<Value>(&spec_json) else {
                continue;
            };
            let references_target =
                spec.get("nodes")
                    .and_then(Value::as_array)
                    .is_some_and(|nodes| {
                        nodes.iter().any(|node| {
                            node.get("type").and_then(Value::as_str) == Some("subgraph")
                                && node
                                    .get("config")
                                    .and_then(|config| config.get("graph_name"))
                                    .and_then(Value::as_str)
                                    == Some(name)
                        })
                    });
            if references_target {
                refs.push(graph_name);
            }
        }
        refs.sort();
        Ok(refs)
    }

    pub fn graph_retention_review(
        &self,
        graph_id: Option<&str>,
        state: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GraphRetentionReport>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT g.name, g.created_at, g.updated_at,
                        r.state, r.reason, r.actor, r.review_after,
                        COUNT(DISTINCT gv.topology_hash), COUNT(DISTINCT e.run_id),
                        MAX(COALESCE(e.finished_at, e.started_at))
                 FROM graphs AS g
                 LEFT JOIN graph_retention AS r ON r.graph_name = g.name
                 LEFT JOIN graph_versions AS gv ON gv.graph_name = g.name
                 LEFT JOIN executions AS e ON e.graph_name = g.name
                 WHERE (?1 IS NULL OR g.name = ?1)
                   AND (?2 IS NULL OR COALESCE(r.state, 'unclassified') = ?2)
                 GROUP BY g.name
                 ORDER BY g.updated_at ASC, g.name ASC
                 LIMIT ?3",
            )
            .map_err(|e| format!("graph retention review query error: {e}"))?;
        let rows = stmt
            .query_map(params![graph_id, state, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(|e| format!("graph retention review query error: {e}"))?;
        let mut reports = Vec::new();
        for row in rows {
            let (
                graph_id,
                created_at,
                updated_at,
                state,
                reason,
                actor,
                review_after,
                versions,
                executions,
                last_execution_at,
            ) = row.map_err(|e| format!("graph retention review row error: {e}"))?;
            let inbound_subgraph_refs = Self::inbound_subgraph_refs(&conn, &graph_id)?;
            let tombstoned: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM graph_tombstones WHERE graph_name = ?1)",
                    params![&graph_id],
                    |row| row.get(0),
                )
                .map_err(|e| format!("graph retention tombstone query error: {e}"))?;
            let state = state.unwrap_or_else(|| "unclassified".into());
            let state_digest = digest(&serde_json::json!({
                "graph_id": graph_id,
                "state": state,
                "reason": reason,
                "actor": actor,
                "review_after": review_after,
                "topology_hashes": versions,
                "execution_count": executions,
                "last_execution_at": last_execution_at,
                "inbound_subgraph_refs": inbound_subgraph_refs,
                "tombstoned": tombstoned,
            }));
            reports.push(GraphRetentionReport {
                graph_id,
                state,
                reason,
                actor,
                review_after,
                created_at,
                updated_at,
                version_count: versions as u64,
                execution_count: executions as u64,
                last_execution_at,
                deletion_eligible: executions == 0 && inbound_subgraph_refs.is_empty(),
                inbound_subgraph_refs,
                state_digest,
                tombstoned,
            });
        }
        Ok(reports)
    }

    /// Apply one OS-authenticated retention mutation and its receipt atomically.
    /// The caller must have already validated peer credentials and the time window.
    pub fn apply_operator_retention(
        &self,
        request: &OperatorRetentionRequest,
    ) -> Result<OperatorRetentionResult, OperatorRetentionError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| OperatorRetentionError::Persistence)?;

        let prior: Option<(String, String)> = conn
            .query_row(
                "SELECT request_digest, receipt_id FROM operator_receipts WHERE nonce = ?1",
                params![request.nonce],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| OperatorRetentionError::Persistence)?;
        if let Some((digest, receipt_id)) = prior {
            if digest == request.request_digest {
                return Ok(OperatorRetentionResult::Replayed { receipt_id });
            }
            return Err(OperatorRetentionError::NonceReplayed);
        }

        let snapshot: Option<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            i64,
            Option<String>,
        )> = conn
            .query_row(
                "SELECT COALESCE(r.state, 'unclassified'), r.reason, r.actor, r.review_after,
                        COUNT(DISTINCT gv.topology_hash), COUNT(DISTINCT e.run_id),
                        MAX(COALESCE(e.finished_at, e.started_at))
                 FROM graphs AS g
                 LEFT JOIN graph_retention AS r ON r.graph_name = g.name
                 LEFT JOIN graph_versions AS gv ON gv.graph_name = g.name
                 LEFT JOIN executions AS e ON e.graph_name = g.name
                 WHERE g.name = ?1
                 GROUP BY g.name",
                params![request.graph_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| OperatorRetentionError::Persistence)?;
        let Some((
            current_state,
            current_reason,
            current_actor,
            current_review_after,
            versions,
            executions,
            last_execution_at,
        )) = snapshot
        else {
            return Err(OperatorRetentionError::NotFound);
        };
        let inbound = Self::inbound_subgraph_refs(&conn, &request.graph_id)
            .map_err(|_| OperatorRetentionError::Persistence)?;
        let tombstoned: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_tombstones WHERE graph_name = ?1)",
                params![request.graph_id],
                |row| row.get(0),
            )
            .map_err(|_| OperatorRetentionError::Persistence)?;
        if tombstoned {
            return Err(OperatorRetentionError::Tombstoned);
        }
        let current_digest = digest(&serde_json::json!({
            "graph_id": request.graph_id,
            "state": current_state,
            "reason": current_reason,
            "actor": current_actor,
            "review_after": current_review_after,
            "topology_hashes": versions,
            "execution_count": executions,
            "last_execution_at": last_execution_at.map_or(serde_json::Value::Null, serde_json::Value::String),
            "inbound_subgraph_refs": inbound,
            "tombstoned": tombstoned,
        }));
        if current_digest != request.expected_state_digest {
            return Err(OperatorRetentionError::StaleState);
        }
        if executions > 0 {
            return Err(OperatorRetentionError::Referenced);
        }
        if !inbound.is_empty() {
            return Err(OperatorRetentionError::ReferencedBySubgraph);
        }

        let action = serde_json::to_string(&request.action)
            .map_err(|_| OperatorRetentionError::InvalidAction)?;
        let actor = format!("uid:{}", request.operator_uid);
        let tx = conn
            .transaction()
            .map_err(|_| OperatorRetentionError::Persistence)?;
        match request.action {
            OperatorAction::SetGraphRetention => {
                let state = request
                    .state
                    .as_deref()
                    .ok_or(OperatorRetentionError::InvalidState)?;
                if !matches!(state, "active" | "active_phantom_contaminated" | "pinned" | "archived" | "expired_pending_review" | "clear_approved" | "lineage_cleared" | "purged" | "delete_candidate") {
                    return Err(OperatorRetentionError::InvalidState);
                }
                tx.execute(
                    "INSERT INTO graph_retention (graph_name, state, reason, actor, review_after)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(graph_name) DO UPDATE SET state=excluded.state,
                       reason=excluded.reason, actor=excluded.actor,
                       review_after=excluded.review_after, updated_at=datetime('now')",
                    params![
                        request.graph_id,
                        state,
                        request.reason.as_deref().unwrap_or("operator decision"),
                        actor,
                        request.review_after
                    ],
                )
                .map_err(|_| OperatorRetentionError::Persistence)?;
            }
            OperatorAction::ApproveGraphDeletion => {
                if current_state != "delete_candidate" {
                    return Err(OperatorRetentionError::InvalidTransition);
                }
                tx.execute(
                    "UPDATE graph_retention SET state='delete_approved', updated_at=datetime('now'), actor=?2, reason=?3 WHERE graph_name=?1",
                    params![request.graph_id, actor, request.reason.as_deref().unwrap_or("operator approval")],
                )
                .map_err(|_| OperatorRetentionError::Persistence)?;
            }
            OperatorAction::DeleteGraph => {
                if current_state != "delete_approved" {
                    return Err(OperatorRetentionError::InvalidTransition);
                }
                let topology_hash: String = tx
                    .query_row(
                        "SELECT topology_hash FROM graphs WHERE name=?1",
                        params![request.graph_id],
                        |row| row.get(0),
                    )
                    .map_err(|_| OperatorRetentionError::NotFound)?;
                tx.execute(
                    "INSERT INTO graph_tombstones (graph_name, topology_hash, reason, actor)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        request.graph_id,
                        topology_hash,
                        request.reason.as_deref().unwrap_or("operator deletion"),
                        actor
                    ],
                )
                .map_err(|_| OperatorRetentionError::Persistence)?;
            }
            OperatorAction::ClearExecutionLineage => {
                return Err(OperatorRetentionError::InvalidState);
                // TODO: implement per spec §4.1 — transactional DELETE of execution data
            }
            OperatorAction::PurgeGraph => {
                return Err(OperatorRetentionError::InvalidTransition);
                // TODO: implement per spec §4.2 — tombstone + graph deletion
            }
            OperatorAction::PromoteTemplate => {
                return Err(OperatorRetentionError::InvalidAction);
                // TODO: wire template promotion
            }
            OperatorAction::Migrate => {
                return Err(OperatorRetentionError::InvalidAction);
                // TODO: wire graph state migration
            }
            OperatorAction::Install => {
                // TODO: wire daemon installation — currently no-op for idempotency
            }
            _ => return Err(OperatorRetentionError::InvalidAction),
        }

        let receipt_id = format!(
            "operator-{}",
            request.request_digest.trim_start_matches("sha256:")
        );
        tx.execute(
            "INSERT INTO operator_receipts
             (receipt_id, request_digest, action, resource_kind, resource_id, state_digest,
              operator_uid, daemon_instance_id, nonce, issued_at, expires_at)
             VALUES (?1, ?2, ?3, 'graph', ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                receipt_id,
                request.request_digest,
                action,
                request.graph_id,
                request.expected_state_digest,
                request.operator_uid,
                request.daemon_instance_id,
                request.nonce,
                request.issued_at,
                request.expires_at
            ],
        )
        .map_err(|_| OperatorRetentionError::Persistence)?;
        tx.commit()
            .map_err(|_| OperatorRetentionError::Persistence)?;
        Ok(OperatorRetentionResult::Applied { receipt_id })
    }

    pub fn set_graph_retention(
        &self,
        graph_id: &str,
        state: &str,
        reason: &str,
        actor: &str,
        review_after: Option<&str>,
    ) -> Result<GraphRetentionRecord, GraphRetentionError> {
        if !matches!(
            state,
            "active" | "pinned" | "archived" | "delete_candidate" | "delete_approved"
        ) {
            return Err(GraphRetentionError::InvalidState);
        }
        if reason.trim().is_empty() || actor.trim().is_empty() {
            return Err(GraphRetentionError::InvalidState);
        }
        let reports = self
            .graph_retention_review(Some(graph_id), None, 1)
            .map_err(|_| GraphRetentionError::NotFound)?;
        let Some(report) = reports.into_iter().next() else {
            return Err(GraphRetentionError::NotFound);
        };
        if matches!(state, "delete_candidate" | "delete_approved") {
            if report.execution_count > 0 {
                return Err(GraphRetentionError::Referenced);
            }
            if !report.inbound_subgraph_refs.is_empty() {
                return Err(GraphRetentionError::ReferencedBySubgraph);
            }
        }
        if state == "delete_approved" && report.state != "delete_candidate" {
            return Err(GraphRetentionError::InvalidTransition);
        }
        let conn = self
            .conn
            .lock()
            .map_err(|_| GraphRetentionError::NotFound)?;
        conn.execute(
            "INSERT INTO graph_retention (graph_name, state, reason, actor, review_after)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(graph_name) DO UPDATE SET
                 state = excluded.state,
                 reason = excluded.reason,
                 actor = excluded.actor,
                 review_after = excluded.review_after,
                 updated_at = datetime('now')",
            params![graph_id, state, reason, actor, review_after],
        )
        .map_err(|_| GraphRetentionError::NotFound)?;
        let updated_at = conn
            .query_row(
                "SELECT updated_at FROM graph_retention WHERE graph_name = ?1",
                params![graph_id],
                |row| row.get(0),
            )
            .map_err(|_| GraphRetentionError::NotFound)?;
        Ok(GraphRetentionRecord {
            graph_id: graph_id.into(),
            state: state.into(),
            reason: reason.into(),
            actor: actor.into(),
            review_after: review_after.map(str::to_owned),
            updated_at,
        })
    }

    pub fn graph_execution_allowed(&self, graph_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let tombstoned: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_tombstones WHERE graph_name = ?1)",
                params![graph_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("graph tombstone execution check error: {e}"))?;
        if tombstoned {
            return Ok(false);
        }
        let state: Option<String> = conn
            .query_row(
                "SELECT state FROM graph_retention WHERE graph_name = ?1",
                params![graph_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("graph retention execution check error: {e}"))?;
        Ok(matches!(
            state.as_deref(),
            None | Some("active") | Some("pinned")
        ))
    }

    pub fn delete_graph(&self, name: &str) -> Result<GraphDeleteResult, String> {
        let mut conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let graph: Option<String> = conn
            .query_row(
                "SELECT topology_hash FROM graphs WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("delete_graph graph lookup error: {e}"))?;
        let Some(topology_hash) = graph else {
            return Ok(GraphDeleteResult::NotFound);
        };
        if !Self::inbound_subgraph_refs(&conn, name)?.is_empty() {
            return Ok(GraphDeleteResult::ReferencedBySubgraph);
        }
        let retention: Option<(String, String, String)> = conn
            .query_row(
                "SELECT state, reason, actor FROM graph_retention WHERE graph_name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| format!("delete_graph retention lookup error: {e}"))?;
        if !matches!(
            retention.as_ref().map(|record| record.0.as_str()),
            Some("delete_approved")
        ) {
            return Ok(GraphDeleteResult::RetentionApprovalRequired);
        }
        let (reason, actor) = retention
            .map(|(_, reason, actor)| (reason, actor))
            .expect("approved retention state exists");
        let tx = conn
            .transaction()
            .map_err(|e| format!("delete_graph transaction begin error: {e}"))?;
        let referenced: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM executions WHERE graph_name = ?1)",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| format!("delete_graph reference check error: {e}"))?;
        if referenced {
            return Ok(GraphDeleteResult::Referenced);
        }
        tx.execute(
            "INSERT INTO graph_tombstones (graph_name, topology_hash, reason, actor)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, topology_hash, reason, actor],
        )
        .map_err(|e| format!("delete_graph tombstone error: {e}"))?;
        let affected: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM graphs WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| format!("delete_graph graph presence error: {e}"))?;
        if affected == 0 {
            return Ok(GraphDeleteResult::NotFound);
        }
        #[cfg(test)]
        if self
            .graph_delete_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("injected graph deletion failure after graph row removal".into());
        }
        tx.commit()
            .map_err(|e| format!("delete_graph transaction commit error: {e}"))?;
        Ok(GraphDeleteResult::Deleted)
    }

    #[cfg(test)]
    pub(crate) fn fail_graph_delete_after_graph_row(&self) {
        self.graph_delete_fault
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn fail_terminal_projection_after_events(&self) {
        self.terminal_projection_fault
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn load_graph_version(
        &self,
        name: &str,
        topology_hash: &str,
    ) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let result = conn
            .query_row(
                "SELECT spec_json FROM graph_versions WHERE graph_name = ?1 AND topology_hash = ?2",
                params![name, topology_hash],
                |row| row.get::<_, String>(0),
            )
            .ok();
        Ok(result)
    }

    pub fn list_graph_versions(&self, name: &str) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT topology_hash FROM graph_versions WHERE graph_name = ?1 ORDER BY created_at, topology_hash")
            .map_err(|e| format!("list graph versions error: {e}"))?;
        let versions = stmt
            .query_map(params![name], |row| row.get::<_, String>(0))
            .map_err(|e| format!("list graph versions error: {e}"))?
            .filter_map(Result::ok)
            .collect();
        Ok(versions)
    }

    // ── Executions ──────────────────────────────────────────────────────

    pub fn save_execution(
        &self,
        run_id: &str,
        graph_name: &str,
        graph_hash: &str,
        status: &str,
        input_json: &str,
    ) -> Result<(), String> {
        self.save_execution_with_budgets(run_id, graph_name, graph_hash, status, input_json, None)
    }

    pub fn save_execution_with_budgets(
        &self,
        run_id: &str,
        graph_name: &str,
        graph_hash: &str,
        status: &str,
        input_json: &str,
        budgets_json: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "INSERT INTO executions (run_id, graph_name, graph_hash, status, input_json, budgets_json, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(run_id) DO UPDATE SET
                status = excluded.status,
                budgets_json = COALESCE(excluded.budgets_json, executions.budgets_json),
                finished_at = CASE WHEN excluded.status IN ('completed','failed','cancelled') THEN datetime('now') ELSE finished_at END",
            params![run_id, graph_name, graph_hash, status, input_json, budgets_json],
        )
        .map_err(|e| format!("save_execution error: {e}"))?;
        Ok(())
    }

    pub fn update_execution_status(
        &self,
        run_id: &str,
        status: &str,
        final_state_json: Option<&str>,
        total_nodes: Option<usize>,
        failed_attempts: Option<usize>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let changed = conn.execute(
            "UPDATE executions SET
                status = ?2,
                final_state_json = COALESCE(?3, final_state_json),
                total_nodes = COALESCE(?4, total_nodes),
                failed_attempts = COALESCE(?5, failed_attempts),
                finished_at = CASE WHEN ?2 IN ('completed','failed','cancelled') THEN datetime('now') ELSE finished_at END
             WHERE run_id = ?1",
            params![
                run_id,
                status,
                final_state_json,
                total_nodes.map(|v| v as i64),
                failed_attempts.map(|v| v as i64)
            ],
        ).map_err(|e| format!("update_execution error: {e}"))?;
        if changed != 1 {
            return Err(format!(
                "update_execution error: run '{run_id}' was not found"
            ));
        }
        Ok(())
    }

    pub fn persist_terminal_projection(
        &self,
        run_id: &str,
        status: &str,
        final_state_json: &str,
        total_nodes: usize,
        events: &[(u64, String, String)],
        receipt_json: &str,
        bundle_json: &str,
    ) -> Result<String, String> {
        let key = self
            .require_integrity_key()
            .map_err(|_| "INTEGRITY_KEY_REQUIRED".to_owned())?;
        let receipt: Value = serde_json::from_str(receipt_json)
            .map_err(|e| format!("terminal receipt JSON error: {e}"))?;
        let receipt_digest = hmac_sha256(&receipt, key);
        let mut conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let tx = conn
            .transaction()
            .map_err(|e| format!("terminal transaction begin error: {e}"))?;
        let changed = tx.execute(
            "UPDATE executions SET status = ?2, final_state_json = ?3, total_nodes = ?4,
             finished_at = CASE WHEN ?2 IN ('completed','failed','cancelled') THEN datetime('now') ELSE finished_at END
             WHERE run_id = ?1",
            params![run_id, status, final_state_json, total_nodes as i64],
        ).map_err(|e| format!("terminal execution update error: {e}"))?;
        if changed != 1 {
            return Err(format!("terminal projection run '{run_id}' was not found"));
        }
        tx.execute("DELETE FROM events WHERE run_id = ?1", params![run_id])
            .map_err(|e| format!("terminal event reset error: {e}"))?;
        for (seq, event_type, event_json) in events {
            tx.execute(
                "INSERT INTO events (run_id, seq, event_type, event_json) VALUES (?1, ?2, ?3, ?4)",
                params![run_id, *seq, event_type, event_json],
            )
            .map_err(|e| format!("terminal event insert error: {e}"))?;
        }
        #[cfg(test)]
        if self
            .terminal_projection_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("injected terminal projection failure after events".into());
        }
        tx.execute(
            "INSERT INTO terminal_receipts (run_id, receipt_json, bundle_json, receipt_digest) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(run_id) DO UPDATE SET receipt_json=excluded.receipt_json, bundle_json=excluded.bundle_json, receipt_digest=excluded.receipt_digest, persisted_at=datetime('now')",
            params![run_id, receipt_json, bundle_json, receipt_digest],
        ).map_err(|e| format!("terminal receipt insert error: {e}"))?;
        tx.commit()
            .map_err(|e| format!("terminal transaction commit error: {e}"))?;
        Ok(receipt_digest)
    }

    pub fn load_terminal_receipt(&self, run_id: &str) -> Result<Option<Value>, String> {
        let key = self
            .require_integrity_key()
            .map_err(|_| "INTEGRITY_KEY_REQUIRED".to_owned())?;
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let row: Result<(String, String), _> = conn.query_row(
            "SELECT receipt_json, receipt_digest FROM terminal_receipts WHERE run_id = ?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        match row {
            Ok((receipt_json, receipt_digest)) => {
                let receipt: Value = serde_json::from_str(&receipt_json)
                    .map_err(|e| format!("stored receipt JSON error: {e}"))?;
                if hmac_sha256(&receipt, key) != receipt_digest {
                    return Err("RECEIPT_INTEGRITY_FAILURE".into());
                }
                Ok(Some(
                    serde_json::json!({"receipt":receipt,"receipt_digest":receipt_digest,"storage_class":"sqlite_terminal_receipt","replay_capability":"integrity_only"}),
                ))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("load terminal receipt error: {e}")),
        }
    }

    /// A server restart cannot resume an in-flight graph. Make that interruption
    /// explicit instead of leaving a permanently misleading `running` row.
    pub fn recover_incomplete_executions(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "UPDATE executions
             SET status = 'interrupted_non_resumable', finished_at = datetime('now')
             WHERE status IN ('accepted', 'running')",
            [],
        )
        .map_err(|e| format!("recover executions error: {e}"))?;
        Ok(())
    }

    /// Return the terminal projection retained by SQLite. This is deliberately
    /// not a resumable checkpoint or replay artifact.
    pub fn load_execution(&self, run_id: &str) -> Result<Option<Value>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare(
                "SELECT graph_name, graph_hash, status, final_state_json, started_at, finished_at
                 FROM executions WHERE run_id = ?1",
            )
            .map_err(|e| format!("load execution error: {e}"))?;
        let row = stmt.query_row(params![run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        });
        match row {
            Ok((graph_id, graph_version, status, final_state_json, started_at, finished_at)) => {
                let final_state = final_state_json
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()
                    .map_err(|e| format!("stored final state JSON error: {e}"))?
                    .unwrap_or(Value::Null);
                Ok(Some(serde_json::json!({
                    "run_id": run_id,
                    "graph_id": graph_id,
                    "graph_version": graph_version,
                    "status": status,
                    "success": status == "completed",
                    "final_state": final_state,
                    "started_at": started_at,
                    "finished_at": finished_at,
                    "storage_class": "sqlite_terminal_record",
                    "durable_resume": false,
                    "replay_capability": "integrity_only"
                })))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("load execution error: {e}")),
        }
    }

    pub fn load_execution_contract(
        &self,
        run_id: &str,
    ) -> Result<Option<ExecutionContract>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let result = conn.query_row(
            "SELECT graph_name, graph_hash, input_json, budgets_json
             FROM executions WHERE run_id = ?1",
            params![run_id],
            |row| {
                let input_json: Option<String> = row.get(2)?;
                let budgets_json: Option<String> = row.get(3)?;
                Ok(ExecutionContract {
                    graph_id: row.get(0)?,
                    graph_version: row.get(1)?,
                    input: input_json
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?
                        .unwrap_or(Value::Null),
                    budgets: budgets_json
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?
                        .unwrap_or(Value::Null),
                })
            },
        );
        match result {
            Ok(contract) => Ok(Some(contract)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(format!("load execution contract error: {error}")),
        }
    }

    // ── Checkpoints ─────────────────────────────────────────────────────

    pub fn create_resume_checkpoint(
        &self,
        run_id: &str,
        graph_id: &str,
        graph_version: &str,
        next_node_cursor: &str,
        state: &Value,
        budgets: &Value,
        budget_counters: &Value,
        dependency_summary: &Value,
        terminal_cursor: u64,
        event_cursor: u64,
    ) -> Result<CheckpointRecord, CheckpointError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| CheckpointError::IntegrityKeyRequired)?;
        let state_json = serde_json::to_string(state).map_err(|_| CheckpointError::Persistence)?;
        let budgets_json =
            serde_json::to_string(budgets).map_err(|_| CheckpointError::Persistence)?;
        let counters_json =
            serde_json::to_string(budget_counters).map_err(|_| CheckpointError::Persistence)?;
        let dependency_json =
            serde_json::to_string(dependency_summary).map_err(|_| CheckpointError::Persistence)?;
        let state_digest = digest(state);
        let dependency_digest = digest(dependency_summary);
        let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
        let checkpoint_id = format!("checkpoint-{run_id}-{next_node_cursor}");
        let mut record = CheckpointRecord {
            checkpoint_id,
            run_id: run_id.to_owned(),
            graph_id: graph_id.to_owned(),
            graph_version: graph_version.to_owned(),
            next_node_cursor: next_node_cursor.to_owned(),
            state: state.clone(),
            state_digest,
            budgets: budgets.clone(),
            budget_counters: budget_counters.clone(),
            dependency_summary: dependency_summary.clone(),
            dependency_digest,
            terminal_cursor,
            event_cursor,
            checkpoint_digest: String::new(),
            created_at,
            consumed_at: None,
        };
        record.checkpoint_digest = checkpoint_digest(&record, key);

        let mut conn = self.conn.lock().map_err(|_| CheckpointError::Persistence)?;
        let tx = conn
            .transaction()
            .map_err(|_| CheckpointError::Persistence)?;
        #[cfg(test)]
        if self
            .checkpoint_persistence_fault
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(CheckpointError::Persistence);
        }
        tx.execute(
            "INSERT INTO checkpoints
             (run_id, node_id, attempt, input_json, status, checkpoint_id,
              graph_id, graph_version, next_cursor, state_json, state_digest,
              budgets_json, budget_counters_json, dependency_json,
              dependency_digest, terminal_cursor, event_cursor, checkpoint_digest,
              created_at, consumed_at)
             VALUES (?1, ?2, 0, ?3, 'available', ?4, ?5, ?6, ?7, ?3, ?8,
                     ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL)",
            params![
                record.run_id,
                record.next_node_cursor,
                state_json,
                record.checkpoint_id,
                record.graph_id,
                record.graph_version,
                record.next_node_cursor,
                record.state_digest,
                budgets_json,
                counters_json,
                dependency_json,
                record.dependency_digest,
                record.terminal_cursor as i64,
                record.event_cursor as i64,
                record.checkpoint_digest,
                record.created_at,
            ],
        )
        .map_err(|_| CheckpointError::Persistence)?;
        tx.commit().map_err(|_| CheckpointError::Persistence)?;
        Ok(record)
    }

    pub fn load_resume_checkpoint(
        &self,
        checkpoint_id: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<Option<CheckpointRecord>, CheckpointError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| CheckpointError::IntegrityKeyRequired)?;
        let conn = self.conn.lock().map_err(|_| CheckpointError::Persistence)?;
        let query = if checkpoint_id.is_some() {
            "SELECT run_id, graph_id, graph_version, next_cursor, state_json,
                    state_digest, budgets_json, budget_counters_json,
                    dependency_json, dependency_digest, terminal_cursor,
                    event_cursor, checkpoint_id, checkpoint_digest, created_at,
                    consumed_at
             FROM checkpoints WHERE checkpoint_id = ?1"
        } else {
            "SELECT run_id, graph_id, graph_version, next_cursor, state_json,
                    state_digest, budgets_json, budget_counters_json,
                    dependency_json, dependency_digest, terminal_cursor,
                    event_cursor, checkpoint_id, checkpoint_digest, created_at,
                    consumed_at
             FROM checkpoints WHERE run_id = ?1 AND checkpoint_id IS NOT NULL
             ORDER BY created_at DESC, checkpoint_id DESC LIMIT 1"
        };
        let selector = checkpoint_id.or(run_id).unwrap_or("");
        let row = conn.query_row(query, params![selector], checkpoint_row);
        let parts = match row {
            Ok(parts) => parts,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(_) => return Err(CheckpointError::Persistence),
        };
        let record = checkpoint_from_parts(parts).ok_or(CheckpointError::Integrity)?;
        validate_checkpoint_record(&record, key)?;
        Ok(Some(record))
    }

    pub fn consume_resume_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<CheckpointRecord, CheckpointError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| CheckpointError::IntegrityKeyRequired)?;
        let mut conn = self.conn.lock().map_err(|_| CheckpointError::Persistence)?;
        let tx = conn
            .transaction()
            .map_err(|_| CheckpointError::Persistence)?;
        let row = tx.query_row(
            "SELECT run_id, graph_id, graph_version, next_cursor, state_json,
                    state_digest, budgets_json, budget_counters_json,
                    dependency_json, dependency_digest, terminal_cursor,
                    event_cursor, checkpoint_id, checkpoint_digest, created_at,
                    consumed_at
             FROM checkpoints WHERE checkpoint_id = ?1",
            params![checkpoint_id],
            checkpoint_row,
        );
        let parts = match row {
            Ok(parts) => parts,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Err(CheckpointError::NotFound),
            Err(_) => return Err(CheckpointError::Persistence),
        };
        let record = checkpoint_from_parts(parts).ok_or(CheckpointError::Integrity)?;
        validate_checkpoint_record(&record, key)?;
        if record.consumed_at.is_some() {
            return Err(CheckpointError::Consumed);
        }
        let consumed_at = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
        let changed = tx
            .execute(
                "UPDATE checkpoints SET consumed_at = ?2, status = 'consumed'
                 WHERE checkpoint_id = ?1 AND consumed_at IS NULL",
                params![checkpoint_id, consumed_at],
            )
            .map_err(|_| CheckpointError::Persistence)?;
        if changed != 1 {
            return Err(CheckpointError::Consumed);
        }
        tx.commit().map_err(|_| CheckpointError::Persistence)?;
        let mut consumed = record;
        consumed.consumed_at = Some(consumed_at);
        Ok(consumed)
    }

    // ── Durable checkpoint-bound approvals ───────────────────────────

    pub fn create_checkpoint_approval(
        &self,
        checkpoint_id: &str,
        graph_id: &str,
        graph_version: &str,
        next_node_cursor: &str,
        expected_state: &Value,
        expected_budgets: &Value,
        expected_budget_counters: &Value,
        dependency_summary: &Value,
        audience: &str,
        prompt_digest: &str,
        allowed_decisions: &[String],
        expires_at: &str,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| ApprovalError::IntegrityKeyRequired)?;
        let mut conn = self.conn.lock().map_err(|_| ApprovalError::Persistence)?;
        let tx = conn.transaction().map_err(|_| ApprovalError::Persistence)?;
        let checkpoint =
            load_checkpoint_from_tx(&tx, checkpoint_id, key).map_err(ApprovalError::Checkpoint)?;
        if checkpoint.consumed_at.is_some() {
            return Err(ApprovalError::Checkpoint(CheckpointError::Consumed));
        }
        if checkpoint.graph_id != graph_id
            || checkpoint.graph_version != graph_version
            || checkpoint.next_node_cursor != next_node_cursor
            || checkpoint.state != *expected_state
            || checkpoint.budgets != *expected_budgets
            || checkpoint.budget_counters != *expected_budget_counters
            || checkpoint.dependency_summary != *dependency_summary
            || checkpoint.terminal_cursor != 0
            || checkpoint.event_cursor != 0
        {
            return Err(ApprovalError::Checkpoint(CheckpointError::Integrity));
        }

        let allowed_json =
            serde_json::to_string(allowed_decisions).map_err(|_| ApprovalError::Persistence)?;
        let existing = tx
            .query_row(
                &format!(
                    "SELECT {APPROVAL_COLUMNS} FROM approval_requests
                          WHERE checkpoint_id = ?1 AND audience = ?2 AND status = 'pending'"
                ),
                params![checkpoint_id, audience],
                approval_row,
            )
            .optional()
            .map_err(|_| ApprovalError::Persistence)?;
        if let Some(parts) = existing {
            let current = parse_approval_parts(parts, key)?;
            if current.graph_id == graph_id
                && current.graph_version == graph_version
                && current.checkpoint_digest == checkpoint.checkpoint_digest
                && current.prompt_digest == prompt_digest
                && current.allowed_decisions.as_slice() == allowed_decisions
                && current.expires_at == expires_at
            {
                tx.commit().map_err(|_| ApprovalError::Persistence)?;
                return Ok(current);
            }
            return Err(ApprovalError::Conflict);
        }

        let created_at = Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true);
        let mut record = ApprovalRecord {
            approval_id: format!("approval-{}", uuid_like()),
            checkpoint_id: checkpoint.checkpoint_id.clone(),
            run_id: checkpoint.run_id.clone(),
            graph_id: graph_id.to_owned(),
            graph_version: graph_version.to_owned(),
            checkpoint_digest: checkpoint.checkpoint_digest.clone(),
            audience: audience.to_owned(),
            prompt_digest: prompt_digest.to_owned(),
            allowed_decisions: allowed_decisions.to_vec(),
            approval_digest: String::new(),
            status: "pending".into(),
            decision: None,
            decided_by: None,
            decided_at: None,
            expires_at: expires_at.to_owned(),
            created_at,
        };
        record.approval_digest = approval_digest(&record, key);
        tx.execute(
            "INSERT INTO approval_requests
             (approval_id, run_id, node_id, checkpoint_id, graph_id, graph_version,
              checkpoint_digest, audience, prompt, prompt_digest, allowed_decisions,
              approval_digest, status, expires_at, created_at)
             VALUES (?1, ?2, 'durable_checkpoint', ?3, ?4, ?5, ?6, ?7, '', ?8, ?9, ?10,
                     'pending', ?11, ?12)",
            params![
                record.approval_id,
                record.run_id,
                record.checkpoint_id,
                record.graph_id,
                record.graph_version,
                record.checkpoint_digest,
                record.audience,
                record.prompt_digest,
                allowed_json,
                record.approval_digest,
                record.expires_at,
                record.created_at,
            ],
        )
        .map_err(|_| ApprovalError::Persistence)?;
        tx.commit().map_err(|_| ApprovalError::Persistence)?;
        Ok(record)
    }

    pub fn get_checkpoint_approval(
        &self,
        approval_id: &str,
    ) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| ApprovalError::IntegrityKeyRequired)?;
        let conn = self.conn.lock().map_err(|_| ApprovalError::Persistence)?;
        let result = conn.query_row(
            &format!(
                "SELECT {APPROVAL_COLUMNS} FROM approval_requests
                      WHERE approval_id = ?1 AND checkpoint_id IS NOT NULL"
            ),
            params![approval_id],
            approval_row,
        );
        match result {
            Ok(parts) => Ok(Some(parse_approval_parts(parts, key)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(_) => Err(ApprovalError::Persistence),
        }
    }

    pub fn list_checkpoint_approvals(
        &self,
        run_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ApprovalRecord>, ApprovalError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| ApprovalError::IntegrityKeyRequired)?;
        let conn = self.conn.lock().map_err(|_| ApprovalError::Persistence)?;
        let mut sql = format!(
            "SELECT {APPROVAL_COLUMNS} FROM approval_requests
                              WHERE checkpoint_id IS NOT NULL"
        );
        if run_id.is_some() {
            sql.push_str(" AND run_id = ?1");
        }
        if status.is_some() {
            sql.push_str(if run_id.is_some() {
                " AND status = ?2"
            } else {
                " AND status = ?1"
            });
        }
        sql.push_str(" ORDER BY created_at DESC, approval_id DESC LIMIT ?3");
        if run_id.is_none() && status.is_none() {
            sql = format!("SELECT {APPROVAL_COLUMNS} FROM approval_requests
                          WHERE checkpoint_id IS NOT NULL ORDER BY created_at DESC, approval_id DESC LIMIT ?1");
        } else if run_id.is_none() || status.is_none() {
            sql = sql.replace("LIMIT ?3", "LIMIT ?2");
        }
        let mut statement = conn.prepare(&sql).map_err(|_| ApprovalError::Persistence)?;
        let mut rows = if run_id.is_some() && status.is_some() {
            statement.query(params![run_id, status, limit.min(200) as i64])
        } else if run_id.is_some() {
            statement.query(params![run_id, limit.min(200) as i64])
        } else if status.is_some() {
            statement.query(params![status, limit.min(200) as i64])
        } else {
            statement.query(params![limit.min(200) as i64])
        }
        .map_err(|_| ApprovalError::Persistence)?;
        let mut approvals = Vec::new();
        while let Some(row) = rows.next().map_err(|_| ApprovalError::Persistence)? {
            approvals.push(parse_approval_parts(
                approval_row(row).map_err(|_| ApprovalError::Persistence)?,
                key,
            )?);
        }
        Ok(approvals)
    }

    pub fn checkpoint_approval_status(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<String>, ApprovalError> {
        let conn = self.conn.lock().map_err(|_| ApprovalError::Persistence)?;
        conn.query_row(
            "SELECT status FROM approval_requests
             WHERE checkpoint_id = ?1 AND checkpoint_id IS NOT NULL
             ORDER BY created_at DESC LIMIT 1",
            params![checkpoint_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ApprovalError::Persistence)
    }

    pub fn decide_checkpoint_approval(
        &self,
        approval_id: &str,
        decision: &str,
        actor: &str,
        now: DateTime<Utc>,
    ) -> Result<ApprovedCheckpoint, ApprovalError> {
        let key = self
            .require_integrity_key()
            .map_err(|_| ApprovalError::IntegrityKeyRequired)?;
        let mut conn = self.conn.lock().map_err(|_| ApprovalError::Persistence)?;
        let tx = conn.transaction().map_err(|_| ApprovalError::Persistence)?;
        let parts = tx
            .query_row(
                &format!(
                    "SELECT {APPROVAL_COLUMNS} FROM approval_requests
                          WHERE approval_id = ?1 AND checkpoint_id IS NOT NULL"
                ),
                params![approval_id],
                approval_row,
            )
            .optional()
            .map_err(|_| ApprovalError::Persistence)?
            .ok_or(ApprovalError::NotFound)?;
        let mut approval = parse_approval_parts(parts, key)?;
        if approval.status == "expired" {
            return Err(ApprovalError::Expired);
        }
        if approval.status != "pending" {
            return Err(ApprovalError::AlreadyDecided);
        }
        let expired = DateTime::parse_from_rfc3339(&approval.expires_at)
            .map_err(|_| ApprovalError::Integrity)?
            .with_timezone(&Utc)
            <= now;
        if expired {
            let checkpoint = load_checkpoint_from_tx(&tx, &approval.checkpoint_id, key)
                .map_err(ApprovalError::Checkpoint)?;
            if checkpoint.run_id != approval.run_id
                || checkpoint.graph_id != approval.graph_id
                || checkpoint.graph_version != approval.graph_version
                || checkpoint.checkpoint_digest != approval.checkpoint_digest
            {
                return Err(ApprovalError::Integrity);
            }
            approval.status = "expired".into();
            approval.decision = None;
            approval.decided_by = None;
            approval.decided_at = Some(now.to_rfc3339_opts(SecondsFormat::Nanos, true));
            approval.approval_digest = approval_digest(&approval, key);
            let changed = tx
                .execute(
                    "UPDATE approval_requests SET status = 'expired', decision = NULL,
                     decided_by = NULL, decided_at = ?2, approval_digest = ?3
                     WHERE approval_id = ?1 AND status = 'pending'",
                    params![approval_id, approval.decided_at, approval.approval_digest],
                )
                .map_err(|_| ApprovalError::Persistence)?;
            if changed != 1 {
                return Err(ApprovalError::AlreadyDecided);
            }
            if checkpoint.consumed_at.is_none() {
                tx.execute(
                    "UPDATE checkpoints SET consumed_at = ?2, status = 'consumed'
                     WHERE checkpoint_id = ?1 AND consumed_at IS NULL",
                    params![
                        approval.checkpoint_id,
                        now.to_rfc3339_opts(SecondsFormat::Nanos, true)
                    ],
                )
                .map_err(|_| ApprovalError::Persistence)?;
            }
            tx.commit().map_err(|_| ApprovalError::Persistence)?;
            return Err(ApprovalError::Expired);
        }

        if !approval
            .allowed_decisions
            .iter()
            .any(|allowed| allowed == decision)
        {
            return Err(ApprovalError::DecisionNotAllowed);
        }

        let checkpoint = load_checkpoint_from_tx(&tx, &approval.checkpoint_id, key)
            .map_err(ApprovalError::Checkpoint)?;
        if checkpoint.run_id != approval.run_id
            || checkpoint.graph_id != approval.graph_id
            || checkpoint.graph_version != approval.graph_version
            || checkpoint.checkpoint_digest != approval.checkpoint_digest
        {
            return Err(ApprovalError::Integrity);
        }
        if checkpoint.consumed_at.is_some() {
            return Err(ApprovalError::Checkpoint(CheckpointError::Consumed));
        }

        let decided_at = now.to_rfc3339_opts(SecondsFormat::Nanos, true);
        let status = if decision == "approve" {
            "approved"
        } else {
            "rejected"
        };
        approval.status = status.into();
        approval.decision = Some(decision.into());
        approval.decided_by = Some(actor.into());
        approval.decided_at = Some(decided_at.clone());
        approval.approval_digest = approval_digest(&approval, key);
        let changed = tx
            .execute(
                "UPDATE approval_requests SET status = ?2, decision = ?3,
                 decided_by = ?4, decided_at = ?5, approval_digest = ?6
                 WHERE approval_id = ?1 AND status = 'pending'",
                params![
                    approval_id,
                    status,
                    decision,
                    actor,
                    decided_at,
                    approval.approval_digest
                ],
            )
            .map_err(|_| ApprovalError::Persistence)?;
        if changed != 1 {
            return Err(ApprovalError::AlreadyDecided);
        }
        let consumed_at = now.to_rfc3339_opts(SecondsFormat::Nanos, true);
        let checkpoint_changed = tx
            .execute(
                "UPDATE checkpoints SET consumed_at = ?2, status = 'consumed'
                 WHERE checkpoint_id = ?1 AND consumed_at IS NULL",
                params![approval.checkpoint_id, consumed_at],
            )
            .map_err(|_| ApprovalError::Persistence)?;
        if checkpoint_changed != 1 {
            return Err(ApprovalError::Checkpoint(CheckpointError::Consumed));
        }
        tx.commit().map_err(|_| ApprovalError::Persistence)?;
        Ok(ApprovedCheckpoint {
            approval,
            checkpoint,
        })
    }

    pub fn approval_receipt_value(approval: &ApprovalRecord) -> Value {
        let decision = approval.decision.clone().unwrap_or_default();
        let decided_by_digest = approval
            .decided_by
            .as_ref()
            .map(|actor| digest(&Value::String(actor.clone())));
        let decision_digest = digest(&serde_json::json!({
            "approval_digest": approval.approval_digest,
            "decision": decision,
            "decided_by_digest": decided_by_digest,
            "decided_at": approval.decided_at,
        }));
        serde_json::json!({
            "approval_id": approval.approval_id,
            "approval_digest": approval.approval_digest,
            "checkpoint_id": approval.checkpoint_id,
            "checkpoint_digest": approval.checkpoint_digest,
            "decision": approval.decision,
            "decided_by_digest": decided_by_digest,
            "decided_at": approval.decided_at,
            "allowed_decisions_digest": digest(&serde_json::json!(approval.allowed_decisions)),
            "audience_digest": digest(&Value::String(approval.audience.clone())),
            "prompt_digest": approval.prompt_digest,
            "decision_digest": decision_digest,
        })
    }

    #[cfg(test)]
    pub(crate) fn fail_checkpoint_persistence(&self) {
        self.checkpoint_persistence_fault
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn save_checkpoint(
        &self,
        run_id: &str,
        node_id: &str,
        attempt: u32,
        input_json: &str,
        output_json: Option<&str>,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "INSERT INTO checkpoints (run_id, node_id, attempt, input_json, output_json, status, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, node_id, attempt) DO UPDATE SET
                output_json = COALESCE(excluded.output_json, output_json),
                status = excluded.status,
                error = COALESCE(excluded.error, error)",
            params![run_id, node_id, attempt, input_json, output_json, status, error],
        )
        .map_err(|e| format!("save_checkpoint error: {e}"))?;
        Ok(())
    }

    // ── Events ──────────────────────────────────────────────────────────

    pub fn save_event(
        &self,
        run_id: &str,
        seq: u64,
        event_type: &str,
        event_json: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO events (run_id, seq, event_type, event_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, seq, event_type, event_json],
        )
        .map_err(|e| format!("save_event error: {e}"))?;
        Ok(())
    }

    pub fn load_events(
        &self,
        run_id: &str,
        cursor: u64,
        limit: usize,
    ) -> Result<Option<Value>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT seq, event_json FROM events WHERE run_id = ?1 AND seq >= ?2 ORDER BY seq LIMIT ?3")
            .map_err(|e| format!("load events error: {e}"))?;
        let events: Vec<Value> = stmt
            .query_map(params![run_id, cursor, limit.min(200) as i64], |row| {
                let seq: u64 = row.get(0)?;
                let event_json: String = row.get(1)?;
                let event = serde_json::from_str::<Value>(&event_json)
                    .unwrap_or_else(|_| serde_json::json!({"receipt":"terminal event persisted with reduced fidelity"}));
                Ok(serde_json::json!({"cursor": seq, "event": event}))
            })
            .map_err(|e| format!("load events error: {e}"))?
            .filter_map(Result::ok)
            .collect();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE run_id = ?1)",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("load events existence error: {e}"))?;
        if !exists {
            return Ok(None);
        }
        let first: u64 = conn
            .query_row(
                "SELECT MIN(seq) FROM events WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("load events first cursor error: {e}"))?;
        let next_cursor: u64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq) + 1, 0) FROM events WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("load events next cursor error: {e}"))?;
        Ok(Some(
            serde_json::json!({"run_id":run_id,"events":events,"next_cursor":next_cursor,"gap":cursor<first,"truncated":false,"dropped":first,"projection":"terminal_persisted_projection","replay_capability":"integrity_only","replayable_execution":false,"resume_supported":false}),
        ))
    }

    // ── Idempotency ─────────────────────────────────────────────────────

    /// Look up a cached idempotent response together with the canonical request
    /// digest it was bound to. A NULL digest is a pre-migration record and must
    /// never be replayed for a new request.
    pub fn check_idempotency(&self, key: &str) -> Result<Option<(Option<String>, Value)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT request_digest, result_json FROM idempotency_keys WHERE key = ?1 AND request_digest IS NOT NULL AND valid = 1")
            .map_err(|e| format!("idempotency error: {e}"))?;
        let result: Option<(Option<String>, String)> = stmt
            .query_row(params![key], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok();
        match result {
            Some((request_digest, json_str)) => serde_json::from_str(&json_str)
                .map(|result| Some((request_digest, result)))
                .map_err(|e| format!("json parse error: {e}")),
            None => Ok(None),
        }
    }

    pub fn save_idempotency(
        &self,
        key: &str,
        request_digest: &str,
        result_json: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|e| format!("lock error: {e}"))?;
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO idempotency_keys (key, request_digest, result_json) VALUES (?1, ?2, ?3)",
            params![key, request_digest, result_json],
        )
        .map_err(|e| format!("idempotency error: {e}"))?;
        Ok(inserted == 1)
    }

    pub fn data_dir(&self) -> Option<PathBuf> {
        // Returns None since we don't store the path separately.
        // The store is identified by its existence, not a path reference.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configure_test_integrity_key() {
        let path = std::env::temp_dir().join("agent-graph-mcp-unit-integrity.key");
        std::fs::write(&path, [0x5au8; 32]).expect("test integrity key");
        std::env::set_var("AGENT_GRAPH_INTEGRITY_KEY_PATH", path);
    }

    #[test]
    fn graph_delete_fault_rolls_back_graph_and_versions_together() {
        let temp = tempfile::tempdir().expect("graph database");
        let store = PersistentStore::open(temp.path()).expect("store");
        store
            .save_graph("atomic-delete", "{\"name\":\"atomic-delete\"}", "v1", false)
            .expect("graph");
        store
            .set_graph_retention(
                "atomic-delete",
                "delete_candidate",
                "test deletion rollback",
                "unit-test",
                None,
            )
            .expect("candidate");
        store
            .set_graph_retention(
                "atomic-delete",
                "delete_approved",
                "test deletion rollback approved",
                "unit-test",
                None,
            )
            .expect("approval");
        store.fail_graph_delete_after_graph_row();
        assert!(store.delete_graph("atomic-delete").is_err());
        assert!(store.load_graph("atomic-delete").unwrap().is_some());
        assert_eq!(
            store.list_graph_versions("atomic-delete").unwrap(),
            vec!["v1"]
        );
    }

    #[test]
    fn checkpoint_persistence_fault_leaves_no_resumable_row() {
        configure_test_integrity_key();
        let temp = tempfile::tempdir().expect("checkpoint database");
        let store = PersistentStore::open(temp.path()).expect("store");
        store
            .save_graph("checkpoint-fault", "{}", "version", false)
            .expect("graph");
        store
            .save_execution_with_budgets(
                "run-checkpoint-fault",
                "checkpoint-fault",
                "version",
                "checkpointed",
                "{}",
                Some("null"),
            )
            .expect("execution");
        store.fail_checkpoint_persistence();
        let result = store.create_resume_checkpoint(
            "run-checkpoint-fault",
            "checkpoint-fault",
            "version",
            "entry",
            &serde_json::json!({"__input__":null}),
            &Value::Null,
            &serde_json::json!({"nodes":0,"llm_calls":0,"wall_clock_ms":0}),
            &serde_json::json!({"eligible":true}),
            0,
            0,
        );
        assert_eq!(result, Err(CheckpointError::Persistence));
        assert_eq!(
            store.load_resume_checkpoint(None, Some("run-checkpoint-fault")),
            Ok(None)
        );
    }
}
