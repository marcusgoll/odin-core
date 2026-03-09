//! Hetzner overflow routing for burst capacity.
//!
//! Routes tasks to remote Hetzner VPS when local infrastructure is saturated
//! and overflow budget is available.

use serde::{Deserialize, Serialize};

use crate::capacity::InfraState;

/// Overflow routing decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowDecision {
    NoOverflow,
    RouteToHetzner { role: String, reason: String },
}

/// Configuration for overflow routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverflowConfig {
    pub swap_threshold: u32,
    pub active_depth_threshold: u32,
    pub budget_usd: f64,
}

impl Default for OverflowConfig {
    fn default() -> Self {
        Self {
            swap_threshold: 50,
            active_depth_threshold: 30,
            budget_usd: 10.0,
        }
    }
}

/// Determines whether to route work to Hetzner overflow.
pub fn should_overflow(
    infra: &InfraState,
    active_depth: u32,
    budget_remaining: f64,
    role: &str,
    config: &OverflowConfig,
) -> OverflowDecision {
    if infra.swap_pct > config.swap_threshold
        && active_depth > config.active_depth_threshold
        && budget_remaining > 0.0
    {
        OverflowDecision::RouteToHetzner {
            role: role.to_string(),
            reason: format!(
                "swap={}% depth={} budget_remaining={:.2}",
                infra.swap_pct, active_depth, budget_remaining
            ),
        }
    } else {
        OverflowDecision::NoOverflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn infra(swap: u32) -> InfraState {
        InfraState {
            swap_pct: swap,
            cpu_pct: 50,
            memory_mb: 8000,
        }
    }

    #[test]
    fn no_overflow_when_healthy() {
        let config = OverflowConfig::default();
        assert_eq!(
            should_overflow(&infra(30), 10, 10.0, "developer", &config),
            OverflowDecision::NoOverflow
        );
    }

    #[test]
    fn no_overflow_when_low_depth() {
        let config = OverflowConfig::default();
        assert_eq!(
            should_overflow(&infra(60), 5, 10.0, "developer", &config),
            OverflowDecision::NoOverflow
        );
    }

    #[test]
    fn no_overflow_when_no_budget() {
        let config = OverflowConfig::default();
        assert_eq!(
            should_overflow(&infra(60), 40, 0.0, "developer", &config),
            OverflowDecision::NoOverflow
        );
    }

    #[test]
    fn overflow_when_all_conditions_met() {
        let config = OverflowConfig::default();
        let result = should_overflow(&infra(60), 40, 8.0, "developer", &config);
        match result {
            OverflowDecision::RouteToHetzner { role, .. } => assert_eq!(role, "developer"),
            _ => panic!("expected RouteToHetzner"),
        }
    }

    #[test]
    fn overflow_includes_reason() {
        let config = OverflowConfig::default();
        let result = should_overflow(&infra(55), 35, 5.0, "qa-lead", &config);
        match result {
            OverflowDecision::RouteToHetzner { reason, .. } => {
                assert!(reason.contains("swap=55%"));
                assert!(reason.contains("depth=35"));
            }
            _ => panic!("expected RouteToHetzner"),
        }
    }
}
