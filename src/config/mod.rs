//! Configuration structures for the AI-Native Gateway.
//!
//! This module implements the three-tier resource model:
//! - Provider: The actual AI endpoint (OpenAI, Azure, Anthropic)
//! - Backend: Orchestration policy (load balancing, circuit breakers, retries)
//! - Route: Traffic entry point (path matching, filters)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The root gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// List of provider definitions
    pub providers: Vec<Provider>,
    /// List of backend definitions
    pub backends: Vec<Backend>,
    /// List of route definitions
    pub routes: Vec<Route>,
}

/// A Provider represents a single AI endpoint with its connection details and protocol adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Unique identifier for the provider
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Provider specification
    pub spec: ProviderSpec,
}

/// Provider specification containing endpoint and adapter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSpec {
    /// Provider type (openai, anthropic, azure_openai, google_ai, bedrock, custom)
    #[serde(rename = "type")]
    pub provider_type: String,
    /// The endpoint URL
    pub endpoint: String,
    /// Protocol adapter configuration
    #[serde(default)]
    pub adapter: AdapterConfig,
    /// Connection pool settings
    #[serde(default)]
    pub connection_pool: ConnectionPoolConfig,
}

/// Adapter configuration for protocol translation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Authentication configuration
    #[serde(default)]
    pub auth: AuthConfig,
    /// Header transformations
    #[serde(default)]
    pub headers: HeaderTransforms,
    /// URL transformations
    #[serde(default)]
    pub url: UrlTransforms,
    /// Request body transformations
    #[serde(default)]
    pub request_body: BodyTransforms,
    /// Response body transformations
    #[serde(default)]
    pub response_body: BodyTransforms,
}

/// Authentication configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Auth type (bearer, header, query_param, aws_sigv4, none)
    #[serde(rename = "type", default = "default_auth_type")]
    pub auth_type: String,
    /// Header key for header-based auth
    #[serde(default)]
    pub key: Option<String>,
    /// Secret reference (e.g., vault://openai-key, env://OPENAI_API_KEY)
    #[serde(default)]
    pub secret_ref: Option<String>,
}

fn default_auth_type() -> String {
    "none".to_string()
}

/// Header transformation configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeaderTransforms {
    /// Headers to add
    #[serde(default)]
    pub add: HashMap<String, String>,
    /// Headers to remove
    #[serde(default)]
    pub remove: Vec<String>,
}

/// URL transformation configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrlTransforms {
    /// Path prefix to add
    #[serde(default)]
    pub path_prefix: Option<String>,
    /// Path template with placeholders
    #[serde(default)]
    pub path_template: Option<String>,
    /// Query parameters to add
    #[serde(default)]
    pub query_params: HashMap<String, String>,
}

/// Body transformation configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BodyTransforms {
    /// Body format (openai_chat, anthropic, google_ai, bedrock_claude, custom)
    #[serde(default)]
    pub format: Option<String>,
    /// Field mappings from source to target
    #[serde(default)]
    pub field_mappings: HashMap<String, String>,
    /// Default values to inject
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,
    /// Fields to remove
    #[serde(default)]
    pub remove_fields: Vec<String>,
}

/// Connection pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// Maximum pool size
    #[serde(default = "default_max_pool_size")]
    pub max_size: u32,
    /// Minimum idle connections
    #[serde(default = "default_min_idle")]
    pub min_idle: u32,
    /// Max idle timeout
    #[serde(default = "default_idle_timeout")]
    pub max_idle_timeout: String,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_size: default_max_pool_size(),
            min_idle: default_min_idle(),
            max_idle_timeout: default_idle_timeout(),
        }
    }
}

fn default_max_pool_size() -> u32 {
    100
}

fn default_min_idle() -> u32 {
    10
}

fn default_idle_timeout() -> String {
    "300s".to_string()
}

/// A Backend groups Providers and applies orchestration policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backend {
    /// Unique identifier for the backend
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Backend specification
    pub spec: BackendSpec,
}

/// Backend specification containing load balancing and resilience settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSpec {
    /// Load balancing configuration
    #[serde(default)]
    pub load_balancing: LoadBalancingConfig,
    /// List of provider references
    pub providers: Vec<ProviderRef>,
    /// Timeout configuration
    #[serde(default)]
    pub timeout: TimeoutConfig,
    /// Retry configuration
    #[serde(default)]
    pub retry: RetryConfig,
    /// Circuit breaker configuration
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
    /// Health check configuration
    #[serde(default)]
    pub health_check: HealthCheckConfig,
}

