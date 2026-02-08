use crate::{
    config::GatewayDpConfig,
    policy::PolicyRegistry,
    proxy::GatewayProxy,
    router::RouteSnapshot,
    state::{RuntimeSnapshot, State},
};
use pingora::prelude::*;
use std::sync::Arc;
use tracing::info;

pub fn run(config: GatewayDpConfig) {
    crate::logging::init(&config.logging.level, config.logging.json);
    let state = Arc::new(State::new(RuntimeSnapshot {
        routes: RouteSnapshot::empty(),
        policies: PolicyRegistry::empty(),
    }));
    let proxy = GatewayProxy::new(state.clone());

    let mut server = Server::new(None).unwrap();
    server.bootstrap();

    let mut svc = http_proxy_service(&server.configuration, proxy);
    svc.add_tcp(&config.listener.bind);
    info!(bind = %config.listener.bind, "gateway-dp listening");

    let cp_sync =
        crate::sync::CpSync::new(config.control_plane.grpc_endpoint.clone(), state.clone());
    let bg = background_service("cp-sync", cp_sync);

    server.add_service(svc);
    server.add_service(bg);
    server.run_forever();
}
