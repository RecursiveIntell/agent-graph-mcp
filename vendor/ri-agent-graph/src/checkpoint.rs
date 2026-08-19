use crate::error::Result;
use crate::state::StateSnapshot;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub execution_id: String,
    pub timestamp: DateTime<Utc>,
    pub current_node: String,
    pub iteration: usize,
    pub state: StateSnapshot,
    #[serde(default)]
    pub step_number: usize,
    #[serde(default)]
    pub active_nodes: Vec<String>,
}

pub struct CheckpointManager {
    conn: Connection,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                execution_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                current_node TEXT NOT NULL,
                iteration INTEGER NOT NULL,
                state_data TEXT NOT NULL,
                PRIMARY KEY (execution_id, timestamp)
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Save a checkpoint
    pub fn save(&self, checkpoint: &Checkpoint) -> Result<()> {
        let state_json = serde_json::to_string(&checkpoint.state)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO checkpoints (execution_id, timestamp, current_node, iteration, state_data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &checkpoint.execution_id,
                checkpoint.timestamp.to_rfc3339(),
                &checkpoint.current_node,
                checkpoint.iteration as i64,
                &state_json,
            ],
        )?;

        Ok(())
    }

    /// Load the most recent checkpoint for an execution
    pub fn load(&self, execution_id: &str) -> Result<Option<Checkpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, current_node, iteration, state_data
             FROM checkpoints
             WHERE execution_id = ?1
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let checkpoint = stmt
            .query_row(params![execution_id], |row| {
                let timestamp_str: String = row.get(0)?;
                let state_json: String = row.get(3)?;

                let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc);
                let state: StateSnapshot = serde_json::from_str(&state_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(Checkpoint {
                    execution_id: execution_id.to_string(),
                    timestamp,
                    current_node: row.get(1)?,
                    iteration: row.get::<_, i64>(2)? as usize,
                    state,
                    step_number: 0,
                    active_nodes: Vec::new(),
                })
            })
            .optional()?;

        Ok(checkpoint)
    }

    /// Load all checkpoints for an execution (ordered by timestamp)
    pub fn load_all(&self, execution_id: &str) -> Result<Vec<Checkpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, current_node, iteration, state_data
             FROM checkpoints
             WHERE execution_id = ?1
             ORDER BY timestamp ASC",
        )?;

        let checkpoints = stmt
            .query_map(params![execution_id], |row| {
                let timestamp_str: String = row.get(0)?;
                let state_json: String = row.get(3)?;

                let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc);
                let state: StateSnapshot = serde_json::from_str(&state_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(Checkpoint {
                    execution_id: execution_id.to_string(),
                    timestamp,
                    current_node: row.get(1)?,
                    iteration: row.get::<_, i64>(2)? as usize,
                    state,
                    step_number: 0,
                    active_nodes: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(checkpoints)
    }

    /// Delete all checkpoints for an execution
    pub fn clear(&self, execution_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM checkpoints WHERE execution_id = ?1",
            params![execution_id],
        )?;
        Ok(())
    }
}
