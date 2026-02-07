use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicySpec {
    pub id: String,
    pub version: String,
    pub wasm_uri: String,
    pub sha256: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSpec {
    pub id: String,
    #[serde(rename = "match")]
    pub match_rules: serde_json::Value,
    pub upstreams: Vec<Upstream>,
    #[serde(default)]
    pub lb: Option<String>,
    #[serde(default)]
    pub failover: Option<Failover>,
    #[serde(default)]
    pub policies: Vec<RoutePolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub url: String,
    #[serde(default)]
    pub weight: Option<u32>,
    #[serde(default)]
    pub priority: Option<u32>,
    #[serde(default)]
    pub tls: Option<TlsOverride>,
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
    #[serde(default)]
    pub outlier_detection: Option<OutlierDetection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsOverride {
    pub server_name: Option<String>,
    pub ca_cert_path: Option<String>,
    pub insecure_skip_verify: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub path: String,
    pub interval_ms: u64,
    pub timeout_ms: u64,
    pub unhealthy_threshold: u32,
    pub healthy_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierDetection {
    pub consecutive_5xx: u32,
    pub eject_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failover {
    pub enabled: bool,
    #[serde(default)]
    pub max_failovers: Option<u32>,
    #[serde(default)]
    pub retry_on: Option<Vec<String>>, // e.g. connect_failure, 5xx, timeout
    #[serde(default)]
    pub per_try_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePolicy {
    pub stage: String, // pre_route | pre_upstream | post_response
    pub id: String,
    pub version: String,
}
