//! Pingora proxy implementation for the AI Gateway.
//!
//! This module implements the ProxyHttp trait for handling
//! AI API requests, performing protocol adaptation, and
//! forwarding to upstream providers.

use crate::config::Provider;
use crate::state::GatewayState;
use async_trait::async_trait;
use bytes::Bytes;
use pingora_core::prelude::*;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Context for each request, carrying routing and provider information.
pub struct RequestContext {
    /// Selected backend ID
    pub backend_id: Option<String>,
    /// Selected provider
    pub provider: Option<Arc<Provider>>,
    /// Original request path
    pub original_path: String,
    /// Request ID for tracing
    pub request_id: String,
    /// Request body (buffered for potential transformation)
    pub request_body: Option<Bytes>,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            backend_id: None,
            provider: None,
            original_path: String::new(),
            request_id: generate_request_id(),
            request_body: None,
        }
    }
}

fn generate_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("req_{:x}", timestamp)
}

/// The AI Gateway proxy service.
pub struct AIGatewayProxy {
    /// Shared gateway state
    pub state: Arc<GatewayState>,
}

impl AIGatewayProxy {
    /// Create a new AI Gateway proxy.
    pub fn new(state: Arc<GatewayState>) -> Self {
        Self { state }
    }

    /// Handle health check requests.
    async fn handle_health_request(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
    ) -> Result<bool> {
        use serde_json::json;

        // Collect health information
        let providers_count = self
            .state
            .providers
            .read()
            .map(|p| p.len())
            .unwrap_or(0);
        let backends_count = self
            .state
            .backends
            .read()
            .map(|b| b.len())
            .unwrap_or(0);
        let routes_count = self
            .state
            .routers
            .read()
            .map(|r| r.len())
            .unwrap_or(0)
            + self
                .state
                .default_router
                .read()
                .map(|_| 1)
                .unwrap_or(0);

        let health_response = json!({
            "status": "healthy",
            "version": env!("CARGO_PKG_VERSION"),
            "gateway": "yali",
            "stats": {
                "providers": providers_count,
                "backends": backends_count,
                "routes": routes_count
            }
        });

        let body = serde_json::to_vec(&health_response).unwrap_or_default();

        let mut resp = ResponseHeader::build(200, None)?;
        resp.insert_header("content-type", "application/json")?;
        resp.insert_header("x-request-id", &ctx.request_id)?;
        resp.insert_header("x-gateway", "yali")?;
        session.write_response_header(Box::new(resp), true).await?;
        session
            .write_response_body(Some(Bytes::from(body)), true)
            .await?;

        info!(
            request_id = %ctx.request_id,
            "Health check responded"
        );

        Ok(true) // Request handled
    }
}

#[async_trait]
impl ProxyHttp for AIGatewayProxy {
    type CTX = RequestContext;

    fn new_ctx(&self) -> Self::CTX {
        RequestContext::default()
    }

    /// Called when a new request arrives. Performs route matching and provider selection.
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let req_header = session.req_header();

        // Extract host and path
        let host = req_header
            .headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let path = req_header.uri.path();

        ctx.original_path = path.to_string();

        // Handle health endpoint
        if path == "/v1/health" || path == "/health" {
            return self.handle_health_request(session, ctx).await;
        }

        info!(
            request_id = %ctx.request_id,
            host = ?host,
            path = %path,
            method = %req_header.method,
            "Incoming request"
        );

        // Match route
        let route_data = match self.state.match_route(host.as_deref(), path) {
            Some(data) => data,
            None => {
                warn!(
                    request_id = %ctx.request_id,
                    path = %path,
                    "No matching route found"
                );
                // Return 404
                let mut resp = ResponseHeader::build(404, None)?;
                resp.insert_header("content-type", "application/json")?;
                resp.insert_header("x-request-id", &ctx.request_id)?;
                session.write_response_header(Box::new(resp), true).await?;
                session
                    .write_response_body(
                        Some(Bytes::from(
                            r#"{"error":{"code":"ROUTE_NOT_FOUND","message":"No matching route for request"}}"#,
                        )),
                        true,
                    )
                    .await?;
                return Ok(true); // Request handled, don't continue
            }
        };

