use serde_json::Value;

use super::ValidationError;

pub fn deep_merge_default_with_params(
    default_config: &Value,
    params: Option<&Value>,
    context: &str,
) -> Result<Value, ValidationError> {
    let mut merged = default_config.clone();
    if !merged.is_object() {
        return Err(ValidationError::new(format!(
            "{context}.default_config must be a JSON object",
        )));
    }

    if let Some(params) = params {
        if !params.is_object() {
            return Err(ValidationError::new(format!(
                "{context}.params must be a JSON object",
            )));
        }
        merge_objects(&mut merged, params);
    }

    Ok(merged)
}

fn merge_objects(target: &mut Value, overlay: &Value) {
    let Some(target_obj) = target.as_object_mut() else {
        *target = overlay.clone();
        return;
    };
    let Some(overlay_obj) = overlay.as_object() else {
        *target = overlay.clone();
        return;
    };

    for (key, overlay_value) in overlay_obj {
        if let Some(target_value) = target_obj.get_mut(key) {
            if target_value.is_object() && overlay_value.is_object() {
                merge_objects(target_value, overlay_value);
            } else {
                *target_value = overlay_value.clone();
            }
        } else {
            target_obj.insert(key.clone(), overlay_value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::deep_merge_default_with_params;

    #[test]
    fn deep_merge_keeps_existing_nested_keys() {
        let defaults = json!({
            "auth": {
                "issuer": "https://issuer",
                "required_scopes": ["read"]
            },
            "audience": "gateway"
        });
        let params = json!({
            "auth": {
                "required_scopes": ["read", "write"]
            }
        });

        let merged = deep_merge_default_with_params(&defaults, Some(&params), "policy")
            .expect("merge failed");

        assert_eq!(
            merged,
            json!({
                "auth": {
                    "issuer": "https://issuer",
                    "required_scopes": ["read", "write"]
                },
                "audience": "gateway"
            })
        );
    }

    #[test]
    fn arrays_are_replaced_instead_of_appended() {
        let defaults = json!({ "scopes": ["read"] });
        let params = json!({ "scopes": ["write"] });

        let merged = deep_merge_default_with_params(&defaults, Some(&params), "policy")
            .expect("merge failed");
        assert_eq!(merged, json!({ "scopes": ["write"] }));
    }
}
