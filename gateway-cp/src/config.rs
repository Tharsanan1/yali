use figment::{providers::Env, providers::Format, providers::Toml, Figment};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GatewayCpConfig {
    pub bind: String,
    pub grpc_bind: String,
    pub logging: LoggingConfig,
    pub database_url: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json: bool,
}

impl GatewayCpConfig {
    pub fn load(path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("GATEWAY_CP__").split("__"))
            .extract()
    }
}
