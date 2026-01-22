//! Health Check Implementation
//!
//! Provides active and passive health checking for providers.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Health status of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Provider is healthy and can receive traffic.
    Healthy,
    /// Provider is unhealthy and should not receive traffic.
    Unhealthy,
    /// Health status is unknown (not yet checked).
    Unknown,
}

/// Configuration for health checks.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Type of health check (active or passive).
    pub check_type: HealthCheckType,
    /// Interval between checks (for active checks).
    pub interval: Duration,
    /// Timeout for health check requests.
    pub timeout: Duration,
    /// Path to check (for HTTP checks).
    pub path: Option<String>,
    /// Number of consecutive successes to mark healthy.
    pub healthy_threshold: u32,
    /// Number of consecutive failures to mark unhealthy.
    pub unhealthy_threshold: u32,
    /// Expected HTTP status codes.
    pub expected_statuses: Vec<u16>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_type: HealthCheckType::Passive,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            path: None,
            healthy_threshold: 2,
            unhealthy_threshold: 3,
            expected_statuses: vec![200, 204],
        }
    }
}

/// Type of health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheckType {
    /// Active checks: periodically send requests to check health.
    Active,
    /// Passive checks: infer health from regular traffic.
    Passive,
}

/// Health state for a single provider.
#[derive(Debug)]
struct ProviderHealthState {
    /// Current health status.
    status: HealthStatus,
    /// Consecutive successes.
    consecutive_successes: u32,
    /// Consecutive failures.
    consecutive_failures: u32,
    /// Last check time.
    last_check: Option<Instant>,
    /// Last status change time.
    last_status_change: Option<Instant>,
}

impl Default for ProviderHealthState {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            consecutive_successes: 0,
            consecutive_failures: 0,
            last_check: None,
            last_status_change: None,
        }
    }
}

/// Health checker for providers.
pub struct HealthChecker {
    /// Configuration.
    config: HealthCheckConfig,
    /// Health state per provider.
    states: Arc<RwLock<HashMap<String, ProviderHealthState>>>,
}

impl HealthChecker {
    /// Create a new health checker with the given configuration.
    pub fn new(config: HealthCheckConfig) -> Self {
        Self {
            config,
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a health checker with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(HealthCheckConfig::default())
    }

    /// Get the health status of a provider.
    pub fn get_status(&self, provider_id: &str) -> HealthStatus {
        self.states
            .read()
            .ok()
            .and_then(|states| states.get(provider_id).map(|s| s.status))
            .unwrap_or(HealthStatus::Unknown)
    }

    /// Check if a provider is healthy (Unknown is treated as healthy).
    pub fn is_healthy(&self, provider_id: &str) -> bool {
        match self.get_status(provider_id) {
            HealthStatus::Healthy | HealthStatus::Unknown => true,
            HealthStatus::Unhealthy => false,
        }
    }

    /// Record a successful request for passive health checking.
    pub fn record_success(&self, provider_id: &str) {
        if let Ok(mut states) = self.states.write() {
            let state = states.entry(provider_id.to_string()).or_default();

            state.consecutive_successes += 1;
            state.consecutive_failures = 0;
            state.last_check = Some(Instant::now());

            // Check if we should mark as healthy
            if state.consecutive_successes >= self.config.healthy_threshold {
                if state.status != HealthStatus::Healthy {
                    state.status = HealthStatus::Healthy;
                    state.last_status_change = Some(Instant::now());
                }
            }
        }
    }

    /// Record a failed request for passive health checking.
    pub fn record_failure(&self, provider_id: &str) {
        if let Ok(mut states) = self.states.write() {
            let state = states.entry(provider_id.to_string()).or_default();

            state.consecutive_failures += 1;
            state.consecutive_successes = 0;
            state.last_check = Some(Instant::now());

            // Check if we should mark as unhealthy
            if state.consecutive_failures >= self.config.unhealthy_threshold {
                if state.status != HealthStatus::Unhealthy {
                    state.status = HealthStatus::Unhealthy;
                    state.last_status_change = Some(Instant::now());
                }
            }
        }
    }

    /// Update health status based on an HTTP status code.
    pub fn record_status_code(&self, provider_id: &str, status_code: u16) {
        if self.config.expected_statuses.contains(&status_code) || (200..300).contains(&status_code)
        {
            self.record_success(provider_id);
        } else if status_code >= 500 {
            self.record_failure(provider_id);
        }
        // 4xx errors don't affect health (client errors)
    }

    /// Get health metrics for all providers.
    pub fn get_all_health(&self) -> HashMap<String, HealthStatus> {
        self.states
            .read()
            .ok()
            .map(|states| states.iter().map(|(k, v)| (k.clone(), v.status)).collect())
            .unwrap_or_default()
    }

    /// Reset health state for a provider.
    pub fn reset(&self, provider_id: &str) {
        if let Ok(mut states) = self.states.write() {
            states.remove(provider_id);
        }
    }

    /// Get detailed health info for a provider.
    pub fn get_health_info(&self, provider_id: &str) -> Option<HealthInfo> {
        self.states.read().ok().and_then(|states| {
            states.get(provider_id).map(|state| HealthInfo {
                status: state.status,
                consecutive_successes: state.consecutive_successes,
                consecutive_failures: state.consecutive_failures,
                last_check: state.last_check,
            })
        })
    }
}

/// Detailed health information for a provider.
#[derive(Debug, Clone)]
pub struct HealthInfo {
    /// Current health status.
    pub status: HealthStatus,
    /// Consecutive successful checks.
    pub consecutive_successes: u32,
    /// Consecutive failed checks.
    pub consecutive_failures: u32,
    /// Time of last check.
    pub last_check: Option<Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker_starts_unknown() {
        let checker = HealthChecker::with_defaults();
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Unknown);
        assert!(checker.is_healthy("provider_1")); // Unknown is treated as healthy
    }

