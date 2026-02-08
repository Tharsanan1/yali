pub mod config;

mod app;
mod logging;
mod policy;
mod proxy;
mod router;
mod state;
mod sync;

pub use config::GatewayDpConfig;

pub fn run(config: GatewayDpConfig) {
    app::run(config);
}
