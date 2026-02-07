use figment::{providers::Env, providers::Format, providers::Toml, Figment};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GatewayDpConfig {
    pub listener: ListenerConfig,
    pub control_plane: ControlPlaneConfig,
    pub logging: LoggingConfig,
    pub limits: LimitsConfig,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ListenerConfig {
    pub bind: String,
    pub tls: Option<TlsConfig>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ControlPlaneConfig {
    pub grpc_endpoint: String,
    pub tls: Option<TlsConfig>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json: bool,
    pub rolling_file: Option<RollingFileConfig>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RollingFileConfig {
    pub directory: String,
    pub prefix: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LimitsConfig {
    pub max_body_bytes: u64,
    pub pre_upstream_body_bytes: u64,
}

impl GatewayDpConfig {
    pub fn load(path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("GATEWAY_DP__").split("__"))
            .extract()
    }
}
