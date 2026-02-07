use arc_swap::ArcSwap;
use std::sync::Arc;

use crate::router::RouteSnapshot;

pub struct State {
    snapshot: ArcSwap<RouteSnapshot>,
}

impl State {
    pub fn new(snapshot: RouteSnapshot) -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(snapshot),
        }
    }

    pub fn snapshot(&self) -> Arc<RouteSnapshot> {
        self.snapshot.load_full()
    }

    pub fn update(&self, snapshot: RouteSnapshot) {
        self.snapshot.store(Arc::new(snapshot));
    }
}
