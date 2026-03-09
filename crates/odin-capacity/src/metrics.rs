//! Observability metrics export.
//!
//! Appends scheduling metrics to JSONL files for trend analysis.

use serde::{Deserialize, Serialize};
use std::io::Write;

/// Metrics snapshot from a scheduling cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsExport {
    pub active_lane_depth: u32,
    pub held_lane_depth: u32,
    pub deferred_lane_depth: u32,
    pub agent_count: u32,
    pub target_agent_count: u32,
    pub spend_today_usd: f64,
    pub spend_ceiling_usd: f64,
    pub throughput_tasks_per_hour: f64,
    pub health: String,
    pub timestamp: String,
}

/// Appends a metrics snapshot as one JSON line to the given path.
pub fn append_to_jsonl(path: &std::path::Path, metrics: &MetricsExport) -> std::io::Result<()> {
    let line = serde_json::to_string(metrics)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_serializes_to_json() {
        let m = MetricsExport {
            active_lane_depth: 12,
            held_lane_depth: 3,
            deferred_lane_depth: 0,
            agent_count: 5,
            target_agent_count: 7,
            spend_today_usd: 14.20,
            spend_ceiling_usd: 50.0,
            throughput_tasks_per_hour: 18.4,
            health: "scaling_up".into(),
            timestamp: "2026-03-09T14:00:00Z".into(),
        };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"active_lane_depth\":12"));
        assert!(json.contains("\"health\":\"scaling_up\""));
    }

    #[test]
    fn append_to_jsonl_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-metrics.jsonl");
        let m = MetricsExport {
            active_lane_depth: 5,
            held_lane_depth: 0,
            deferred_lane_depth: 0,
            agent_count: 3,
            target_agent_count: 3,
            spend_today_usd: 0.0,
            spend_ceiling_usd: 50.0,
            throughput_tasks_per_hour: 0.0,
            health: "steady".into(),
            timestamp: "2026-03-09T14:00:00Z".into(),
        };
        append_to_jsonl(&path, &m).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
    }
}
