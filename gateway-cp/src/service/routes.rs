use sqlx::SqlitePool;

use crate::{db, model::RouteSpec};

use super::{
    merge::deep_merge_default_with_params,
    validation::{compile_schema, validate_against_schema},
    ValidationError,
};

pub async fn validate_route_policies(
    pool: &SqlitePool,
    route: &RouteSpec,
) -> Result<(), ValidationError> {
    let mut details = Vec::new();

    for (index, route_policy) in route.policies.iter().enumerate() {
        let context = format!("route.policies[{index}]");

        if route_policy.params.as_ref().is_some_and(|v| !v.is_object()) {
            details.push(format!("{context}.params must be a JSON object"));
            continue;
        }

        let policy = db::get_policy_version(pool, &route_policy.id, &route_policy.version)
            .await
            .map_err(|err| {
                ValidationError::new(format!("{context}: failed to read policy from db: {err}"))
            })?;

        let Some(policy) = policy else {
            details.push(format!(
                "{context}: policy {}@{} not found",
                route_policy.id, route_policy.version
            ));
            continue;
        };

        if !policy
            .supported_stages
            .iter()
            .any(|stage| stage == &route_policy.stage)
        {
            details.push(format!(
                "{context}: stage {} not allowed for {}@{}",
                route_policy.stage, route_policy.id, route_policy.version
            ));
            continue;
        }

        let schema = match compile_schema(&policy.config_schema, &context) {
            Ok(schema) => schema,
            Err(err) => {
                details.extend(err.details);
                continue;
            }
        };

        let merged = match deep_merge_default_with_params(
            &policy.default_config,
            route_policy.params.as_ref(),
            &context,
        ) {
            Ok(value) => value,
            Err(err) => {
                details.extend(err.details);
                continue;
            }
        };

        if let Err(err) = validate_against_schema(&schema, &merged, &format!("{context}.params")) {
            details.extend(err.details);
        }
    }

    if details.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::with_details(details))
    }
}
