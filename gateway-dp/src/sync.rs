use async_trait::async_trait;
use gateway_proto::config::config_service_client::ConfigServiceClient;
use gateway_proto::config::{Snapshot, SubscribeRequest};
use pingora::prelude::*;
use pingora::server::ShutdownWatch;
use pingora::services::background::BackgroundService;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::router::{Route, RouteSnapshot, Upstream};
use crate::state::State;

pub struct CpSync {
    endpoint: String,
    state: Arc<State>,
}

impl CpSync {
    pub fn new(endpoint: String, state: Arc<State>) -> Self {
        Self { endpoint, state }
    }

    async fn run_once(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(endpoint = %self.endpoint, "connecting to control plane");
        let mut client = ConfigServiceClient::connect(self.endpoint.clone()).await?;
        let mut stream = client
            .subscribe(SubscribeRequest { last_version: 0 })
            .await?
            .into_inner();

        while let Some(snapshot) = stream.message().await? {
            let route_count = snapshot.routes.len();
            debug!(
                version = snapshot.version,
                routes = route_count,
                "received config snapshot"
            );
            let new_snapshot = snapshot_to_routes(snapshot);
            debug!(
                routes = new_snapshot.routes.len(),
                "applying config snapshot"
            );
            self.state.update(new_snapshot);
        }

        Ok(())
    }
}

#[async_trait]
impl BackgroundService for CpSync {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => break,
                result = self.run_once() => {
                    if let Err(err) = result {
                        warn!(error = %err, "cp sync error");
                    }
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    }
}

fn snapshot_to_routes(snapshot: Snapshot) -> RouteSnapshot {
    let routes = snapshot
        .routes
        .into_iter()
        .map(|route| {
            let path_prefix = route.r#match.as_ref().and_then(|m| {
                if m.path_prefix.is_empty() {
                    None
                } else {
                    Some(m.path_prefix.clone())
                }
            });
            let methods = route
                .r#match
                .as_ref()
                .map(|m| m.methods.clone())
                .unwrap_or_default();
            let host = route.r#match.as_ref().and_then(|m| {
                if m.host.is_empty() {
                    None
                } else {
                    Some(m.host.clone())
                }
            });

            let upstreams = route
                .upstreams
                .into_iter()
                .map(|u| Upstream { url: u.url })
                .collect();

            Route::new(route.id, path_prefix, methods, host, upstreams)
        })
        .collect();

    RouteSnapshot { routes }
}
