use arc_swap::ArcSwap;
use std::sync::Arc;

use crate::policy::PolicyRegistry;
use crate::router::RouteSnapshot;

pub struct RuntimeSnapshot {
    pub routes: RouteSnapshot,
    pub policies: PolicyRegistry,
}

pub struct State {
    snapshot: ArcSwap<RuntimeSnapshot>,
}

impl State {
    pub fn new(snapshot: RuntimeSnapshot) -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(snapshot),
        }
    }

    pub fn snapshot(&self) -> Arc<RuntimeSnapshot> {
        self.snapshot.load_full()
    }

    pub fn update(&self, snapshot: RuntimeSnapshot) {
        self.snapshot.store(Arc::new(snapshot));
    }
}
