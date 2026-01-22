//! Circuit Breaker Implementation
//!
//! The circuit breaker pattern prevents cascading failures by stopping
//! requests to a failing provider when errors exceed a threshold.
//!
//! States:
//! - Closed: Normal operation, requests flow through
//! - Open: Failures exceeded threshold, requests are rejected
//! - HalfOpen: Testing if provider has recovered

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// The state of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Normal operation - requests flow through.
    Closed,
    /// Failures exceeded threshold - requests are rejected.
    Open,
    /// Testing if the provider has recovered.
    HalfOpen,
}

/// Configuration for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Error threshold percentage to trip the circuit (0-100).
    pub error_threshold_percentage: u32,
    /// Minimum number of requests before the circuit can trip.
    pub min_request_volume: u32,
    /// Duration to stay in Open state before transitioning to HalfOpen.
    pub sleep_window: Duration,
    /// Number of requests to allow in HalfOpen state.
    pub half_open_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            error_threshold_percentage: 50,
            min_request_volume: 20,
            sleep_window: Duration::from_secs(30),
            half_open_requests: 5,
        }
    }
}

/// A circuit breaker for a single provider.
pub struct CircuitBreaker {
    /// Configuration.
    config: CircuitBreakerConfig,
    /// Current state.
    state: RwLock<CircuitBreakerState>,
    /// Total requests in current window.
    total_requests: AtomicU32,
    /// Failed requests in current window.
    failed_requests: AtomicU32,
    /// Requests allowed in half-open state.
    half_open_attempts: AtomicU32,
    /// Successful requests in half-open state.
    half_open_successes: AtomicU32,
    /// Timestamp when circuit opened.
    opened_at: RwLock<Option<Instant>>,
    /// Last state transition timestamp (as unix millis for atomic access).
    last_transition: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitBreakerState::Closed),
            total_requests: AtomicU32::new(0),
            failed_requests: AtomicU32::new(0),
            half_open_attempts: AtomicU32::new(0),
            half_open_successes: AtomicU32::new(0),
            opened_at: RwLock::new(None),
            last_transition: AtomicU64::new(0),
        }
    }

    /// Create a circuit breaker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitBreakerState {
        // Check if we should transition from Open to HalfOpen
        if let Ok(state) = self.state.read() {
            if *state == CircuitBreakerState::Open {
                if let Ok(opened_at) = self.opened_at.read() {
                    if let Some(opened) = *opened_at {
                        if opened.elapsed() >= self.config.sleep_window {
                            drop(opened_at);
                            drop(state);
                            self.transition_to_half_open();
                            return CircuitBreakerState::HalfOpen;
                        }
                    }
                }
            }
            return *state;
        }
        CircuitBreakerState::Closed
    }

    /// Check if a request should be allowed through.
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => false,
            CircuitBreakerState::HalfOpen => {
                let attempts = self.half_open_attempts.fetch_add(1, Ordering::SeqCst);
                attempts < self.config.half_open_requests
            }
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::SeqCst);

        match self.state() {
            CircuitBreakerState::HalfOpen => {
                let successes = self.half_open_successes.fetch_add(1, Ordering::SeqCst) + 1;
                // If all half-open requests succeeded, close the circuit
                if successes >= self.config.half_open_requests {
                    self.transition_to_closed();
                }
            }
            CircuitBreakerState::Closed => {
                // Reset counters periodically if error rate is low
                self.maybe_reset_counters();
            }
            CircuitBreakerState::Open => {
                // Shouldn't happen, but ignore
            }
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::SeqCst);
        self.failed_requests.fetch_add(1, Ordering::SeqCst);

        match self.state() {
            CircuitBreakerState::Closed => {
                self.maybe_trip();
            }
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open state opens the circuit again
                self.transition_to_open();
            }
            CircuitBreakerState::Open => {
                // Already open
            }
        }
    }

    /// Check if the circuit should trip based on current metrics.
    fn maybe_trip(&self) {
        let total = self.total_requests.load(Ordering::SeqCst);
        let failed = self.failed_requests.load(Ordering::SeqCst);

        if total < self.config.min_request_volume {
            return;
        }

        let error_rate = (failed as f64 / total as f64) * 100.0;
        if error_rate >= self.config.error_threshold_percentage as f64 {
            self.transition_to_open();
        }
    }

    /// Reset counters if conditions are met (e.g., low error rate).
    fn maybe_reset_counters(&self) {
        let total = self.total_requests.load(Ordering::SeqCst);
        if total >= self.config.min_request_volume * 2 {
            let failed = self.failed_requests.load(Ordering::SeqCst);
            let error_rate = (failed as f64 / total as f64) * 100.0;

            // If error rate is well below threshold, reset counters
            if error_rate < (self.config.error_threshold_percentage as f64 / 2.0) {
                self.total_requests.store(0, Ordering::SeqCst);
                self.failed_requests.store(0, Ordering::SeqCst);
            }
        }
    }

    fn transition_to_open(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitBreakerState::Open;
            if let Ok(mut opened_at) = self.opened_at.write() {
                *opened_at = Some(Instant::now());
            }
            self.last_transition.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                Ordering::SeqCst,
            );
        }
    }

    fn transition_to_half_open(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitBreakerState::HalfOpen;
            self.half_open_attempts.store(0, Ordering::SeqCst);
            self.half_open_successes.store(0, Ordering::SeqCst);
            self.last_transition.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                Ordering::SeqCst,
            );
        }
    }

    fn transition_to_closed(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = CircuitBreakerState::Closed;
            self.total_requests.store(0, Ordering::SeqCst);
            self.failed_requests.store(0, Ordering::SeqCst);
            if let Ok(mut opened_at) = self.opened_at.write() {
                *opened_at = None;
            }
            self.last_transition.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                Ordering::SeqCst,
            );
        }
    }

    /// Get metrics for the circuit breaker.
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.state(),
            total_requests: self.total_requests.load(Ordering::SeqCst),
            failed_requests: self.failed_requests.load(Ordering::SeqCst),
            error_rate: {
                let total = self.total_requests.load(Ordering::SeqCst);
                let failed = self.failed_requests.load(Ordering::SeqCst);
                if total > 0 {
                    (failed as f64 / total as f64) * 100.0
                } else {
                    0.0
                }
            },
        }
    }
}

