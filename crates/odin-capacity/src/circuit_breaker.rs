//! Per-role circuit breaker for agent spawning.
//!
//! Tracks failure rates in a rolling window and gates spawning when a role
//! exceeds configured thresholds.
//!
//! States: Closed -> Caution -> HalfOpen -> Open

use serde::{Deserialize, Serialize};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Closed,
    Caution,
    HalfOpen,
    Open,
}

/// Event types recorded by the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Spawn,
    Failure,
    Success,
}

/// A timestamped circuit breaker event.
#[derive(Debug, Clone)]
pub struct Event {
    pub timestamp: i64,
    pub event_type: EventType,
}

/// Configuration for circuit breaker thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub caution_threshold: f64,
    pub half_open_threshold: f64,
    pub open_threshold: f64,
    pub window_minutes: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            caution_threshold: 0.20,
            half_open_threshold: 0.50,
            open_threshold: 0.80,
            window_minutes: 60,
        }
    }
}

/// Per-role circuit breaker.
pub struct CircuitBreaker {
    pub role: String,
    pub events: Vec<Event>,
    pub config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(role: String, config: CircuitBreakerConfig) -> Self {
        Self {
            role,
            events: Vec::new(),
            config,
        }
    }

    pub fn record_event(&mut self, event_type: EventType, timestamp: i64) {
        self.events.push(Event {
            timestamp,
            event_type,
        });
    }

    pub fn get_state(&self, now: i64) -> State {
        let window = (self.config.window_minutes * 60) as i64;
        let cutoff = now - window;

        let mut total = 0u32;
        let mut failures = 0u32;

        for event in &self.events {
            if event.timestamp < cutoff {
                continue;
            }
            total += 1;
            if event.event_type == EventType::Failure {
                failures += 1;
            }
        }

        if total == 0 {
            return State::Closed;
        }

        let rate = failures as f64 / total as f64;

        if rate >= self.config.open_threshold {
            State::Open
        } else if rate >= self.config.half_open_threshold {
            State::HalfOpen
        } else if rate >= self.config.caution_threshold {
            State::Caution
        } else {
            State::Closed
        }
    }

    pub fn can_spawn(&self, now: i64) -> bool {
        self.get_state(now) != State::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> CircuitBreakerConfig {
        CircuitBreakerConfig::default()
    }

    #[test]
    fn empty_is_closed() {
        let cb = CircuitBreaker::new("dev".into(), config());
        assert_eq!(cb.get_state(1000), State::Closed);
        assert!(cb.can_spawn(1000));
    }

    #[test]
    fn low_failure_rate_is_closed() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        for i in 0..10 {
            cb.record_event(EventType::Spawn, i * 60);
            cb.record_event(EventType::Success, i * 60 + 30);
        }
        cb.record_event(EventType::Failure, 600);
        // 1 failure out of 21 events = ~5%
        assert_eq!(cb.get_state(700), State::Closed);
    }

    #[test]
    fn caution_at_25_percent() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        for i in 0..3 {
            cb.record_event(EventType::Spawn, i * 60);
        }
        cb.record_event(EventType::Failure, 200);
        // 1 failure out of 4 = 25%
        assert_eq!(cb.get_state(300), State::Caution);
    }

    #[test]
    fn half_open_at_50_percent() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        cb.record_event(EventType::Spawn, 100);
        cb.record_event(EventType::Failure, 200);
        // 1 failure out of 2 = 50%
        assert_eq!(cb.get_state(300), State::HalfOpen);
    }

    #[test]
    fn open_at_80_percent() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        for i in 0..5 {
            cb.record_event(EventType::Failure, i * 60);
        }
        cb.record_event(EventType::Spawn, 350);
        // 5 failures out of 6 = 83%
        assert_eq!(cb.get_state(400), State::Open);
        assert!(!cb.can_spawn(400));
    }

    #[test]
    fn old_events_outside_window() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        // Events from 2 hours ago (outside 1hr window)
        for i in 0..5 {
            cb.record_event(EventType::Failure, i * 60);
        }
        // Now is 2 hours later
        let now = 7200;
        assert_eq!(cb.get_state(now), State::Closed);
    }

    #[test]
    fn recovery_after_successes() {
        let mut cb = CircuitBreaker::new("dev".into(), config());
        // Start with failures
        for i in 0..4 {
            cb.record_event(EventType::Failure, i * 60);
        }
        assert_eq!(cb.get_state(300), State::Open);

        // Add many successes
        for i in 5..25 {
            cb.record_event(EventType::Success, i * 60);
        }
        // 4 failures out of 24 = 16.7% -> Closed
        assert_eq!(cb.get_state(1500), State::Closed);
    }

    #[test]
    fn independent_roles() {
        let mut dev = CircuitBreaker::new("developer".into(), config());
        let ops = CircuitBreaker::new("ops".into(), config());

        for i in 0..5 {
            dev.record_event(EventType::Failure, i * 60);
        }
        assert_eq!(dev.get_state(400), State::Open);
        assert_eq!(ops.get_state(400), State::Closed);
    }
}
