use crate::error::{AgentGraphError, Result};
use crate::reducer::Reducer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Resource bounds for agent state (PRIMITIVES_CONTRACT §3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateLimits {
    /// Maximum number of keys allowed in state. Default: 10_000.
    pub max_keys: usize,
    /// Maximum size in bytes for a single serialized value. Default: 1_048_576 (1 MiB).
    pub max_value_bytes: usize,
    /// Maximum number of history snapshots to retain. Default: 100.
    pub max_history_len: usize,
    /// Timeout for acquiring state locks. Default: 5 s.
    pub lock_timeout: Duration,
}

impl Default for StateLimits {
    fn default() -> Self {
        Self {
            max_keys: 10_000,
            max_value_bytes: 1_048_576,
            max_history_len: 100,
            lock_timeout: Duration::from_secs(5),
        }
    }
}

/// A transaction over [`AgentState`] that can be committed or rolled back.
///
/// Created via [`AgentState::transaction()`]. Writes are staged against an
/// isolated working copy and only become visible when
/// [`commit()`](Self::commit) is called.
pub struct StateTransaction {
    state: AgentState,
    working: RwLock<HashMap<String, Value>>,
    snapshot_version: u64,
    committed: bool,
}

impl StateTransaction {
    /// Read a value within the transaction.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T> {
        let working = self.working.read().await;
        let value = working
            .get(key)
            .ok_or_else(|| AgentGraphError::StateError(format!("Key not found: {}", key)))?;

        serde_json::from_value(value.clone()).map_err(|e| {
            AgentGraphError::StateError(format!("Failed to deserialize {}: {}", key, e))
        })
    }

    /// Write a value within the transaction.
    pub async fn set<T: Serialize>(&self, key: &str, value: T) -> Result<()> {
        let json_value = self.state.serialize_value(key, value)?;
        let existing = {
            let working = self.working.read().await;
            working.get(key).cloned()
        };
        let next_value = if let Some(existing) = existing.as_ref() {
            self.state
                .reduce_value_if_needed(key, existing, &json_value)
                .await?
        } else {
            json_value
        };

        let mut working = self.working.write().await;
        self.state.validate_insert(&working, key, &next_value)?;
        working.insert(key.to_string(), next_value);
        Ok(())
    }

    /// Commit the transaction. After this call, staged changes are applied atomically.
    ///
    /// Returns an error if the underlying state was modified concurrently since
    /// this transaction was created.
    pub async fn commit(mut self) -> Result<()> {
        let current_version = self.state.version.load(Ordering::SeqCst);
        if current_version != self.snapshot_version {
            return Err(AgentGraphError::StateError(
                "Transaction conflict: state was modified concurrently".to_string(),
            ));
        }
        let next = self.working.read().await.clone();
        // replace_data increments the version internally.
        self.state.replace_data(next).await;
        self.committed = true;
        Ok(())
    }

    /// Roll back all staged changes.
    pub async fn rollback(mut self) {
        self.committed = true;
    }
}

impl Drop for StateTransaction {
    fn drop(&mut self) {
        if !self.committed {
            tracing::debug!("dropping uncommitted state transaction");
        }
    }
}

/// Shared state that persists across graph execution.
/// All nodes can read and write to this state.
#[derive(Clone)]
pub struct AgentState {
    data: Arc<RwLock<HashMap<String, Value>>>,
    history: Arc<RwLock<Vec<StateSnapshot>>>,
    pub(crate) reducers: Arc<RwLock<HashMap<String, Arc<dyn Reducer>>>>,
    limits: StateLimits,
    version: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: HashMap<String, Value>,
}