/// Metrics from the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    /// Current state.
    pub state: CircuitBreakerState,
    /// Total requests in current window.
    pub total_requests: u32,
    /// Failed requests in current window.
    pub failed_requests: u32,
    /// Current error rate percentage.
    pub error_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_trips_on_high_error_rate() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 10,
            sleep_window: Duration::from_secs(30),
            half_open_requests: 5,
        };
        let cb = CircuitBreaker::new(config);

        // Record 5 successes and 5 failures (50% error rate)
        for _ in 0..5 {
            cb.record_success();
        }
        for _ in 0..5 {
            cb.record_failure();
        }

        // Should be open now
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_stays_closed_below_threshold() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 10,
            sleep_window: Duration::from_secs(30),
            half_open_requests: 5,
        };
        let cb = CircuitBreaker::new(config);

        // Record 8 successes and 2 failures (20% error rate)
        for _ in 0..8 {
            cb.record_success();
        }
        for _ in 0..2 {
            cb.record_failure();
        }

        // Should still be closed
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_requires_min_volume() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 10,
            sleep_window: Duration::from_secs(30),
            half_open_requests: 5,
        };
        let cb = CircuitBreaker::new(config);

        // Record 5 failures (100% error rate, but below min volume)
        for _ in 0..5 {
            cb.record_failure();
        }

        // Should still be closed (not enough volume)
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_circuit_breaker_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 4,
            sleep_window: Duration::from_millis(10), // Very short for testing
            half_open_requests: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Trip the circuit
        for _ in 0..4 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        // Wait for sleep window
        std::thread::sleep(Duration::from_millis(20));

        // Should be half-open now
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_breaker_closes_after_half_open_success() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 4,
            sleep_window: Duration::from_millis(10),
            half_open_requests: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Trip the circuit
        for _ in 0..4 {
            cb.record_failure();
        }

        // Wait for half-open
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        // Record successful half-open requests
        cb.record_success();
        cb.record_success();

        // Should be closed now
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_circuit_breaker_reopens_on_half_open_failure() {
        let config = CircuitBreakerConfig {
            error_threshold_percentage: 50,
            min_request_volume: 4,
            sleep_window: Duration::from_millis(10),
            half_open_requests: 2,
        };
        let cb = CircuitBreaker::new(config);

        // Trip the circuit
        for _ in 0..4 {
            cb.record_failure();
        }

        // Wait for half-open
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        // Record a failure in half-open
        cb.record_failure();

        // Should be open again
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_circuit_breaker_metrics() {
        let cb = CircuitBreaker::with_defaults();

        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let metrics = cb.metrics();
        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.failed_requests, 1);
        assert!((metrics.error_rate - 33.33).abs() < 1.0);
    }
}
