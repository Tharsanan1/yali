use async_trait::async_trait;
use http::Method;
use http::Uri;
use pingora::prelude::*;
use std::sync::Arc;
use tracing::{debug, warn};
use url::Url;

use crate::policy::errors::PolicyRuntimeError;
use crate::policy::types::{PolicyDecision, RequestView};
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
    type CTX = ProxyCtx;

    fn new_ctx(&self) -> Self::CTX {
        ProxyCtx::default()
    }

    async fn request_filter(&self, _session: &mut Session, _ctx: &mut Self::CTX) -> Result<bool> {
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let request = session.req_header();
        let path = request.uri.path().to_string();
        let method = request.method.as_str().to_string();
        let host = request
            .headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .map(ToString::to_string);
        let snapshot = self.state.snapshot();

        let route = router::match_route(&snapshot.routes, &path, &method, host.as_deref())
            .cloned()
            .ok_or_else(|| {
                warn!(path = %path, method = %method, host = host.as_deref().unwrap_or(""), routes = snapshot.routes.routes.len(), "no route match");
                Error::new(ErrorType::Custom("no route"))
            })?;

        let request_view = RequestView {
            method: method.clone(),
            path: path.clone(),
            host: host.clone(),
            headers: request
                .headers
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_string(), value.to_string()))
                })
                .collect(),
        };
        let decision = snapshot
            .policies
            .evaluate_pre_upstream(&route.policies, &request_view)
            .map_err(policy_fail_closed)?;
        validate_supported_pre_upstream_actions(&decision).map_err(policy_fail_closed)?;

        let upstream = if let Some(hint) = decision.upstream_hint.as_deref() {
            route
                .upstreams
                .iter()
                .find(|u| u.url == hint)
                .cloned()
                .ok_or_else(|| {
                    warn!(route_id = %route.id, upstream_hint = %hint, "upstream hint does not match any route upstream");
                    Error::new(ErrorType::HTTPStatus(500))
                })?
        } else {
            router::select_upstream(&route).ok_or_else(|| {
                warn!(route_id = %route.id, "no upstream available for route");
                Error::new(ErrorType::Custom("no upstream"))
            })?
        };

        ctx.decision = Some(decision);
        let peer = build_peer(&upstream)?;
        debug!(
            path = %path,
            method = %method,
            host = host.as_deref().unwrap_or(""),
            route_id = %route.id,
            upstream = %upstream.url,
            "proxying request"
        );
        Ok(Box::new(peer))
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let Some(decision) = ctx.decision.take() else {
            return Ok(());
        };

        if let Some(rewrite) = decision.request_rewrite {
            if let Some(method) = rewrite.method {
                let parsed = Method::from_bytes(method.as_bytes())
                    .map_err(|_| Error::new(ErrorType::HTTPStatus(500)))?;
                upstream_request.set_method(parsed);
            }

            if let Some(path) = rewrite.path {
                let mut uri_builder = Uri::builder().path_and_query(path);
                if let Some(authority) = upstream_request.uri.authority().cloned() {
                    uri_builder = uri_builder.authority(authority.as_str());
                }
                if let Some(scheme) = upstream_request.uri.scheme_str() {
                    uri_builder = uri_builder.scheme(scheme);
                }
                let new_uri = uri_builder
                    .build()
                    .map_err(|_| Error::new(ErrorType::HTTPStatus(500)))?;
                upstream_request.set_uri(new_uri);
            }
        }

        for header in decision.request_headers {
            if header.overwrite {
                upstream_request
                    .insert_header(header.name, header.value)
                    .map_err(|_| Error::new(ErrorType::HTTPStatus(500)))?;
            } else {
                upstream_request
                    .append_header(header.name, header.value)
                    .map_err(|_| Error::new(ErrorType::HTTPStatus(500)))?;
            }
        }

        Ok(())
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

#[derive(Default)]
pub struct ProxyCtx {
    decision: Option<PolicyDecision>,
}

fn validate_supported_pre_upstream_actions(
    decision: &PolicyDecision,
) -> Result<(), PolicyRuntimeError> {
    if decision.direct_response.is_some() {
        return Err(PolicyRuntimeError::UnsupportedDecisionAction {
            reason: "direct response is not supported in pre_upstream yet".to_string(),
        });
    }
    if decision.request_body_patch.is_some() {
        return Err(PolicyRuntimeError::UnsupportedDecisionAction {
            reason: "request body mutation is not supported yet".to_string(),
        });
    }
    if !decision.response_headers.is_empty() {
        return Err(PolicyRuntimeError::UnsupportedDecisionAction {
            reason: "response header mutations are not supported in pre_upstream".to_string(),
        });
    }
    Ok(())
}

fn policy_fail_closed(err: PolicyRuntimeError) -> Box<Error> {
    warn!(error = %err, "policy execution failed");
    Error::new(ErrorType::HTTPStatus(500))
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

    let host = url
        .host_str()
        .ok_or_else(|| Error::new(ErrorType::Custom("invalid upstream host")))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| Error::new(ErrorType::Custom("invalid upstream port")))?;
    let addr = format!("{host}:{port}");

    Ok(HttpPeer::new(addr, tls, host.to_string()))
}
