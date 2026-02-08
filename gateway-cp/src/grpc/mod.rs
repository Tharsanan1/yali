use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_core::Stream;
use gateway_proto::config::{
    config_service_server::{ConfigService, ConfigServiceServer},
    Match, PolicyArtifact, PolicyBinding, Route, Snapshot, SubscribeRequest, Upstream,
};
use sqlx::SqlitePool;
use tokio::sync::watch;
use tokio_stream::{wrappers::WatchStream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::model::PolicySpec;
use crate::model::{RoutePolicy, RouteSpec, Upstream as ModelUpstream};
use crate::service::merge::deep_merge_default_with_params;

#[derive(Clone)]
pub struct ConfigState {
    version: Arc<AtomicU64>,
    tx: watch::Sender<Snapshot>,
}

impl ConfigState {
    pub fn new() -> Self {
        let snapshot = Snapshot {
            version: 0,
            routes: Vec::new(),
            policy_artifacts: Vec::new(),
        };
        let (tx, _) = watch::channel(snapshot);
        Self {
            version: Arc::new(AtomicU64::new(0)),
            tx,
        }
    }

    pub fn server(self: &Arc<Self>) -> ConfigServiceServer<ConfigServiceImpl> {
        ConfigServiceServer::new(ConfigServiceImpl {
            state: self.clone(),
        })
    }

    pub async fn publish_from_db(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let routes = crate::db::list_routes(pool).await?;
        let snapshot = self.build_snapshot(pool, routes).await?;
        debug!(
            version = snapshot.version,
            routes = snapshot.routes.len(),
            "published config snapshot"
        );
        let _ = self.tx.send(snapshot);
        Ok(())
    }

    async fn build_snapshot(
        &self,
        pool: &SqlitePool,
        routes: Vec<RouteSpec>,
    ) -> Result<Snapshot, sqlx::Error> {
        let policies = load_policies_for_routes(pool, &routes).await?;
        let artifacts = policies
            .values()
            .cloned()
            .map(policy_artifact_to_proto)
            .collect();
        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Snapshot {
            version,
            routes: routes
                .into_iter()
                .map(|route| route_to_proto(route, &policies))
                .collect::<Result<Vec<_>, _>>()?,
            policy_artifacts: artifacts,
        })
    }

    fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.tx.subscribe()
    }
}

#[derive(Clone)]
pub struct ConfigServiceImpl {
    state: Arc<ConfigState>,
}

#[tonic::async_trait]
impl ConfigService for ConfigServiceImpl {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Snapshot, Status>> + Send>>;

    async fn subscribe(
        &self,
        _request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        debug!("config subscriber connected");
        let rx = self.state.subscribe();
        let initial = rx.borrow().clone();
        let updates = WatchStream::new(rx).map(Ok);
        let stream = tokio_stream::iter(vec![Ok(initial)]).chain(updates);
        Ok(Response::new(Box::pin(stream)))
    }
}

fn route_to_proto(
    route: RouteSpec,
    policies: &std::collections::HashMap<(String, String), PolicySpec>,
) -> Result<Route, sqlx::Error> {
    let (path_prefix, methods, host) = parse_match(route.match_rules);
    let route_id = route.id.clone();
    let bindings = route
        .policies
        .into_iter()
        .map(|policy| policy_to_proto(&route_id, policy, policies))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Route {
        id: route.id,
        r#match: Some(Match {
            path_prefix,
            methods,
            host,
        }),
        upstreams: route.upstreams.into_iter().map(upstream_to_proto).collect(),
        lb: route.lb.unwrap_or_default(),
        policies: bindings,
    })
}

fn upstream_to_proto(upstream: ModelUpstream) -> Upstream {
    Upstream {
        url: upstream.url,
        weight: upstream.weight.unwrap_or_default(),
        priority: upstream.priority.unwrap_or_default(),
    }
}

fn policy_to_proto(
    route_id: &str,
    policy: RoutePolicy,
    policies: &std::collections::HashMap<(String, String), PolicySpec>,
) -> Result<PolicyBinding, sqlx::Error> {
    let policy_spec = policies
        .get(&(policy.id.clone(), policy.version.clone()))
        .ok_or_else(|| {
            sqlx::Error::Protocol(format!(
                "missing policy {}@{} referenced by route {}",
                policy.id, policy.version, route_id
            ))
        })?;

    let effective = deep_merge_default_with_params(
        &policy_spec.default_config,
        policy.params.as_ref(),
        &format!("route {route_id} policy {}@{}", policy.id, policy.version),
    )
    .map_err(|err| sqlx::Error::Protocol(err.details.join("; ")))?;

    Ok(PolicyBinding {
        stage: policy.stage,
        id: policy.id,
        version: policy.version,
        effective_config_json: serde_json::to_string(&effective)
            .unwrap_or_else(|_| "{}".to_string()),
    })
}

async fn load_policies_for_routes(
    pool: &SqlitePool,
    routes: &[RouteSpec],
) -> Result<std::collections::HashMap<(String, String), PolicySpec>, sqlx::Error> {
    let mut result = std::collections::HashMap::new();
    for route in routes {
        for route_policy in &route.policies {
            let key = (route_policy.id.clone(), route_policy.version.clone());
            if result.contains_key(&key) {
                continue;
            }
            let policy =
                crate::db::get_policy_version(pool, &route_policy.id, &route_policy.version)
                    .await?
                    .ok_or_else(|| {
                        sqlx::Error::Protocol(format!(
                            "missing policy {}@{} referenced by route {}",
                            route_policy.id, route_policy.version, route.id
                        ))
                    })?;
            result.insert(key, policy);
        }
    }
    Ok(result)
}

fn policy_artifact_to_proto(policy: PolicySpec) -> PolicyArtifact {
    PolicyArtifact {
        id: policy.id,
        version: policy.version,
        wasm_uri: policy.wasm_uri,
        sha256: policy.sha256,
    }
}

fn parse_match(match_rules: serde_json::Value) -> (String, Vec<String>, String) {
    let mut path_prefix = String::new();
    let mut methods = Vec::new();
    let mut host = String::new();

    if let Some(obj) = match_rules.as_object() {
        if let Some(value) = obj.get("path_prefix").and_then(|v| v.as_str()) {
            path_prefix = value.to_string();
        }
        if let Some(value) = obj.get("host").and_then(|v| v.as_str()) {
            host = value.to_string();
        }
        if let Some(array) = obj.get("method").and_then(|v| v.as_array()) {
            for method in array {
                if let Some(m) = method.as_str() {
                    methods.push(m.to_string());
                }
            }
        }
    }

    (path_prefix, methods, host)
}
