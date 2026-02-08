use crate::model::PolicySpec;

use super::{
    validation::{compile_schema, validate_against_schema, validate_supported_stages},
    ValidationError,
};

pub fn validate_policy_spec(policy: &PolicySpec) -> Result<(), ValidationError> {
    let mut details = Vec::new();

    if policy.sha256.trim().is_empty() {
        details.push("policy.sha256 must not be empty".to_string());
    }

    if !policy.default_config.is_object() {
        details.push("policy.default_config must be a JSON object".to_string());
    }

    if let Err(err) = validate_supported_stages(&policy.supported_stages, "policy") {
        details.extend(err.details);
    }

    if !details.is_empty() {
        return Err(ValidationError::with_details(details));
    }

    let schema = compile_schema(&policy.config_schema, "policy")?;
    validate_against_schema(&schema, &policy.default_config, "policy.default_config")
}
