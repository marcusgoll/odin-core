//! Audit interface and baseline record types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("write failure: {0}")]
    Write(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuditRecord {
    pub ts_unix: u64,
    pub event_type: String,
    pub request_id: Option<String>,
    pub task_id: Option<String>,
    pub project: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

pub trait AuditSink: Send + Sync {
    fn record(&self, record: AuditRecord) -> Result<(), AuditError>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopAuditSink;

impl AuditSink for NoopAuditSink {
    fn record(&self, _record: AuditRecord) -> Result<(), AuditError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_sink_accepts_records() {
        let sink = NoopAuditSink;
        let result = sink.record(AuditRecord {
            ts_unix: 1,
            event_type: "policy.decision".to_string(),
            request_id: Some("r1".to_string()),
            task_id: None,
            project: Some("demo".to_string()),
            metadata: Value::Null,
        });

        assert!(result.is_ok());
    }
}