/// Load balancing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// Algorithm (failover, round_robin, weighted, least_connections)
    #[serde(default = "default_lb_algorithm")]
    pub algorithm: String,
    /// Enable sticky sessions
    #[serde(default)]
    pub sticky_sessions: bool,
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            algorithm: default_lb_algorithm(),
            sticky_sessions: false,
        }
    }
}

fn default_lb_algorithm() -> String {
    "failover".to_string()
}

/// Provider reference within a backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRef {
    /// Reference to provider ID
    #[serde(rename = "ref")]
    pub provider_ref: String,
    /// Priority (lower = higher priority)
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Weight for weighted load balancing
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_priority() -> u32 {
    1
}

fn default_weight() -> u32 {
    100
}

/// Timeout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Connection timeout
    #[serde(default = "default_connect_timeout")]
    pub connect: String,
    /// Response timeout
    #[serde(default = "default_response_timeout")]
    pub response: String,
    /// Idle timeout
    #[serde(default = "default_idle")]
    pub idle: String,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect: default_connect_timeout(),
            response: default_response_timeout(),
            idle: default_idle(),
        }
    }
}

fn default_connect_timeout() -> String {
    "2s".to_string()
}

fn default_response_timeout() -> String {
    "600s".to_string()
}

fn default_idle() -> String {
    "60s".to_string()
}

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Number of retry attempts
    #[serde(default = "default_retry_attempts")]
    pub attempts: u32,
    /// Backoff configuration
    #[serde(default)]
    pub backoff: BackoffConfig,
    /// Conditions for retry (5xx, connect-failure, reset)
    #[serde(default = "default_retry_conditions")]
    pub conditions: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            attempts: default_retry_attempts(),
            backoff: BackoffConfig::default(),
            conditions: default_retry_conditions(),
        }
    }
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_retry_conditions() -> Vec<String> {
    vec![
        "5xx".to_string(),
        "connect-failure".to_string(),
        "reset".to_string(),
    ]
}

/// Backoff configuration for retries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffConfig {
    /// Initial backoff duration
    #[serde(default = "default_initial_backoff")]
    pub initial: String,
    /// Maximum backoff duration
    #[serde(default = "default_max_backoff")]
    pub max: String,
    /// Backoff multiplier
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial: default_initial_backoff(),
            max: default_max_backoff(),
            multiplier: default_multiplier(),
        }
    }
}

fn default_initial_backoff() -> String {
    "100ms".to_string()
}

fn default_max_backoff() -> String {
    "10s".to_string()
}

fn default_multiplier() -> f64 {
    2.0
}

/// Circuit breaker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Enable circuit breaker
    #[serde(default)]
    pub enabled: bool,
    /// Error threshold percentage to trip
    #[serde(default = "default_error_threshold")]
    pub error_threshold_percentage: u32,
    /// Minimum requests before tripping
    #[serde(default = "default_min_requests")]
    pub min_request_volume: u32,
    /// Sleep window before half-open
    #[serde(default = "default_sleep_window")]
    pub sleep_window: String,
    /// Requests to allow in half-open state
    #[serde(default = "default_half_open_requests")]
    pub half_open_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            error_threshold_percentage: default_error_threshold(),
            min_request_volume: default_min_requests(),
            sleep_window: default_sleep_window(),
            half_open_requests: default_half_open_requests(),
        }
    }
}

fn default_error_threshold() -> u32 {
    50
}

fn default_min_requests() -> u32 {
    20
}

fn default_sleep_window() -> String {
    "30s".to_string()
}

fn default_half_open_requests() -> u32 {
    5
}

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check type (active, passive)
    #[serde(rename = "type", default = "default_health_check_type")]
    pub check_type: String,
    /// Check interval
    #[serde(default = "default_health_interval")]
    pub interval: String,
    /// Check timeout
    #[serde(default = "default_health_timeout")]
    pub timeout: String,
    /// Health check path
    #[serde(default)]
    pub path: Option<String>,
    /// Consecutive successes to mark healthy
    #[serde(default = "default_healthy_threshold")]
    pub healthy_threshold: u32,
    /// Consecutive failures to mark unhealthy
    #[serde(default = "default_unhealthy_threshold")]
    pub unhealthy_threshold: u32,
    /// Expected HTTP statuses
    #[serde(default = "default_expected_statuses")]
    pub expected_statuses: Vec<u16>,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_type: default_health_check_type(),
            interval: default_health_interval(),
            timeout: default_health_timeout(),
            path: None,
            healthy_threshold: default_healthy_threshold(),
            unhealthy_threshold: default_unhealthy_threshold(),
            expected_statuses: default_expected_statuses(),
        }
    }
}

