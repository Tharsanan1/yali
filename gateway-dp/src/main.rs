use gateway_dp::GatewayDpConfig;

fn main() {
    let config_path = std::env::var("GATEWAY_DP_CONFIG").unwrap_or_else(|_| "config/gateway.example.toml".to_string());
    let config = GatewayDpConfig::load(&config_path)
        .unwrap_or_else(|err| panic!("failed to load config: {err}"));

    println!("gateway-dp boot config loaded: {config:?}");

    gateway_dp::run(config);
}
