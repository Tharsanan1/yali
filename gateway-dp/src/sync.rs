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

use crate::policy::types::{PolicyArtifact, PolicyBinding, PolicyKey, PolicyStage};
use crate::policy::PolicyRegistry;
use crate::router::{Route, RouteSnapshot, Upstream};
use crate::state::{RuntimeSnapshot, State};

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
            let new_snapshot = snapshot_to_runtime(snapshot).await?;
            debug!(
                routes = new_snapshot.routes.routes.len(),
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

async fn snapshot_to_runtime(
    snapshot: Snapshot,
) -> Result<RuntimeSnapshot, Box<dyn std::error::Error + Send + Sync>> {
    let artifacts = snapshot
        .policy_artifacts
        .into_iter()
        .map(|artifact| PolicyArtifact {
            key: PolicyKey::new(artifact.id, artifact.version),
            wasm_uri: artifact.wasm_uri,
            sha256: artifact.sha256,
        })
        .collect::<Vec<_>>();
    let policies = PolicyRegistry::preload(&artifacts).await?;

    let routes = snapshot
        .routes
        .into_iter()
        .map(
            |route| -> Result<Route, Box<dyn std::error::Error + Send + Sync>> {
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

                let route_policies = route
                    .policies
                    .into_iter()
                    .map(|policy| {
                        let stage = policy
                            .stage
                            .parse::<PolicyStage>()
                            .map_err(|err| format!("invalid stage {}: {err}", policy.stage))?;
                        let effective_config = serde_json::from_str(&policy.effective_config_json)
                            .map_err(|err| format!("invalid effective_config_json: {err}"))?;
                        Ok::<PolicyBinding, String>(PolicyBinding {
                            stage,
                            key: PolicyKey::new(policy.id, policy.version),
                            effective_config,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> { err.into() })?;

                Ok(Route::new(
                    route.id,
                    path_prefix,
                    methods,
                    host,
                    upstreams,
                    route_policies,
                ))
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RuntimeSnapshot {
        routes: RouteSnapshot { routes },
        policies,
    })
}
