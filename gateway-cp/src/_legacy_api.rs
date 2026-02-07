use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::error;

use crate::{db, models::{PolicySpec, RouteSpec}};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/policies", post(create_policy).get(list_policies))
        .route("/policies/:id", get(get_policy))
        .route("/routes", post(create_route).get(list_routes))
        .route("/routes/:id", get(get_route).put(update_route).delete(delete_route))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct PolicyQuery {
    version: Option<String>,
}

async fn create_policy(
    State(state): State<AppState>,
    Json(policy): Json<PolicySpec>,
) -> Result<impl IntoResponse, ApiError> {
    db::insert_policy(&state.pool, &policy).await.map_err(map_db_error)?;
    Ok((StatusCode::CREATED, Json(policy)))
}

async fn list_policies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let policies = db::list_policies(&state.pool).await.map_err(map_db_error)?;
    Ok(Json(policies))
}

async fn get_policy(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PolicyQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let policies = db::get_policy(&state.pool, &id, query.version.as_deref())
        .await
        .map_err(map_db_error)?;

    if policies.is_empty() {
        return Err(ApiError::not_found("policy not found"));
    }

    Ok(Json(policies))
}

async fn create_route(
    State(state): State<AppState>,
    Json(route): Json<RouteSpec>,
) -> Result<impl IntoResponse, ApiError> {
    db::insert_route(&state.pool, &route).await.map_err(map_db_error)?;
    Ok((StatusCode::CREATED, Json(route)))
}

async fn list_routes(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let routes = db::list_routes(&state.pool).await.map_err(map_db_error)?;
    Ok(Json(routes))
}

async fn get_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let route = db::get_route(&state.pool, &id).await.map_err(map_db_error)?;
    match route {
        Some(route) => Ok(Json(route)),
        None => Err(ApiError::not_found("route not found")),
    }
}

async fn update_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut route): Json<RouteSpec>,
) -> Result<impl IntoResponse, ApiError> {
    if route.id != id {
        route.id = id;
    }

    let rows = db::update_route(&state.pool, &route).await.map_err(map_db_error)?;
    if rows == 0 {
        return Err(ApiError::not_found("route not found"));
    }

    Ok(Json(route))
}

async fn delete_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = db::delete_route(&state.pool, &id).await.map_err(map_db_error)?;
    if rows == 0 {
        return Err(ApiError::not_found("route not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: &str) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: message.to_string() }
    }

    fn conflict(message: &str) -> Self {
        Self { status: StatusCode::CONFLICT, message: message.to_string() }
    }

    fn internal(message: &str) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: message.to_string() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(serde_json::json!({
            "error": self.message,
        }));
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