impl AgentState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::with_limits(StateLimits::default())
    }

    /// Create a new empty state with explicit limits.
    pub fn with_limits(limits: StateLimits) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            reducers: Arc::new(RwLock::new(HashMap::new())),
            limits,
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create state with initial data
    pub fn with_data(data: HashMap<String, Value>) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            history: Arc::new(RwLock::new(Vec::new())),
            reducers: Arc::new(RwLock::new(HashMap::new())),
            limits: StateLimits::default(),
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Create state with initial data and explicit limits.
    pub fn with_data_and_limits(data: HashMap<String, Value>, limits: StateLimits) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            history: Arc::new(RwLock::new(Vec::new())),
            reducers: Arc::new(RwLock::new(HashMap::new())),
            limits,
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Register a reducer for a specific state key.
    /// When `set()` is called on this key and a value already exists,
    /// the reducer is used to combine old and new values.
    pub async fn register_reducer(&self, key: impl Into<String>, reducer: impl Reducer + 'static) {
        match self.write_reducers().await {
            Ok(mut reducers) => {
                reducers.insert(key.into(), Arc::new(reducer));
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to register reducer");
            }
        }
    }

    /// Get a value from state (type-safe)
    pub async fn get<T>(&self, key: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let data = self.read_data().await?;
        let value = data
            .get(key)
            .ok_or_else(|| AgentGraphError::StateError(format!("Key not found: {}", key)))?;

        serde_json::from_value(value.clone()).map_err(|e| {
            AgentGraphError::StateError(format!("Failed to deserialize {}: {}", key, e))
        })
    }

    /// Try to get a value (returns None if missing)
    pub async fn get_opt<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let data = self.read_data().await?;
        match data.get(key) {
            Some(value) => {
                let v = serde_json::from_value(value.clone())?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    /// Set a value in state (type-safe).
    /// If a reducer is registered for this key and a value already exists,
    /// the reducer is used to combine old and new values.
    pub async fn set<T>(&self, key: &str, value: T) -> Result<()>
    where
        T: Serialize,
    {
        let json_value = self.serialize_value(key, value)?;
        let existing = {
            let data = self.read_data().await?;
            data.get(key).cloned()
        };
        let next_value = if let Some(existing) = existing.as_ref() {
            self.reduce_value_if_needed(key, existing, &json_value)
                .await?
        } else {
            json_value
        };

        let mut data = self.write_data().await?;
        self.validate_insert(&data, key, &next_value)?;
        data.insert(key.to_string(), next_value);
        self.version.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Set a raw JSON value without applying reducers.
    pub async fn set_raw(&self, key: &str, value: Value) -> Result<()> {
        let mut data = self.write_data().await?;
        self.validate_insert(&data, key, &value)?;
        data.insert(key.to_string(), value);
        self.version.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// Apply a reducer to combine current and new values for a key.
    /// If no reducer is registered, returns the new value (last-write-wins).
    pub async fn apply_reducer(&self, key: &str, current: &Value, new: &Value) -> Result<Value> {
        let reducers = self.read_reducers().await?;
        if let Some(reducer) = reducers.get(key) {
            reducer.reduce(current, new)
        } else {
            Ok(new.clone())
        }
    }

    /// Update a value using a closure
    pub async fn update<T, F>(&self, key: &str, f: F) -> Result<()>
    where
        T: serde::de::DeserializeOwned + Serialize,
        F: FnOnce(T) -> T,
    {
        let mut data = self.write_data().await?;

        if let Some(value) = data.get(key).cloned() {
            let current: T = serde_json::from_value(value)?;
            let updated = f(current);
            let updated = self.serialize_value(key, updated)?;
            self.validate_insert(&data, key, &updated)?;
            data.insert(key.to_string(), updated);
            self.version.fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    }

    /// Check if a key exists
    pub async fn contains(&self, key: &str) -> bool {
        match self.read_data().await {
            Ok(data) => data.contains_key(key),
            Err(_) => false,
        }
    }

    /// Remove a key
    pub async fn remove(&self, key: &str) -> Option<Value> {
        match self.write_data().await {
            Ok(mut data) => {
                let removed = data.remove(key);
                if removed.is_some() {
                    self.version.fetch_add(1, Ordering::SeqCst);
                }
                removed
            }
            Err(_) => None,
        }
    }

    /// Get all keys
    pub async fn keys(&self) -> Vec<String> {
        match self.read_data().await {
            Ok(data) => data.keys().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Create a snapshot of current state
    pub async fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            timestamp: chrono::Utc::now(),
            data: self
                .read_data()
                .await
                .map(|data| data.clone())
                .unwrap_or_default(),
        }
    }

    /// Save current state to history
    pub async fn save_to_history(&self) {
        let snapshot = self.snapshot().await;
        if let Ok(mut history) = self.write_history().await {
            if history.len() >= self.limits.max_history_len && !history.is_empty() {
                history.remove(0);
            }
            history.push(snapshot);
        }
    }

    /// Restore state from snapshot
    pub async fn restore(&self, snapshot: &StateSnapshot) {
        self.replace_data(snapshot.data.clone()).await;
    }

    /// Get state history
    pub async fn get_history(&self) -> Vec<StateSnapshot> {
        self.read_history()
            .await
            .map(|history| history.clone())
            .unwrap_or_default()
    }

    /// Export state as HashMap for serialization
    pub async fn export(&self) -> HashMap<String, Value> {
        self.read_data()
            .await
            .map(|data| data.clone())
            .unwrap_or_default()
    }

    /// Begin a transaction. Captures a snapshot so changes can be rolled back.
    pub async fn transaction(&self) -> StateTransaction {
        let snapshot_version = self.version.load(Ordering::SeqCst);
        StateTransaction {
            state: self.clone(),
            working: RwLock::new(self.export().await),
            snapshot_version,
            committed: false,
        }
    }

    /// Create an independent deep copy of this state for parallel branches.
    /// The forked state has its own data storage but shares the same reducers.
    pub async fn fork(&self) -> AgentState {
        let data = self.export().await;
        AgentState {
            data: Arc::new(RwLock::new(data)),
            history: Arc::new(RwLock::new(Vec::new())),
            reducers: self.reducers.clone(),
            limits: self.limits.clone(),
            version: Arc::new(AtomicU64::new(0)),
        }
    }

    fn serialize_value<T: Serialize>(&self, key: &str, value: T) -> Result<Value> {
        let json_value = serde_json::to_value(value)?;
        self.validate_value_size(key, &json_value)?;
        Ok(json_value)
    }

    fn validate_value_size(&self, key: &str, value: &Value) -> Result<()> {
        let bytes = serde_json::to_vec(value)?.len();
        if bytes > self.limits.max_value_bytes {
            return Err(AgentGraphError::StateError(format!(
                "Value for key '{}' exceeds max size: {} > {} bytes",
                key, bytes, self.limits.max_value_bytes
            )));
        }
        Ok(())
    }

    fn validate_insert(
        &self,
        data: &HashMap<String, Value>,
        key: &str,
        value: &Value,
    ) -> Result<()> {
        self.validate_value_size(key, value)?;
        if !data.contains_key(key) && data.len() >= self.limits.max_keys {
            return Err(AgentGraphError::StateError(format!(
                "State key limit exceeded: {} >= {}",
                data.len() + 1,
                self.limits.max_keys
            )));
        }
        Ok(())
    }

    async fn reduce_value_if_needed(
        &self,
        key: &str,
        current: &Value,
        new: &Value,
    ) -> Result<Value> {
        let reducers = self.read_reducers().await?;
        if let Some(reducer) = reducers.get(key) {
            reducer.reduce(current, new)
        } else {
            Ok(new.clone())
        }
    }

    async fn replace_data(&self, next: HashMap<String, Value>) {
        if let Err(error) = self.validate_state_map(&next) {
            tracing::warn!(error = %error, "rejected state replacement that exceeded limits");
            return;
        }

        if let Ok(mut data) = self.write_data().await {
            *data = next;
            self.version.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn validate_state_map(&self, data: &HashMap<String, Value>) -> Result<()> {
        if data.len() > self.limits.max_keys {
            return Err(AgentGraphError::StateError(format!(
                "State key limit exceeded: {} > {}",
                data.len(),
                self.limits.max_keys
            )));
        }

        for (key, value) in data {
            self.validate_value_size(key, value)?;
        }

        Ok(())
    }

    async fn read_data(&self) -> Result<tokio::sync::RwLockReadGuard<'_, HashMap<String, Value>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.data.read())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring state read lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }

    async fn write_data(
        &self,
    ) -> Result<tokio::sync::RwLockWriteGuard<'_, HashMap<String, Value>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.data.write())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring state write lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }

    async fn read_history(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Vec<StateSnapshot>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.history.read())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring state history read lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }

    async fn write_history(&self) -> Result<tokio::sync::RwLockWriteGuard<'_, Vec<StateSnapshot>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.history.write())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring state history write lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }

    async fn read_reducers(
        &self,
    ) -> Result<tokio::sync::RwLockReadGuard<'_, HashMap<String, Arc<dyn Reducer>>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.reducers.read())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring reducer read lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }

    async fn write_reducers(
        &self,
    ) -> Result<tokio::sync::RwLockWriteGuard<'_, HashMap<String, Arc<dyn Reducer>>>> {
        tokio::time::timeout(self.limits.lock_timeout, self.reducers.write())
            .await
            .map_err(|_| {
                AgentGraphError::StateError(format!(
                    "Timed out acquiring reducer write lock after {} ms",
                    self.limits.lock_timeout.as_millis()
                ))
            })
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentState")
            .field("data", &"<locked>")
            .finish()
    }
}