fn default_health_check_type() -> String {
    "passive".to_string()
}

fn default_health_interval() -> String {
    "10s".to_string()
}

fn default_health_timeout() -> String {
    "5s".to_string()
}

fn default_healthy_threshold() -> u32 {
    2
}

fn default_unhealthy_threshold() -> u32 {
    3
}

fn default_expected_statuses() -> Vec<u16> {
    vec![200, 204]
}

/// A Route defines the traffic entry point, matching logic, and policy filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Unique identifier for the route
    pub id: String,
    /// Host for tenant isolation (e.g., org-1.gateway.com)
    #[serde(default)]
    pub host: Option<String>,
    /// Route specification
    pub spec: RouteSpec,
}

/// Route specification containing match rules and backend reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSpec {
    /// Match configuration
    #[serde(rename = "match")]
    pub match_rule: MatchRule,
    /// Filter chain
    #[serde(default)]
    pub filters: Vec<FilterConfig>,
    /// Reference to backend ID
    pub backend_ref: String,
}

/// Match rule for routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRule {
    /// Path to match
    pub path: String,
    /// Match type (prefix, exact, regex)
    #[serde(rename = "type", default = "default_match_type")]
    pub match_type: String,
    /// Allowed HTTP methods
    #[serde(default = "default_methods")]
    pub methods: Vec<String>,
}

fn default_match_type() -> String {
    "prefix".to_string()
}

fn default_methods() -> Vec<String> {
    vec!["GET".to_string(), "POST".to_string()]
}

/// Filter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Filter name
    pub name: String,
    /// Filter configuration
    #[serde(default)]
    pub config: serde_json::Value,
    /// Error handling behavior
    #[serde(default = "default_on_error")]
    pub on_error: String,
}

fn default_on_error() -> String {
    "terminate".to_string()
}

impl GatewayConfig {
    /// Load configuration from a JSON file.
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: GatewayConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Validate the configuration for referential integrity.
    pub fn validate(&self) -> Result<(), String> {
        // Build provider index
        let provider_ids: std::collections::HashSet<_> =
            self.providers.iter().map(|p| &p.id).collect();

        // Build backend index
        let backend_ids: std::collections::HashSet<_> =
            self.backends.iter().map(|b| &b.id).collect();

        // Validate backend provider references
        for backend in &self.backends {
            for provider_ref in &backend.spec.providers {
                if !provider_ids.contains(&provider_ref.provider_ref) {
                    return Err(format!(
                        "Backend '{}' references unknown provider '{}'",
                        backend.id, provider_ref.provider_ref
                    ));
                }
            }
        }

        // Validate route backend references
        for route in &self.routes {
            if !backend_ids.contains(&route.spec.backend_ref) {
                return Err(format!(
                    "Route '{}' references unknown backend '{}'",
                    route.id, route.spec.backend_ref
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let json = r#"{
            "providers": [{
                "id": "provider_test",
                "name": "Test Provider",
                "spec": {
                    "type": "openai",
                    "endpoint": "http://localhost:8080"
                }
            }],
            "backends": [{
                "id": "backend_test",
                "name": "Test Backend",
                "spec": {
                    "providers": [{ "ref": "provider_test" }]
                }
            }],
            "routes": [{
                "id": "route_test",
                "spec": {
                    "match": { "path": "/v1/chat" },
                    "backend_ref": "backend_test"
                }
            }]
        }"#;

        let config: GatewayConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.backends.len(), 1);
        assert_eq!(config.routes.len(), 1);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_invalid_backend_ref() {
        let json = r#"{
            "providers": [],
            "backends": [],
            "routes": [{
                "id": "route_test",
                "spec": {
                    "match": { "path": "/v1/chat" },
                    "backend_ref": "nonexistent"
                }
            }]
        }"#;

        let config: GatewayConfig = serde_json::from_str(json).unwrap();
        assert!(config.validate().is_err());
    }
}
