use crate::error::{AgentGraphError, Result};
use serde_json::Value;

/// A reducer combines old and new values during state updates.
/// Critical for correctness during parallel execution.
pub trait Reducer: Send + Sync {
    /// Combine the current value with the new value
    fn reduce(&self, current: &Value, update: &Value) -> Result<Value>;
}

/// Default reducer: the new value replaces the old value.
pub struct LastWriteWins;

impl Reducer for LastWriteWins {
    fn reduce(&self, _current: &Value, update: &Value) -> Result<Value> {
        Ok(update.clone())
    }
}

/// Append reducer: appends items from the update array to the current array.
pub struct AppendReducer;

impl Reducer for AppendReducer {
    fn reduce(&self, current: &Value, update: &Value) -> Result<Value> {
        match (current, update) {
            (Value::Null, val) => {
                // First write: if update is array, use it directly; otherwise wrap in array
                if let Value::Array(_) = val {
                    Ok(val.clone())
                } else {
                    Ok(Value::Array(vec![val.clone()]))
                }
            }
            (Value::Array(curr), Value::Array(upd)) => {
                let mut result = curr.clone();
                result.extend(upd.iter().cloned());
                Ok(Value::Array(result))
            }
            (Value::Array(curr), val) => {
                let mut result = curr.clone();
                result.push(val.clone());
                Ok(Value::Array(result))
            }
            (_, Value::Array(upd)) => {
                let mut result = vec![current.clone()];
                result.extend(upd.iter().cloned());
                Ok(Value::Array(result))
            }
            _ => Ok(Value::Array(vec![current.clone(), update.clone()])),
        }
    }
}

/// Add reducer: adds numeric values together.
pub struct AddReducer;

impl Reducer for AddReducer {
    fn reduce(&self, current: &Value, update: &Value) -> Result<Value> {
        let a_f = match current {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::Null => 0.0,
            _ => {
                return Err(AgentGraphError::StateError(
                    "AddReducer: current value must be a number".to_string(),
                ))
            }
        };
        let b_f = match update {
            Value::Number(n) => n.as_f64().ok_or_else(|| {
                AgentGraphError::StateError("AddReducer: cannot convert update to f64".to_string())
            })?,
            _ => {
                return Err(AgentGraphError::StateError(
                    "AddReducer: update value must be a number".to_string(),
                ))
            }
        };
        Ok(serde_json::json!(a_f + b_f))
    }
}

/// Merge reducer: deep-merges JSON objects.
pub struct MergeReducer;

impl Reducer for MergeReducer {
    fn reduce(&self, current: &Value, update: &Value) -> Result<Value> {
        match (current, update) {
            (Value::Object(curr), Value::Object(upd)) => {
                let mut result = curr.clone();
                for (k, v) in upd {
                    if let Some(existing) = result.get(k) {
                        if existing.is_object() && v.is_object() {
                            result.insert(k.clone(), self.reduce(existing, v)?);
                        } else {
                            result.insert(k.clone(), v.clone());
                        }
                    } else {
                        result.insert(k.clone(), v.clone());
                    }
                }
                Ok(Value::Object(result))
            }
            _ => Ok(update.clone()),
        }
    }
}

/// Closure-based reducer for custom logic.
pub struct FnReducer<F>
where
    F: Fn(&Value, &Value) -> Result<Value> + Send + Sync,
{
    func: F,
}

impl<F> FnReducer<F>
where
    F: Fn(&Value, &Value) -> Result<Value> + Send + Sync,
{
    pub fn new(func: F) -> Self {
        Self { func }
    }
}

impl<F> Reducer for FnReducer<F>
where
    F: Fn(&Value, &Value) -> Result<Value> + Send + Sync,
{
    fn reduce(&self, current: &Value, update: &Value) -> Result<Value> {
        (self.func)(current, update)
    }
}