        // Check HTTP method
        let method = req_header.method.as_str();
        if !route_data
            .methods
            .iter()
            .any(|m| m.eq_ignore_ascii_case(method))
        {
            warn!(
                request_id = %ctx.request_id,
                method = %method,
                allowed = ?route_data.methods,
                "Method not allowed"
            );
            let mut resp = ResponseHeader::build(405, None)?;
            resp.insert_header("content-type", "application/json")?;
            resp.insert_header("x-request-id", &ctx.request_id)?;
            session.write_response_header(Box::new(resp), true).await?;
            session
                .write_response_body(
                    Some(Bytes::from(
                        r#"{"error":{"code":"METHOD_NOT_ALLOWED","message":"Method not allowed for this route"}}"#,
                    )),
                    true,
                )
                .await?;
            return Ok(true);
        }

        ctx.backend_id = Some(route_data.backend_ref.clone());

        // Get backend
        let backend = match self.state.get_backend(&route_data.backend_ref) {
            Some(b) => b,
            None => {
                error!(
                    request_id = %ctx.request_id,
                    backend_ref = %route_data.backend_ref,
                    "Backend not found"
                );
                let mut resp = ResponseHeader::build(503, None)?;
                resp.insert_header("content-type", "application/json")?;
                resp.insert_header("x-request-id", &ctx.request_id)?;
                session.write_response_header(Box::new(resp), true).await?;
                session
                    .write_response_body(
                        Some(Bytes::from(
                            r#"{"error":{"code":"BACKEND_UNAVAILABLE","message":"Backend configuration not found"}}"#,
                        )),
                        true,
                    )
                    .await?;
                return Ok(true);
            }
        };

        // Select provider from backend
        let provider = match self.state.select_provider(&backend) {
            Some(p) => p,
            None => {
                error!(
                    request_id = %ctx.request_id,
                    backend_id = %backend.id,
                    "No available provider"
                );
                let mut resp = ResponseHeader::build(503, None)?;
                resp.insert_header("content-type", "application/json")?;
                resp.insert_header("x-request-id", &ctx.request_id)?;
                session.write_response_header(Box::new(resp), true).await?;
                session
                    .write_response_body(
                        Some(Bytes::from(
                            r#"{"error":{"code":"PROVIDER_UNAVAILABLE","message":"No available provider in backend"}}"#,
                        )),
                        true,
                    )
                    .await?;
                return Ok(true);
            }
        };

        info!(
            request_id = %ctx.request_id,
            backend_id = %backend.id,
            provider_id = %provider.id,
            "Selected provider"
        );

        ctx.provider = Some(provider);

