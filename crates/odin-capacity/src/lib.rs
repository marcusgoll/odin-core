//! odin-capacity: Adaptive scheduling engine for the Odin orchestrator.
//!
//! Called as a subprocess by the bash orchestrator. Reads queue/agent/infra state,
//! returns JSON scheduling decisions.
//!
//! # Architecture
//!
//! The scheduler composes several sub-modules:
//! - **classifier**: Scores tasks by priority, cost tier, and urgency decay
//! - **circuit_breaker**: Per-role health tracking with rolling failure windows
//! - **cost_controller**: Spend tracking and budget enforcement
//! - **capacity**: Infrastructure-aware agent scaling
//! - **overflow**: Hetzner burst routing when local infra is saturated
//! - **scheduler**: Top-level orchestrator composing all sub-modules
//! - **metrics**: Observability export (JSONL)

pub mod capacity;
pub mod circuit_breaker;
pub mod classifier;
pub mod cost_controller;
pub mod metrics;
pub mod overflow;
pub mod scheduler;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CapacityError {
    #[error("config load failed: {0}")]
    Config(String),
    #[error("state read failed: {0}")]
    State(String),
    #[error("schedule failed: {0}")]
    Schedule(String),
}

pub type CapacityResult<T> = Result<T, CapacityError>;
