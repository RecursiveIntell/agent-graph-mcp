//! Lock-owning daemon primitives.
use crate::migrations;
use fs2::FileExt;
use rusqlite::{Connection, OptionalExtension};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[allow(dead_code)]
pub const MAX_FRAME: usize = 1024 * 1024;
#[derive(Debug)]
pub enum DaemonError {
    AlreadyOwned,
    Io(io::Error),
    Sql(rusqlite::Error),
}
impl From<io::Error> for DaemonError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<rusqlite::Error> for DaemonError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyOwned => write!(f, "data directory already owned by another daemon"),
            Self::Io(e) => write!(f, "daemon io error: {e}"),
            Self::Sql(e) => write!(f, "daemon sql error: {e}"),
        }
    }
}

impl std::error::Error for DaemonError {}
impl DaemonError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadyOwned => "DATA_DIR_ALREADY_OWNED",
            Self::Io(_) => "DAEMON_IO",
            Self::Sql(_) => "DAEMON_SQL",
        }
    }
}
#[derive(Debug)]
pub struct DaemonLock {
    file: File,
    #[allow(dead_code)]
    pub path: PathBuf,
}
impl DaemonLock {
    pub fn acquire(data_dir: &Path) -> Result<Self, DaemonError> {
        fs::create_dir_all(data_dir)?;
        let path = data_dir.join("daemon.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        file.try_lock_exclusive().map_err(|e| {
            if e.kind() == io::ErrorKind::WouldBlock {
                DaemonError::AlreadyOwned
            } else {
                DaemonError::Io(e)
            }
        })?;
        Ok(Self { file, path })
    }
}
impl Drop for DaemonLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = self.file.sync_all();
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DaemonIdentity {
    pub instance_id: String,
    pub generation: i64,
    pub pid: u32,
    pub started_at: String,
}

#[allow(dead_code)]
pub fn identity(conn: &Connection) -> rusqlite::Result<DaemonIdentity> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let instance_id = format!(
        "{}-{}-{}",
        std::process::id(),
        now,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let generation: i64 = conn.query_row(
        "SELECT COALESCE(MAX(generation),0)+1 FROM daemon_instances",
        [],
        |r| r.get(0),
    )?;
    let started_at = now.to_string();
    conn.execute(
        "INSERT INTO daemon_instances(instance_id,generation,pid,started_at,heartbeat_at) VALUES (?1,?2,?3,?4,?4)",
        rusqlite::params![instance_id, generation, std::process::id(), started_at],
    )?;
    Ok(DaemonIdentity {
        instance_id,
        generation,
        pid: std::process::id(),
        started_at,
    })
}

#[allow(dead_code)]
pub fn recover_owned_state(
    conn: &Connection,
    instance_id: &str,
    generation: i64,
) -> rusqlite::Result<usize> {
    let has_executions: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='executions'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();

    let has_owner_column: bool = if has_executions {
        conn.query_row(
            "SELECT 1 FROM pragma_table_info('executions') WHERE name='owner_instance_id'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some()
    } else {
        false
    };

    let changed = if has_executions && has_owner_column {
        conn.execute(
            "UPDATE executions SET status='legacy_unverified' WHERE owner_instance_id IS NULL AND status IN ('accepted','running')",
            [],
        )?
    } else {
        0
    };

    conn.execute(
        "UPDATE daemon_instances SET heartbeat_at=CURRENT_TIMESTAMP WHERE instance_id=?1 AND generation=?2",
        rusqlite::params![instance_id, generation],
    )?;
    Ok(changed)
}
pub fn open_owned(
    data_dir: &Path,
    binary_digest: &str,
) -> Result<(DaemonLock, Connection), DaemonError> {
    let lock = DaemonLock::acquire(data_dir)?;
    let mut c = Connection::open(data_dir.join("agent-graph.db"))?;
    migrations::apply(&mut c, binary_digest)?;
    Ok((lock, c))
}

/// Persist the integrity mode and reject mixed keyless/key-enabled restarts.
pub fn enforce_startup_mode(conn: &Connection, key_enabled: bool) -> rusqlite::Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS daemon_startup_mode (mode TEXT PRIMARY KEY CHECK(mode IN ('keyless','key-enabled')));")?;
    let expected = if key_enabled {
        "key-enabled"
    } else {
        "keyless"
    };
    let current: Option<String> = conn
        .query_row("SELECT mode FROM daemon_startup_mode LIMIT 1", [], |r| {
            r.get(0)
        })
        .optional()?;
    match current {
        Some(mode) if mode != expected => Err(rusqlite::Error::InvalidParameterName(
            "STARTUP_MODE_MISMATCH".into(),
        )),
        Some(_) => Ok(()),
        None => {
            conn.execute(
                "INSERT INTO daemon_startup_mode(mode) VALUES (?1)",
                [expected],
            )?;
            Ok(())
        }
    }
}
#[allow(dead_code)]
pub fn socket_path(runtime_dir: &Path, instance: &str) -> PathBuf {
    runtime_dir
        .join("agent-graph")
        .join(instance)
        .join("daemon.sock")
}
