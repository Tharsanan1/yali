use super::{Route, RouteSnapshot};

struct Candidate<'a> {
    route: &'a Route,
    path_len: usize,
    host_specific: bool,
    method_specific: bool,
}

pub fn match_route<'a>(
    snapshot: &'a RouteSnapshot,
    path: &str,
    method: &str,
    host: Option<&str>,
) -> Option<&'a Route> {
    let mut best: Option<Candidate<'a>> = None;

    for route in &snapshot.routes {
        if !matches_route(route, path, method, host) {
            continue;
        }

        let path_len = route.path_prefix.as_ref().map(|p| p.len()).unwrap_or(0);
        let candidate = Candidate {
            route,
            path_len,
            host_specific: route.host.is_some(),
            method_specific: !route.methods.is_empty(),
        };

        match &best {
            None => best = Some(candidate),
            Some(current) => {
                if is_better(&candidate, current) {
                    best = Some(candidate);
                }
            }
        }
    }

    best.map(|candidate| candidate.route)
}

fn matches_route(route: &Route, path: &str, method: &str, host: Option<&str>) -> bool {
    if let Some(prefix) = &route.path_prefix {
        if !path.starts_with(prefix) {
            return false;
        }
    }

    if !route.methods.is_empty()
        && !route
            .methods
            .iter()
            .any(|route_method| route_method.eq_ignore_ascii_case(method))
    {
        return false;
    }

    if let Some(route_host) = &route.host {
        let request_host = match host {
            Some(value) => normalize_host(value),
            None => return false,
        };
        if normalize_host(route_host) != request_host {
            return false;
        }
    }

    true
}

fn is_better(a: &Candidate<'_>, b: &Candidate<'_>) -> bool {
    if a.path_len != b.path_len {
        return a.path_len > b.path_len;
    }
    if a.host_specific != b.host_specific {
        return a.host_specific && !b.host_specific;
    }
    if a.method_specific != b.method_specific {
        return a.method_specific && !b.method_specific;
    }
    a.route.id < b.route.id
}

fn normalize_host(value: &str) -> String {
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.starts_with('[') {
        if let Some(end) = trimmed.find(']') {
            return trimmed[..=end].to_string();
        }
    }

    trimmed
        .split(':')
        .next()
        .unwrap_or(trimmed.as_str())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::Upstream;

    fn route(id: &str, path: Option<&str>, methods: &[&str], host: Option<&str>) -> Route {
        Route::new(
            id.to_string(),
            path.map(ToString::to_string),
            methods.iter().map(|m| m.to_string()).collect(),
            host.map(ToString::to_string),
            vec![Upstream {
                url: "http://127.0.0.1:9000".to_string(),
            }],
        )
    }

    #[test]
    fn selects_longest_path_prefix() {
        let snapshot = RouteSnapshot {
            routes: vec![
                route("generic", Some("/v1/users"), &["GET"], None),
                route("specific", Some("/v1/users/profile"), &["GET"], None),
            ],
        };

        let selected = match_route(&snapshot, "/v1/users/profile", "GET", None).unwrap();
        assert_eq!(selected.id, "specific");
    }

    #[test]
    fn selects_method_specific_over_wildcard() {
        let snapshot = RouteSnapshot {
            routes: vec![
                route("any", Some("/v1/resource"), &[], None),
                route("get", Some("/v1/resource"), &["GET"], None),
            ],
        };

        let selected = match_route(&snapshot, "/v1/resource", "GET", None).unwrap();
        assert_eq!(selected.id, "get");
    }

    #[test]
    fn selects_host_specific_over_wildcard() {
        let snapshot = RouteSnapshot {
            routes: vec![
                route("any-host", Some("/v1/resource"), &["GET"], None),
                route(
                    "host-a",
                    Some("/v1/resource"),
                    &["GET"],
                    Some("api.example.com"),
                ),
            ],
        };

        let selected = match_route(
            &snapshot,
            "/v1/resource",
            "GET",
            Some("api.example.com:443"),
        )
        .unwrap();
        assert_eq!(selected.id, "host-a");
    }

    #[test]
    fn tie_breaks_by_route_id_for_stability() {
        let snapshot = RouteSnapshot {
            routes: vec![
                route("route-b", Some("/v1/resource"), &["GET"], None),
                route("route-a", Some("/v1/resource"), &["GET"], None),
            ],
        };

        let selected = match_route(&snapshot, "/v1/resource", "GET", None).unwrap();
        assert_eq!(selected.id, "route-a");
    }
}
