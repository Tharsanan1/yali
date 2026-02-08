use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::{db, model::RouteSpec, service};
use tracing::info;

use super::{map_db_error, ApiError, AppState};

pub async fn create_route(
    State(state): State<AppState>,
    Json(route): Json<RouteSpec>,
) -> Result<impl IntoResponse, ApiError> {
    service::validate_route_policies(&state.pool, &route)
        .await
        .map_err(|err| ApiError::validation(err.details))?;
    db::insert_route(&state.pool, &route)
        .await
        .map_err(map_db_error)?;
    state
        .config_state
        .publish_from_db(&state.pool)
        .await
        .map_err(map_db_error)?;
    info!(route_id = %route.id, "route created");
    Ok((StatusCode::CREATED, Json(route)))
}

pub async fn list_routes(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let routes = db::list_routes(&state.pool).await.map_err(map_db_error)?;
    Ok(Json(routes))
}

pub async fn get_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let route = db::get_route(&state.pool, &id)
        .await
        .map_err(map_db_error)?;
    match route {
        Some(route) => Ok(Json(route)),
        None => Err(ApiError::not_found("route not found")),
    }
}

pub async fn update_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut route): Json<RouteSpec>,
) -> Result<impl IntoResponse, ApiError> {
    if route.id != id {
        route.id = id;
    }

    service::validate_route_policies(&state.pool, &route)
        .await
        .map_err(|err| ApiError::validation(err.details))?;

    let rows = db::update_route(&state.pool, &route)
        .await
        .map_err(map_db_error)?;
    if rows == 0 {
        return Err(ApiError::not_found("route not found"));
    }

    state
        .config_state
        .publish_from_db(&state.pool)
        .await
        .map_err(map_db_error)?;
    info!(route_id = %route.id, "route updated");
    Ok(Json(route))
}

pub async fn delete_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let rows = db::delete_route(&state.pool, &id)
        .await
        .map_err(map_db_error)?;
    if rows == 0 {
        return Err(ApiError::not_found("route not found"));
    }

    state
        .config_state
        .publish_from_db(&state.pool)
        .await
        .map_err(map_db_error)?;
    info!(route_id = %id, "route deleted");
    Ok(StatusCode::NO_CONTENT)
}
