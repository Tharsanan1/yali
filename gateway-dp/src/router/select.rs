use super::{Route, Upstream};
use std::sync::atomic::Ordering;

pub fn select_upstream(route: &Route) -> Option<Upstream> {
    if route.upstreams.is_empty() {
        return None;
    }
    let idx = route.rr_index.fetch_add(1, Ordering::Relaxed);
    Some(route.upstreams[idx % route.upstreams.len()].clone())
}