        Ok(false) // Continue processing
    }

    /// Select the upstream peer (provider endpoint).
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let provider = ctx
            .provider
            .as_ref()
            .ok_or_else(|| Error::new(ErrorType::ConnectError))?;

        // Parse the endpoint URL
        let endpoint = &provider.spec.endpoint;
        let (host, port, use_tls) = parse_endpoint(endpoint)?;

        debug!(
            request_id = %ctx.request_id,
            provider_id = %provider.id,
            host = %host,
            port = %port,
            tls = %use_tls,
            "Connecting to upstream"
        );

        let peer = HttpPeer::new((host.as_str(), port), use_tls, host.clone());
        Ok(Box::new(peer))
    }

    /// Modify the request before sending to upstream.
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        let provider = match &ctx.provider {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let adapter = &provider.spec.adapter;
        let original_path = ctx.original_path.clone();
        let request_id = ctx.request_id.clone();

        // Apply URL transformations
        // NOTE: Currently `path_prefix` replaces the entire path rather than being a true prefix.
        // This is intentional for simple proxy use cases where you want to route to a specific
        // upstream endpoint (e.g., /v1/chat -> /v1/chat/completions on the provider).
        // For more complex scenarios like stripping/replacing only the matched prefix,
        // consider using `url.path_template` with placeholders instead.
        if let Some(path_prefix) = &adapter.url.path_prefix {
            debug!(
                request_id = %request_id,
                original_path = %original_path,
                new_path = %path_prefix,
                "Rewriting path (path_prefix replaces entire path)"
            );
            upstream_request.set_uri(path_prefix.parse().map_err(|e| {
                error!("Failed to parse URI: {}", e);
                Error::new(ErrorType::InvalidHTTPHeader)
            })?);
        }

        // Add query parameters
        if !adapter.url.query_params.is_empty() {
            let mut uri_parts = upstream_request.uri.clone().into_parts();
            let existing_query = uri_parts
                .path_and_query
                .as_ref()
                .and_then(|pq| pq.query())
                .unwrap_or("");

            let new_params: Vec<String> = adapter
                .url
                .query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();

            let new_query = if existing_query.is_empty() {
                new_params.join("&")
            } else {
                format!("{}&{}", existing_query, new_params.join("&"))
            };

            let path = uri_parts
                .path_and_query
                .as_ref()
                .map(|pq| pq.path())
                .unwrap_or("/");

            let new_path_and_query = format!("{}?{}", path, new_query);
            uri_parts.path_and_query = Some(
                new_path_and_query
                    .parse()
                    .map_err(|_| Error::new(ErrorType::InvalidHTTPHeader))?,
            );

            upstream_request.set_uri(
                http::Uri::from_parts(uri_parts)
                    .map_err(|_| Error::new(ErrorType::InvalidHTTPHeader))?,
            );
        }

        // Apply authentication
        match adapter.auth.auth_type.as_str() {
            "bearer" => {
                if let Some(secret_ref) = &adapter.auth.secret_ref {
                    let token = resolve_secret(secret_ref);
                    if let Some(t) = token {
                        upstream_request.insert_header("Authorization", format!("Bearer {}", t))?;
                    }
                }
            }
            "header" => {
                if let (Some(key), Some(secret_ref)) = (&adapter.auth.key, &adapter.auth.secret_ref)
                {
                    let value = resolve_secret(secret_ref);
                    if let Some(v) = value {
                        upstream_request.insert_header(key.clone(), v)?;
                    }
                }
            }
            _ => {}
        }

        // Add headers
        for (key, value) in &adapter.headers.add {
            upstream_request.insert_header(key.clone(), value.clone())?;
        }

        // Remove headers
        for key in &adapter.headers.remove {
            upstream_request.remove_header(key);
        }

        // Set host header
        let endpoint = &provider.spec.endpoint;
        if let Ok((host, _, _)) = parse_endpoint(endpoint) {
            upstream_request.insert_header("Host", host)?;
        }

        // Add request ID header
        upstream_request.insert_header("X-Request-ID", request_id.clone())?;

        debug!(
            request_id = %request_id,
            uri = %upstream_request.uri,
            "Upstream request prepared"
        );

        Ok(())
    }

    /// Called when the upstream response header is received.
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        // Add gateway headers
        upstream_response.insert_header("X-Request-ID", &ctx.request_id)?;
        upstream_response.insert_header("X-Gateway", "yali")?;

        info!(
            request_id = %ctx.request_id,
            status = %upstream_response.status.as_u16(),
            "Upstream response received"
        );

        Ok(())
    }
}

/// Parse an endpoint URL into host, port, and TLS flag.
fn parse_endpoint(endpoint: &str) -> Result<(String, u16, bool)> {
    let use_tls = endpoint.starts_with("https://");
    let stripped = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .unwrap_or(endpoint);

    let (host, port) = if let Some(idx) = stripped.find(':') {
        let h = &stripped[..idx];
        let p_str =
            stripped[idx + 1..]
                .split('/')
                .next()
                .unwrap_or(if use_tls { "443" } else { "80" });
        let p = p_str
            .parse::<u16>()
            .unwrap_or(if use_tls { 443 } else { 80 });
        (h.to_string(), p)
    } else {
        let h = stripped.split('/').next().unwrap_or(stripped);
        (h.to_string(), if use_tls { 443 } else { 80 })
    };

    Ok((host, port, use_tls))
}

/// Resolve a secret reference to its value.
/// Supports env:// for environment variables.
fn resolve_secret(secret_ref: &str) -> Option<String> {
    if let Some(env_var) = secret_ref.strip_prefix("env://") {
        std::env::var(env_var).ok()
    } else {
        // For now, return the reference as-is for testing
        // In production, this would integrate with Vault, AWS SM, etc.
        Some(secret_ref.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_endpoint() {
        let (host, port, tls) = parse_endpoint("https://api.openai.com").unwrap();
        assert_eq!(host, "api.openai.com");
        assert_eq!(port, 443);
        assert!(tls);

        let (host, port, tls) = parse_endpoint("http://localhost:8080").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert!(!tls);

        let (host, port, tls) = parse_endpoint("http://localhost:8080/v1").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
        assert!(!tls);
    }

    #[test]
    fn test_resolve_secret_env() {
        std::env::set_var("TEST_SECRET", "test_value");
        assert_eq!(
            resolve_secret("env://TEST_SECRET"),
            Some("test_value".to_string())
        );
        std::env::remove_var("TEST_SECRET");
    }
}
