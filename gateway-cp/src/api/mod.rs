use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;
use tracing::error;

mod health;
mod policies;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config_state: std::sync::Arc<crate::grpc::ConfigState>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route(
            "/policies",
            post(policies::create_policy).get(policies::list_policies),
        )
        .route("/policies/:id", get(policies::get_policy))
        .route(
            "/routes",
            post(routes::create_route).get(routes::list_routes),
        )
        .route(
            "/routes/:id",
            get(routes::get_route)
                .put(routes::update_route)
                .delete(routes::delete_route),
        )
        .with_state(state)
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ApiErrorBody,
}

#[derive(Debug)]
enum ApiErrorBody {
    Message(String),
    Validation { details: Vec<String> },
}

impl ApiError {
    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            body: ApiErrorBody::Message(message.to_string()),
        }
    }

    fn conflict(message: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: ApiErrorBody::Message(message.to_string()),
        }
    }

    fn validation(details: Vec<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            body: ApiErrorBody::Validation { details },
        }
    }

    fn internal(message: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiErrorBody::Message(message.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = match self.body {
            ApiErrorBody::Message(message) => Json(serde_json::json!({
                "error": message,
            })),
            ApiErrorBody::Validation { details } => Json(serde_json::json!({
                "error": "validation_error",
                "details": details,
            })),
        };
        (self.status, body).into_response()
    }
}

fn map_db_error(err: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.message().contains("UNIQUE") || db_err.message().contains("PRIMARY") {
            return ApiError::conflict("resource already exists");
        }
    }

    error!(error = ?err, "database error");
    ApiError::internal("database error")
}
