//! Adaptive scheduler — top-level orchestrator composing all sub-modules.
//!
//! Called by the bash orchestrator as a subprocess. Reads queue/agent/infra/cost
//! state and returns JSON scheduling decisions (spawn, drain, defer, overflow).

use serde::{Deserialize, Serialize};

use crate::capacity::{self, CapacityConfig, InfraState};
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, State as CbState};
use crate::cost_controller::{CostController, CostDecision};
use crate::metrics::MetricsExport;
use crate::overflow::{self, OverflowConfig, OverflowDecision};

/// Input to the scheduler (assembled from filesystem state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleInput {
    pub queue_depths: std::collections::HashMap<String, u32>,
    pub held_depth: u32,
    pub deferred_depth: u32,
    pub agent_count: u32,
    pub infra: InfraState,
    pub spend_today_usd: f64,
    pub spend_ceiling_usd: f64,
}

/// A spawn directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnDirective {
    pub role: String,
    pub target: String,
    pub reason: String,
}

/// A drain directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainDirective {
    pub agent_id: String,
    pub reason: String,
}

/// Full scheduling decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleDecision {
    pub spawn: Vec<SpawnDirective>,
    pub drain: Vec<DrainDirective>,
    pub overflow: Vec<SpawnDirective>,
    pub metrics: MetricsExport,
}

/// The scheduler.
pub struct Scheduler {
    pub capacity_config: CapacityConfig,
    pub cb_config: CircuitBreakerConfig,
    pub overflow_config: OverflowConfig,
    pub role_priority: Vec<String>,
}

