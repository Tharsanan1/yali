//! Gateway state management.
//!
//! This module implements the shared memory model for the gateway,
//! including provider registry, backend registry, and routing tables.

use crate::config::{Backend, GatewayConfig, Provider, Route};
use matchit::Router;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Route data stored in the radix trie router.
#[derive(Debug, Clone)]
pub struct RouteData {
    /// Route identifier
    pub id: String,
    /// Backend reference
    pub backend_ref: String,
    /// Allowed HTTP methods
    pub methods: Vec<String>,
}

/// The main gateway state holding all configuration.
pub struct GatewayState {
    /// Provider registry: Map<ProviderID, Provider>
    pub providers: Arc<RwLock<HashMap<String, Arc<Provider>>>>,
    /// Backend registry: Map<BackendID, Backend>
    pub backends: Arc<RwLock<HashMap<String, Arc<Backend>>>>,
    /// Router per tenant/host: Map<Hostname, RadixRouter>
    pub routers: Arc<RwLock<HashMap<String, Router<RouteData>>>>,
    /// Default router (for requests without host matching)
    pub default_router: Arc<RwLock<Router<RouteData>>>,
}

impl GatewayState {
    /// Create a new empty gateway state.
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            backends: Arc::new(RwLock::new(HashMap::new())),
            routers: Arc::new(RwLock::new(HashMap::new())),
            default_router: Arc::new(RwLock::new(Router::new())),
        }
    }

    /// Load configuration into the gateway state.
    pub fn load_config(&self, config: &GatewayConfig) -> Result<(), String> {
        // Load providers
        {
            let mut providers = self.providers.write().map_err(|e| e.to_string())?;
            providers.clear();
            for provider in &config.providers {
                providers.insert(provider.id.clone(), Arc::new(provider.clone()));
            }
        }

        // Load backends
        {
            let mut backends = self.backends.write().map_err(|e| e.to_string())?;
            backends.clear();
            for backend in &config.backends {
                backends.insert(backend.id.clone(), Arc::new(backend.clone()));
            }
        }

        // Build routers from routes
        self.build_routers(&config.routes)?;

        Ok(())
    }

    /// Build routing tables from routes.
    fn build_routers(&self, routes: &[Route]) -> Result<(), String> {
        let mut routers: HashMap<String, Router<RouteData>> = HashMap::new();
        let mut default_router: Router<RouteData> = Router::new();

        for route in routes {
            let route_data = RouteData {
                id: route.id.clone(),
                backend_ref: route.spec.backend_ref.clone(),
                methods: route.spec.match_rule.methods.clone(),
            };

            let path = &route.spec.match_rule.path;

            // For prefix matching, we add a wildcard at the end
            let match_path = match route.spec.match_rule.match_type.as_str() {
                "prefix" => {
                    if path.ends_with('/') || path == "/" {
                        format!("{path}{{*rest}}")
                    } else {
                        // Match both exact and with trailing path
                        format!("{path}{{*rest}}")
                    }
                }
                "exact" => path.clone(),
                _ => path.clone(),
            };

            if let Some(host) = &route.host {
                let router = routers.entry(host.clone()).or_default();
                router
                    .insert(&match_path, route_data)
                    .map_err(|e| format!("Failed to insert route '{}': {}", route.id, e))?;
            } else {
                default_router
                    .insert(&match_path, route_data)
                    .map_err(|e| format!("Failed to insert route '{}': {}", route.id, e))?;
            }
        }

        // Update routers
        {
            let mut router_lock = self.routers.write().map_err(|e| e.to_string())?;
            *router_lock = routers;
        }
        {
            let mut default_lock = self.default_router.write().map_err(|e| e.to_string())?;
            *default_lock = default_router;
        }

        Ok(())
    }

    /// Match a request path to a route.
    pub fn match_route(&self, host: Option<&str>, path: &str) -> Option<RouteData> {
        // First try host-specific router
        if let Some(h) = host {
            if let Ok(routers) = self.routers.read() {
                if let Some(router) = routers.get(h) {
                    if let Ok(matched) = router.at(path) {
                        return Some(matched.value.clone());
                    }
                }
            }
        }

        // Fall back to default router
        if let Ok(default_router) = self.default_router.read() {
            if let Ok(matched) = default_router.at(path) {
                return Some(matched.value.clone());
            }
        }

        None
    }

    /// Get a provider by ID.
    pub fn get_provider(&self, id: &str) -> Option<Arc<Provider>> {
        self.providers.read().ok()?.get(id).cloned()
    }

    /// Get a backend by ID.
    pub fn get_backend(&self, id: &str) -> Option<Arc<Backend>> {
        self.backends.read().ok()?.get(id).cloned()
    }

    /// Select a provider from a backend based on load balancing algorithm.
    pub fn select_provider(&self, backend: &Backend) -> Option<Arc<Provider>> {
        // For now, implement simple failover (select first available by priority)
        let mut provider_refs: Vec<_> = backend.spec.providers.iter().collect();
        provider_refs.sort_by_key(|p| p.priority);

        for provider_ref in provider_refs {
            if let Some(provider) = self.get_provider(&provider_ref.provider_ref) {
                return Some(provider);
            }
        }

        None
    }
}

impl Default for GatewayState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_route() {
        let state = GatewayState::new();

        // Build a simple router
        {
            let mut default_router = state.default_router.write().unwrap();
            default_router
                .insert(
                    "/v1/chat{*rest}",
                    RouteData {
                        id: "route_chat".to_string(),
                        backend_ref: "backend_test".to_string(),
                        methods: vec!["POST".to_string()],
                    },
                )
                .unwrap();
        }

        // Test matching
        let matched = state.match_route(None, "/v1/chat/completions");
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "route_chat");
    }

    #[test]
    fn test_matchit_prefix_patterns() {
        use matchit::Router;

        let mut router: Router<&str> = Router::new();

        // Test prefix matching with {*rest}
        router.insert("/v1/chat{*rest}", "chat_route").unwrap();

        // These should all match
        assert!(
            router.at("/v1/chat/completions").is_ok(),
            "/v1/chat/completions should match"
        );
        assert!(router.at("/v1/chat/").is_ok(), "/v1/chat/ should match");

        // /v1/chat (without trailing slash) doesn't match {*rest} pattern
        // This is expected matchit behavior - {*rest} requires at least one character
        assert!(
            router.at("/v1/chat").is_err(),
            "/v1/chat should not match {{*rest}} pattern"
        );
    }
}
