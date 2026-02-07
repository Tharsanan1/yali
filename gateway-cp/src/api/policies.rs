use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::{db, model::PolicySpec};

use super::{ApiError, AppState, map_db_error};

#[derive(Deserialize)]
pub(super) struct PolicyQuery {
    version: Option<String>,
}

pub async fn create_policy(
    State(state): State<AppState>,
    Json(policy): Json<PolicySpec>,
) -> Result<impl IntoResponse, ApiError> {
    db::insert_policy(&state.pool, &policy).await.map_err(map_db_error)?;
    state.config_state.publish_from_db(&state.pool).await.map_err(map_db_error)?;
    info!(policy_id = %policy.id, version = %policy.version, "policy created");
    Ok((StatusCode::CREATED, Json(policy)))
}

pub async fn list_policies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let policies = db::list_policies(&state.pool).await.map_err(map_db_error)?;
    Ok(Json(policies))
}

pub(super) async fn get_policy(
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
