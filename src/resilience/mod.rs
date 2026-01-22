//! Resilience Module
//!
//! Provides resilience features for the gateway:
//! - Circuit Breaker: Prevent cascading failures by stopping requests to failing providers
//! - Retry with Backoff: Automatic retry with exponential backoff
//! - Health Checks: Active and passive health monitoring
//! - Load Balancing: Multiple algorithms for distributing traffic

mod circuit_breaker;
mod health_check;
mod load_balancer;
mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitBreakerState};
pub use health_check::{HealthChecker, HealthStatus};
pub use load_balancer::{LoadBalancer, LoadBalancerAlgorithm};
pub use retry::{RetryPolicy, RetryResult};
