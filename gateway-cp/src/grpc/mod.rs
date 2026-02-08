use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_core::Stream;
use gateway_proto::config::{
    config_service_server::{ConfigService, ConfigServiceServer},
    Match, PolicyRef, Route, Snapshot, SubscribeRequest, Upstream,
};
use sqlx::SqlitePool;
use tokio::sync::watch;
use tokio_stream::{wrappers::WatchStream, StreamExt};
use tonic::{Request, Response, Status};
use tracing::debug;

use crate::model::{RoutePolicy, RouteSpec, Upstream as ModelUpstream};

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
        let snapshot = self.build_snapshot(routes);
        debug!(
            version = snapshot.version,
            routes = snapshot.routes.len(),
            "published config snapshot"
        );
        let _ = self.tx.send(snapshot);
        Ok(())
    }

    fn build_snapshot(&self, routes: Vec<RouteSpec>) -> Snapshot {
        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        Snapshot {
            version,
            routes: routes.into_iter().map(route_to_proto).collect(),
        }
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

fn route_to_proto(route: RouteSpec) -> Route {
    let (path_prefix, methods, host) = parse_match(route.match_rules);
    Route {
        id: route.id,
        r#match: Some(Match {
            path_prefix,
            methods,
            host,
        }),
        upstreams: route.upstreams.into_iter().map(upstream_to_proto).collect(),
        lb: route.lb.unwrap_or_default(),
        policies: route.policies.into_iter().map(policy_to_proto).collect(),
    }
}

fn upstream_to_proto(upstream: ModelUpstream) -> Upstream {
    Upstream {
        url: upstream.url,
        weight: upstream.weight.unwrap_or_default(),
        priority: upstream.priority.unwrap_or_default(),
    }
}

fn policy_to_proto(policy: RoutePolicy) -> PolicyRef {
    PolicyRef {
        stage: policy.stage,
        id: policy.id,
        version: policy.version,
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
