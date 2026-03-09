//! Infrastructure-aware agent capacity calculator.
//!
//! Determines max agents based on system resource pressure (swap, CPU)
//! and computes target agent count from queue depth.

use serde::{Deserialize, Serialize};

/// Current infrastructure state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraState {
    pub swap_pct: u32,
    pub cpu_pct: u32,
    pub memory_mb: u32,
}

/// Capacity configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityConfig {
    pub min_agents: u32,
    pub max_agents: u32,
    pub tasks_per_agent: u32,
}

impl Default for CapacityConfig {
    fn default() -> Self {
        Self {
            min_agents: 2,
            max_agents: 16,
            tasks_per_agent: 3,
        }
    }
}

/// Returns max agents allowed given current infrastructure pressure.
pub fn max_agents(infra: &InfraState) -> u32 {
    if infra.swap_pct > 60 {
        4
    } else if infra.swap_pct > 40 {
        8
    } else if infra.swap_pct <= 40 && infra.cpu_pct < 70 {
        if infra.swap_pct <= 20 && infra.cpu_pct < 50 {
            16
        } else {
            12
        }
    } else {
        8
    }
}

/// Computes target agent count from active queue depth and config.
pub fn compute_target(active_depth: u32, config: &CapacityConfig, infra: &InfraState) -> u32 {
    let infra_max = max_agents(infra).min(config.max_agents);
    let raw = if config.tasks_per_agent > 0 {
        (active_depth + config.tasks_per_agent - 1) / config.tasks_per_agent
    } else {
        active_depth
    };
    raw.clamp(config.min_agents, infra_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_swap_limits_agents() {
        let infra = InfraState {
            swap_pct: 65,
            cpu_pct: 30,
            memory_mb: 8000,
        };
        assert_eq!(max_agents(&infra), 4);
    }

    #[test]
    fn moderate_swap_limits_agents() {
        let infra = InfraState {
            swap_pct: 45,
            cpu_pct: 30,
            memory_mb: 8000,
        };
        assert_eq!(max_agents(&infra), 8);
    }

    #[test]
    fn low_pressure_allows_max() {
        let infra = InfraState {
            swap_pct: 15,
            cpu_pct: 40,
            memory_mb: 16000,
        };
        assert_eq!(max_agents(&infra), 16);
    }

    #[test]
    fn compute_target_clamps_to_min() {
        let config = CapacityConfig::default();
        let infra = InfraState {
            swap_pct: 10,
            cpu_pct: 20,
            memory_mb: 16000,
        };
        assert_eq!(compute_target(0, &config, &infra), 2);
    }

    #[test]
    fn compute_target_scales_with_depth() {
        let config = CapacityConfig::default();
        let infra = InfraState {
            swap_pct: 10,
            cpu_pct: 20,
            memory_mb: 16000,
        };
        // 9 tasks / 3 per agent = 3
        assert_eq!(compute_target(9, &config, &infra), 3);
        // 15 tasks / 3 = 5
        assert_eq!(compute_target(15, &config, &infra), 5);
    }

    #[test]
    fn compute_target_clamps_to_infra_max() {
        let config = CapacityConfig::default();
        let infra = InfraState {
            swap_pct: 65,
            cpu_pct: 80,
            memory_mb: 4000,
        };
        // Would want 20 agents but infra caps at 4
        assert_eq!(compute_target(60, &config, &infra), 4);
    }
}
