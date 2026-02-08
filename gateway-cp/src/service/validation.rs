use jsonschema::{error::ValidationErrorKind, JSONSchema};
use serde_json::Value;

use super::ValidationError;

pub const ALLOWED_POLICY_STAGES: [&str; 3] = ["pre_route", "pre_upstream", "post_response"];

pub fn validate_supported_stages(stages: &[String], context: &str) -> Result<(), ValidationError> {
    if stages.is_empty() {
        return Err(ValidationError::new(format!(
            "{context}.supported_stages must not be empty",
        )));
    }

    let mut details = Vec::new();
    for stage in stages {
        if !ALLOWED_POLICY_STAGES.iter().any(|allowed| allowed == stage) {
            details.push(format!(
                "{context}.supported_stages contains unsupported stage {stage}",
            ));
        }
    }

    if details.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::with_details(details))
    }
}

pub fn compile_schema(schema: &Value, context: &str) -> Result<JSONSchema, ValidationError> {
    JSONSchema::compile(schema)
        .map_err(|err| ValidationError::new(format!("{context}.config_schema is invalid: {err}")))
}

pub fn validate_against_schema(
    schema: &JSONSchema,
    value: &Value,
    context: &str,
) -> Result<(), ValidationError> {
    let errors = schema.validate(value).err();
    let Some(errors) = errors else {
        return Ok(());
    };

    let mut details = Vec::new();
    for error in errors {
        let pointer = error.instance_path.to_string();
        let location = if pointer.is_empty() {
            context.to_string()
        } else {
            format!("{context}{pointer}")
        };

        let detail = match error.kind {
            ValidationErrorKind::AdditionalItems { .. } => {
                format!("{location}: additional items are not allowed")
            }
            _ => format!("{location}: {error}"),
        };
        details.push(detail);
    }

    Err(ValidationError::with_details(details))
}