impl Scheduler {
    pub fn schedule(
        &self,
        input: &ScheduleInput,
        circuit_breakers: &[CircuitBreaker],
        cost_controller: &CostController,
        now: i64,
    ) -> ScheduleDecision {
        let active_depth: u32 = input.queue_depths.values().sum();
        let target = capacity::compute_target(active_depth, &self.capacity_config, &input.infra);

        let mut spawn = Vec::new();
        let mut overflow_directives = Vec::new();

        if target > input.agent_count {
            // Scale up: pick role with most work that isn't circuit-broken
            if let Some(role) = self.next_role_to_spawn(&input.queue_depths, circuit_breakers, now)
            {
                // Check cost
                let cost_decision = cost_controller.can_spend(&role, 1.0);
                match cost_decision {
                    CostDecision::Allow | CostDecision::Continue => {
                        spawn.push(SpawnDirective {
                            role: role.clone(),
                            target: "local".into(),
                            reason: format!(
                                "scale_up target={} current={}",
                                target, input.agent_count
                            ),
                        });
                    }
                    CostDecision::Overflow => {
                        let budget_remaining =
                            self.overflow_config.budget_usd - input.spend_today_usd;
                        let overflow_decision = overflow::should_overflow(
                            &input.infra,
                            active_depth,
                            budget_remaining,
                            &role,
                            &self.overflow_config,
                        );
                        if let OverflowDecision::RouteToHetzner { role, reason } = overflow_decision
                        {
                            overflow_directives.push(SpawnDirective {
                                role,
                                target: "hetzner".into(),
                                reason,
                            });
                        }
                    }
                    CostDecision::Defer => {} // Skip this cycle
                }
            }
        }

        let drain = if target < input.agent_count {
            let drain_count = 1.min(input.agent_count - target);
            vec![
                DrainDirective {
                    agent_id: String::new(),
                    reason: format!("scale_down target={} current={}", target, input.agent_count),
                };
                drain_count as usize
            ]
        } else {
            Vec::new()
        };

        let health = if !spawn.is_empty() {
            "scaling_up"
        } else if !drain.is_empty() {
            "scaling_down"
        } else {
            "steady"
        };

        let metrics = MetricsExport {
            active_lane_depth: active_depth,
            held_lane_depth: input.held_depth,
            deferred_lane_depth: input.deferred_depth,
            agent_count: input.agent_count,
            target_agent_count: target,
            spend_today_usd: input.spend_today_usd,
            spend_ceiling_usd: input.spend_ceiling_usd,
            throughput_tasks_per_hour: 0.0,
            health: health.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        ScheduleDecision {
            spawn,
            drain,
            overflow: overflow_directives,
            metrics,
        }
    }

    fn next_role_to_spawn(
        &self,
        depths: &std::collections::HashMap<String, u32>,
        circuit_breakers: &[CircuitBreaker],
        now: i64,
    ) -> Option<String> {
        // Priority order, pick first with work that isn't circuit-broken
        for role in &self.role_priority {
            let depth = depths.get(role).copied().unwrap_or(0);
            if depth == 0 {
                continue;
            }
            let is_open = circuit_breakers
                .iter()
                .any(|cb| cb.role == *role && cb.get_state(now) == CbState::Open);
            if !is_open {
                return Some(role.clone());
            }
        }
        // Fallback: any role with work
        depths
            .iter()
            .filter(|(_, &d)| d > 0)
            .max_by_key(|(_, &d)| d)
            .map(|(r, _)| r.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost_controller::CostConfig;
    use std::collections::HashMap;

    fn test_scheduler() -> Scheduler {
        Scheduler {
            capacity_config: CapacityConfig::default(),
            cb_config: CircuitBreakerConfig::default(),
            overflow_config: OverflowConfig::default(),
            role_priority: vec![
                "ops".into(),
                "developer".into(),
                "qa-lead".into(),
                "strategist".into(),
            ],
        }
    }

    fn healthy_infra() -> InfraState {
        InfraState {
            swap_pct: 10,
            cpu_pct: 20,
            memory_mb: 16000,
        }
    }

    #[test]
    fn scale_up_when_tasks_exceed_agents() {
        let scheduler = test_scheduler();
        let cost = CostController::new(CostConfig::default());
        let mut depths = HashMap::new();
        depths.insert("developer".into(), 9);
        let input = ScheduleInput {
            queue_depths: depths,
            held_depth: 0,
            deferred_depth: 0,
            agent_count: 1,
            infra: healthy_infra(),
            spend_today_usd: 5.0,
            spend_ceiling_usd: 50.0,
        };
        let decision = scheduler.schedule(&input, &[], &cost, 1000);
        assert!(!decision.spawn.is_empty());
        assert_eq!(decision.spawn[0].role, "developer");
        assert_eq!(decision.metrics.health, "scaling_up");
    }

    #[test]
    fn scale_down_when_agents_exceed_target() {
        let scheduler = test_scheduler();
        let cost = CostController::new(CostConfig::default());
        let input = ScheduleInput {
            queue_depths: HashMap::new(),
            held_depth: 0,
            deferred_depth: 0,
            agent_count: 5,
            infra: healthy_infra(),
            spend_today_usd: 5.0,
            spend_ceiling_usd: 50.0,
        };
        let decision = scheduler.schedule(&input, &[], &cost, 1000);
        assert!(decision.spawn.is_empty());
        assert!(!decision.drain.is_empty());
        assert_eq!(decision.metrics.health, "scaling_down");
    }

    #[test]
    fn steady_when_balanced() {
        let scheduler = test_scheduler();
        let cost = CostController::new(CostConfig::default());
        let mut depths = HashMap::new();
        depths.insert("developer".into(), 6);
        let input = ScheduleInput {
            queue_depths: depths,
            held_depth: 0,
            deferred_depth: 0,
            agent_count: 2,
            infra: healthy_infra(),
            spend_today_usd: 5.0,
            spend_ceiling_usd: 50.0,
        };
        let decision = scheduler.schedule(&input, &[], &cost, 1000);
        assert!(decision.spawn.is_empty());
        assert!(decision.drain.is_empty());
        assert_eq!(decision.metrics.health, "steady");
    }

    #[test]
    fn respects_role_priority() {
        let scheduler = test_scheduler();
        let cost = CostController::new(CostConfig::default());
        let mut depths = HashMap::new();
        depths.insert("developer".into(), 5);
        depths.insert("ops".into(), 1);
        let input = ScheduleInput {
            queue_depths: depths,
            held_depth: 0,
            deferred_depth: 0,
            agent_count: 1,
            infra: healthy_infra(),
            spend_today_usd: 0.0,
            spend_ceiling_usd: 50.0,
        };
        let decision = scheduler.schedule(&input, &[], &cost, 1000);
        // Ops has priority even with fewer tasks
        assert_eq!(decision.spawn[0].role, "ops");
    }

    #[test]
    fn metrics_populated() {
        let scheduler = test_scheduler();
        let cost = CostController::new(CostConfig::default());
        let input = ScheduleInput {
            queue_depths: HashMap::new(),
            held_depth: 2,
            deferred_depth: 1,
            agent_count: 3,
            infra: healthy_infra(),
            spend_today_usd: 10.0,
            spend_ceiling_usd: 50.0,
        };
        let decision = scheduler.schedule(&input, &[], &cost, 1000);
        assert_eq!(decision.metrics.held_lane_depth, 2);
        assert_eq!(decision.metrics.deferred_lane_depth, 1);
        assert_eq!(decision.metrics.spend_today_usd, 10.0);
    }
}
