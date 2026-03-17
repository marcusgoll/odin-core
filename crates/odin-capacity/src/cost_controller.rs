//! Cost controller: spend tracking and budget enforcement.
//!
//! Enforces daily and per-role spending ceilings. Returns decisions on
//! whether to allow, defer, overflow, or continue spending.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Decision returned by the cost controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostDecision {
    Allow,
    Defer,
    Overflow,
    Continue,
}

/// Per-role ceiling action when budget is exceeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CeilingAction {
    Defer,
    Overflow,
    Expire,
    Continue,
}

/// Cost configuration loaded from capacity.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    pub daily_ceiling_usd: f64,
    #[serde(default)]
    pub per_role_ceiling: HashMap<String, f64>,
    #[serde(default)]
    pub on_ceiling: HashMap<String, CeilingAction>,
    #[serde(default)]
    pub overflow_budget_usd: f64,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            daily_ceiling_usd: 50.0,
            per_role_ceiling: HashMap::new(),
            on_ceiling: HashMap::new(),
            overflow_budget_usd: 10.0,
        }
    }
}

/// Summary of current spend status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendSummary {
    pub daily_spend_usd: f64,
    pub daily_ceiling_usd: f64,
    pub daily_pct: f64,
    pub per_role: HashMap<String, RoleSpend>,
}

/// Per-role spend tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleSpend {
    pub spend_usd: f64,
    pub ceiling_usd: f64,
    pub pct: f64,
}

/// Cost controller tracks spend and enforces budgets.
pub struct CostController {
    pub config: CostConfig,
    pub daily_spend: HashMap<String, f64>,
}

impl CostController {
    pub fn new(config: CostConfig) -> Self {
        Self {
            config,
            daily_spend: HashMap::new(),
        }
    }

    pub fn record_spend(&mut self, role: &str, amount: f64) {
        *self.daily_spend.entry(role.to_string()).or_insert(0.0) += amount;
    }

    pub fn can_spend(&self, role: &str, amount: f64) -> CostDecision {
        let total: f64 = self.daily_spend.values().sum();
        let role_spend = self.daily_spend.get(role).copied().unwrap_or(0.0);

        // Check daily ceiling
        if total + amount > self.config.daily_ceiling_usd {
            // Ops never capped
            if let Some(action) = self.config.on_ceiling.get(role) {
                if *action == CeilingAction::Continue {
                    return CostDecision::Continue;
                }
            }
            return self.ceiling_to_decision(role);
        }

        // Check per-role ceiling
        if let Some(&ceiling) = self.config.per_role_ceiling.get(role) {
            if role_spend + amount > ceiling {
                return self.ceiling_to_decision(role);
            }
        }

        CostDecision::Allow
    }

    pub fn spend_status(&self) -> SpendSummary {
        let daily_spend: f64 = self.daily_spend.values().sum();
        let daily_pct = if self.config.daily_ceiling_usd > 0.0 {
            (daily_spend / self.config.daily_ceiling_usd) * 100.0
        } else {
            0.0
        };

        let mut per_role = HashMap::new();
        for (role, &spend) in &self.daily_spend {
            let ceiling = self
                .config
                .per_role_ceiling
                .get(role)
                .copied()
                .unwrap_or(0.0);
            let pct = if ceiling > 0.0 {
                (spend / ceiling) * 100.0
            } else {
                0.0
            };
            per_role.insert(
                role.clone(),
                RoleSpend {
                    spend_usd: spend,
                    ceiling_usd: ceiling,
                    pct,
                },
            );
        }

        SpendSummary {
            daily_spend_usd: daily_spend,
            daily_ceiling_usd: self.config.daily_ceiling_usd,
            daily_pct,
            per_role,
        }
    }

    fn ceiling_to_decision(&self, role: &str) -> CostDecision {
        match self.config.on_ceiling.get(role) {
            Some(CeilingAction::Defer) => CostDecision::Defer,
            Some(CeilingAction::Overflow) => CostDecision::Overflow,
            Some(CeilingAction::Continue) => CostDecision::Continue,
            Some(CeilingAction::Expire) | None => CostDecision::Defer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CostConfig {
        let mut per_role = HashMap::new();
        per_role.insert("strategist".into(), 15.0);
        per_role.insert("developer".into(), 20.0);
        per_role.insert("ops".into(), 5.0);

        let mut on_ceiling = HashMap::new();
        on_ceiling.insert("strategist".into(), CeilingAction::Defer);
        on_ceiling.insert("developer".into(), CeilingAction::Overflow);
        on_ceiling.insert("ops".into(), CeilingAction::Continue);

        CostConfig {
            daily_ceiling_usd: 50.0,
            per_role_ceiling: per_role,
            on_ceiling,
            overflow_budget_usd: 10.0,
        }
    }

    #[test]
    fn allows_under_budget() {
        let cc = CostController::new(test_config());
        assert_eq!(cc.can_spend("developer", 5.0), CostDecision::Allow);
    }

    #[test]
    fn defers_strategist_at_ceiling() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("strategist", 14.0);
        assert_eq!(cc.can_spend("strategist", 2.0), CostDecision::Defer);
    }

    #[test]
    fn overflows_developer_at_ceiling() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("developer", 19.0);
        assert_eq!(cc.can_spend("developer", 2.0), CostDecision::Overflow);
    }

    #[test]
    fn ops_never_capped() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("ops", 48.0);
        assert_eq!(cc.can_spend("ops", 5.0), CostDecision::Continue);
    }

    #[test]
    fn daily_ceiling_blocks_all_except_continue() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("developer", 49.0);
        // Developer at daily ceiling -> overflow (on_ceiling config)
        assert_eq!(cc.can_spend("developer", 2.0), CostDecision::Overflow);
        // Ops at daily ceiling -> continue
        assert_eq!(cc.can_spend("ops", 2.0), CostDecision::Continue);
    }

    #[test]
    fn spend_summary_correct() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("developer", 10.0);
        cc.record_spend("ops", 3.0);
        let summary = cc.spend_status();
        assert_eq!(summary.daily_spend_usd, 13.0);
        assert!((summary.daily_pct - 26.0).abs() < 0.1);
        assert_eq!(summary.per_role["developer"].spend_usd, 10.0);
    }

    #[test]
    fn record_spend_accumulates() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("developer", 5.0);
        cc.record_spend("developer", 3.0);
        assert_eq!(*cc.daily_spend.get("developer").unwrap(), 8.0);
    }

    #[test]
    fn unknown_role_defaults_to_defer() {
        let mut cc = CostController::new(test_config());
        cc.record_spend("unknown", 49.0);
        assert_eq!(cc.can_spend("unknown", 2.0), CostDecision::Defer);
    }
}
