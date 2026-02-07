use super::{Route, RouteSnapshot};

pub fn match_route<'a>(snapshot: &'a RouteSnapshot, path: &str) -> Option<&'a Route> {
    snapshot.routes.iter().find(|route| {
        route
            .path_prefix
            .as_ref()
            .map(|prefix| path.starts_with(prefix))
            .unwrap_or(false)
    })
}
