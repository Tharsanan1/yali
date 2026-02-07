use gateway_cp::{config::GatewayCpConfig, run};

#[tokio::main]
async fn main() {
    let config_path = std::env::var("GATEWAY_CP_CONFIG")
        .unwrap_or_else(|_| "config/control-plane.example.toml".to_string());
    let config = GatewayCpConfig::load(&config_path)
        .unwrap_or_else(|err| panic!("failed to load config: {err}"));

    run(config)
        .await
        .unwrap_or_else(|err| panic!("server error: {err}"));
}
