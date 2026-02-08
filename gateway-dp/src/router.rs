mod matcher;
mod select;

use std::sync::{atomic::AtomicUsize, Arc};

use crate::policy::types::PolicyBinding;

pub use matcher::match_route;
pub use select::select_upstream;

#[derive(Clone, Debug)]
pub struct RouteSnapshot {
    pub routes: Vec<Route>,
}

#[derive(Clone, Debug)]
pub struct Route {
    #[allow(dead_code)]
    pub id: String,
    pub path_prefix: Option<String>,
    pub methods: Vec<String>,
    pub host: Option<String>,
    pub upstreams: Vec<Upstream>,
    pub policies: Vec<PolicyBinding>,
    pub rr_index: Arc<AtomicUsize>,
}

#[derive(Clone, Debug)]
pub struct Upstream {
    pub url: String,
}

impl RouteSnapshot {
    pub fn empty() -> Self {
        Self { routes: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn from_static() -> Self {
        Self {
            routes: vec![Route {
                id: "default".to_string(),
                path_prefix: Some("/".to_string()),
                methods: vec!["GET".to_string()],
                host: None,
                upstreams: vec![Upstream {
                    url: "http://127.0.0.1:9000".to_string(),
                }],
                policies: Vec::new(),
                rr_index: Arc::new(AtomicUsize::new(0)),
            }],
        }
    }
}

impl Route {
    pub fn new(
        id: String,
        path_prefix: Option<String>,
        methods: Vec<String>,
        host: Option<String>,
        upstreams: Vec<Upstream>,
        policies: Vec<PolicyBinding>,
    ) -> Self {
        Self {
            id,
            path_prefix,
            methods,
            host,
            upstreams,
            policies,
            rr_index: Arc::new(AtomicUsize::new(0)),
        }
    }
}
