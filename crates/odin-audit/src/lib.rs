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
    /// Optional run identifier; defaults to None for backward compatibility.
    #[serde(default)]
    pub run_id: Option<String>,
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
            run_id: None,
            task_id: None,
            project: Some("demo".to_string()),
            metadata: Value::Null,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn deserializing_legacy_record_without_run_id_defaults_to_none() {
        let legacy = r#"{
            "ts_unix": 1,
            "event_type": "policy.decision",
            "request_id": "r1",
            "task_id": null,
            "project": "demo",
            "metadata": {"decision": "allow"}
        }"#;

        let record: AuditRecord =
            serde_json::from_str(legacy).expect("legacy record should deserialize");

        assert_eq!(record.run_id, None);
        assert_eq!(record.request_id.as_deref(), Some("r1"));
        assert_eq!(record.event_type, "policy.decision");
    }
}
