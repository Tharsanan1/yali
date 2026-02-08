pub mod merge;
mod policies;
mod routes;
mod validation;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub details: Vec<String>,
}

impl ValidationError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            details: vec![detail.into()],
        }
    }

    pub fn with_details(details: Vec<String>) -> Self {
        Self { details }
    }
}

pub use policies::validate_policy_spec;
pub use routes::validate_route_policies;