    #[test]
    fn test_health_checker_becomes_healthy() {
        let config = HealthCheckConfig {
            healthy_threshold: 2,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_success("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Unknown);

        checker.record_success("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_checker_becomes_unhealthy() {
        let config = HealthCheckConfig {
            unhealthy_threshold: 3,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_failure("provider_1");
        assert_ne!(checker.get_status("provider_1"), HealthStatus::Unhealthy);

        checker.record_failure("provider_1");
        assert_ne!(checker.get_status("provider_1"), HealthStatus::Unhealthy);

        checker.record_failure("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Unhealthy);
        assert!(!checker.is_healthy("provider_1"));
    }

    #[test]
    fn test_health_checker_success_resets_failures() {
        let config = HealthCheckConfig {
            unhealthy_threshold: 3,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_failure("provider_1");
        checker.record_failure("provider_1");
        // 2 failures, but success resets
        checker.record_success("provider_1");
        checker.record_failure("provider_1");
        checker.record_failure("provider_1");
        // Should not be unhealthy (only 2 consecutive failures)
        assert_ne!(checker.get_status("provider_1"), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_checker_status_code() {
        let config = HealthCheckConfig {
            healthy_threshold: 1,
            unhealthy_threshold: 1,
            expected_statuses: vec![200, 201],
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_status_code("provider_1", 200);
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);

        checker.record_status_code("provider_1", 500);
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_checker_4xx_does_not_affect_health() {
        let config = HealthCheckConfig {
            healthy_threshold: 1,
            unhealthy_threshold: 1,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_success("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);

        // 4xx should not mark as unhealthy
        checker.record_status_code("provider_1", 404);
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);

        checker.record_status_code("provider_1", 400);
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_checker_get_all_health() {
        let config = HealthCheckConfig {
            healthy_threshold: 1,
            unhealthy_threshold: 1,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_success("provider_1");
        checker.record_failure("provider_2");

        let health = checker.get_all_health();
        assert_eq!(health.get("provider_1"), Some(&HealthStatus::Healthy));
        assert_eq!(health.get("provider_2"), Some(&HealthStatus::Unhealthy));
    }

    #[test]
    fn test_health_checker_reset() {
        let config = HealthCheckConfig {
            healthy_threshold: 1,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_success("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Healthy);

        checker.reset("provider_1");
        assert_eq!(checker.get_status("provider_1"), HealthStatus::Unknown);
    }

    #[test]
    fn test_health_info() {
        let config = HealthCheckConfig {
            healthy_threshold: 2,
            ..Default::default()
        };
        let checker = HealthChecker::new(config);

        checker.record_success("provider_1");
        let info = checker.get_health_info("provider_1").unwrap();
        assert_eq!(info.consecutive_successes, 1);
        assert_eq!(info.consecutive_failures, 0);
        assert!(info.last_check.is_some());
    }
}
