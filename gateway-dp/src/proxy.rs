use async_trait::async_trait;
use pingora::prelude::*;
use std::sync::Arc;
use tracing::{debug, warn};
use url::Url;

use crate::{router, state::State};

pub struct GatewayProxy {
    state: Arc<State>,
}

impl GatewayProxy {
    pub fn new(state: Arc<State>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl ProxyHttp for GatewayProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {
        ()
    }

    async fn request_filter(&self, _session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path().to_string();
        let snapshot = self.state.snapshot();

        let route = router::match_route(&snapshot, &path).cloned().ok_or_else(|| {
            warn!(path = %path, routes = snapshot.routes.len(), "no route match");
            Error::new(ErrorType::Custom("no route"))
        })?;
        let upstream = router::select_upstream(&route).ok_or_else(|| {
            warn!(route_id = %route.id, "no upstream available for route");
            Error::new(ErrorType::Custom("no upstream"))
        })?;

        let peer = build_peer(&upstream)?;
        debug!(
            path = %path,
            route_id = %route.id,
            upstream = %upstream.url,
            "proxying request"
        );
        Ok(Box::new(peer))
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        _resp: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        Ok(())
    }
}

fn build_peer(upstream: &router::Upstream) -> Result<HttpPeer> {
    let url = if upstream.url.contains("://") {
        Url::parse(&upstream.url)
            .map_err(|_| Error::new(ErrorType::Custom("invalid upstream url")))?
    } else {
        Url::parse(&format!("http://{}", upstream.url))
            .map_err(|_| Error::new(ErrorType::Custom("invalid upstream url")))?
    };
    let tls = matches!(url.scheme(), "https");

    let host = url.host_str().ok_or_else(|| Error::new(ErrorType::Custom("invalid upstream host")))?;
    let port = url.port_or_known_default().ok_or_else(|| Error::new(ErrorType::Custom("invalid upstream port")))?;
    let addr = format!("{host}:{port}");

    Ok(HttpPeer::new(addr, tls, host.to_string()))
}
