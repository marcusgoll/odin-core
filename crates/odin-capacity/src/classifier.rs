//! Task classifier: scores tasks by priority, cost tier, and urgency decay.
//!
//! Three dimensions scored at ingestion:
//! - **Priority** (source trust + task type): 0-100
//! - **Cost tier** (predicted compute): Architect > Standard > Lightweight > Local
//! - **Urgency decay**: exponential decay based on TTL

use serde::{Deserialize, Serialize};

/// Cost tier for a task, predicting compute requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostTier {
    Architect,
    Standard,
    Lightweight,
    Local,
}

/// Which lane a task should be routed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Active,
    Held,
    Deferred,
}

/// Input to the classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    pub task_type: String,
    pub source: String,
    pub trust_level: String,
    pub created_at: String,
    #[serde(default)]
    pub ttl_seconds: Option<u64>,
}

/// Output of classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedTask {
    pub priority: u32,
    pub cost_tier: CostTier,
    pub urgency: f64,
    pub ttl_seconds: u64,
    pub lane: Lane,
}

/// Classifies tasks for scheduling decisions.
pub struct TaskClassifier;

impl TaskClassifier {
    pub fn classify(input: &TaskInput) -> ClassifiedTask {
        let priority = Self::score_priority(&input.task_type, &input.trust_level);
        let cost_tier = Self::score_cost_tier(&input.task_type);
        let ttl_seconds = input.ttl_seconds.unwrap_or(86400);
        let urgency = Self::compute_urgency(priority, ttl_seconds, 0);
        let lane = Lane::Active;

        ClassifiedTask {
            priority,
            cost_tier,
            urgency,
            ttl_seconds,
            lane,
        }
    }

    fn score_priority(task_type: &str, trust_level: &str) -> u32 {
        let base = match task_type {
            "alert" | "sentry_fix" => 85,
            "issue_implement" | "pr_fix" => 70,
            "pr_review" | "acceptance_test" => 50,
            "daily_standup" | "health_check" | "daily_report" => 30,
            _ => 50,
        };

        match trust_level {
            "operator" => 90.max(base),
            _ => base,
        }
    }

    fn score_cost_tier(task_type: &str) -> CostTier {
        match task_type {
            "research" | "study_deep" | "arch_review" | "arch_audit" => CostTier::Architect,
            "issue_implement" | "pr_fix" | "spec_create" => CostTier::Standard,
            "pr_review" | "acceptance_test" | "quality_gate" => CostTier::Lightweight,
            "health_check" | "daily_report" | "daily_standup" => CostTier::Local,
            _ => CostTier::Standard,
        }
    }

    fn compute_urgency(priority: u32, ttl_seconds: u64, age_seconds: u64) -> f64 {
        if ttl_seconds == 0 {
            return priority as f64;
        }
        let lambda = 1.0 / ttl_seconds as f64;
        priority as f64 * (-lambda * age_seconds as f64).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_gets_max_priority() {
        let input = TaskInput {
            task_type: "health_check".into(),
            source: "telegram".into(),
            trust_level: "operator".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: Some(1800),
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.priority, 90);
    }

    #[test]
    fn alert_has_high_priority() {
        let input = TaskInput {
            task_type: "alert".into(),
            source: "n8n".into(),
            trust_level: "internal".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: Some(1800),
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.priority, 85);
    }

    #[test]
    fn pr_review_is_lightweight() {
        let input = TaskInput {
            task_type: "pr_review".into(),
            source: "github_event".into(),
            trust_level: "webhook".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: Some(14400),
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.cost_tier, CostTier::Lightweight);
        assert_eq!(result.priority, 50);
    }

    #[test]
    fn research_is_architect_tier() {
        let input = TaskInput {
            task_type: "research".into(),
            source: "telegram".into(),
            trust_level: "operator".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: Some(86400),
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.cost_tier, CostTier::Architect);
    }

    #[test]
    fn urgency_decays_with_age() {
        let fresh = TaskClassifier::compute_urgency(70, 3600, 0);
        let aged = TaskClassifier::compute_urgency(70, 3600, 1800);
        let old = TaskClassifier::compute_urgency(70, 3600, 3600);
        assert!(fresh > aged);
        assert!(aged > old);
        // At TTL, urgency should be ~37% of base (e^-1)
        assert!((old / fresh - (-1.0_f64).exp()).abs() < 0.01);
    }

    #[test]
    fn default_lane_is_active() {
        let input = TaskInput {
            task_type: "issue_implement".into(),
            source: "n8n".into(),
            trust_level: "internal".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: None,
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.lane, Lane::Active);
        assert_eq!(result.ttl_seconds, 86400);
    }

    #[test]
    fn scheduled_tasks_low_priority() {
        let input = TaskInput {
            task_type: "daily_standup".into(),
            source: "n8n".into(),
            trust_level: "internal".into(),
            created_at: "2026-03-09T12:00:00Z".into(),
            ttl_seconds: Some(7200),
        };
        let result = TaskClassifier::classify(&input);
        assert_eq!(result.priority, 30);
        assert_eq!(result.cost_tier, CostTier::Local);
    }

    #[test]
    fn zero_ttl_returns_base_urgency() {
        let u = TaskClassifier::compute_urgency(70, 0, 100);
        assert_eq!(u, 70.0);
    }
}
