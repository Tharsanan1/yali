//! Azure OpenAI Protocol Adapter
//!
//! Transforms between canonical OpenAI format and Azure OpenAI format.
//!
//! Key differences:
//! - Uses `api-key` header instead of Bearer token
//! - Model is specified via deployment name in URL path
//! - Requires `api-version` query parameter
//! - URL pattern: /openai/deployments/{deployment}/chat/completions

use super::traits::{
    AdapterError, ProtocolAdapter, TransformContext, TransformedRequest, TransformedResponse,
};
use crate::config::Provider;
use bytes::Bytes;
use serde_json::Value;
use std::collections::HashMap;

/// Azure OpenAI protocol adapter.
pub struct AzureOpenAIAdapter {
    /// Default values to inject into requests.
    defaults: HashMap<String, Value>,
    /// Fields to remove from requests.
    remove_fields: Vec<String>,
    /// Azure API version.
    api_version: String,
    /// Default deployment name (if not specified in request).
    default_deployment: Option<String>,
}

impl AzureOpenAIAdapter {
    /// Create a new Azure OpenAI adapter from provider configuration.
    pub fn new(provider: &Provider) -> Self {
        let api_version = provider
            .spec
            .adapter
            .url
            .query_params
            .get("api-version")
            .cloned()
            .unwrap_or_else(|| "2024-02-15-preview".to_string());

        // Check for deployment in path_template
        let default_deployment =
            provider
                .spec
                .adapter
                .url
                .path_template
                .as_ref()
                .and_then(|template| {
                    // Extract deployment from template if static
                    if template.contains("{deployment}") {
                        None // Dynamic deployment
                    } else {
                        // Try to extract deployment from static path
                        template
                            .split("/deployments/")
                            .nth(1)
                            .and_then(|s| s.split('/').next())
                            .map(String::from)
                    }
                });

        Self {
            defaults: provider.spec.adapter.request_body.defaults.clone(),
            remove_fields: provider.spec.adapter.request_body.remove_fields.clone(),
            api_version,
            default_deployment,
        }
    }

    /// Extract deployment name from model string or use default.
    fn get_deployment(model: Option<&str>, default: &Option<String>) -> Option<String> {
        model.map(String::from).or_else(|| default.clone())
    }
}

impl ProtocolAdapter for AzureOpenAIAdapter {
    fn name(&self) -> &str {
        "azure_openai"
    }

    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError> {
        let mut value: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidRequestBody(e.to_string()))?;

        // Extract model for deployment name
        let model = value.get("model").and_then(|m| m.as_str());
        let deployment = Self::get_deployment(model, &self.default_deployment)
            .ok_or_else(|| AdapterError::MissingField("model or deployment".to_string()))?;

        ctx.model = model.map(String::from);
        ctx.deployment = Some(deployment.clone());

        // Remove model field for Azure (it's in the URL)
        if let Some(obj) = value.as_object_mut() {
            obj.remove("model");

            // Apply defaults
            for (key, default_value) in &self.defaults {
                if !obj.contains_key(key) {
                    obj.insert(key.clone(), default_value.clone());
                }
            }

            // Remove specified fields
            for field in &self.remove_fields {
                obj.remove(field);
            }
        }

        let body = serde_json::to_vec(&value)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        // Build the Azure-specific path
        let path = format!("/openai/deployments/{}/chat/completions", deployment);

        // Query params
        let mut query_params = HashMap::new();
        query_params.insert("api-version".to_string(), self.api_version.clone());

        Ok(TransformedRequest {
            body: Bytes::from(body),
            headers: HashMap::new(),
            path: Some(path),
            query_params,
            context: ctx.clone(),
        })
    }

    fn transform_response(
        &self,
        body: &Bytes,
        ctx: &TransformContext,
    ) -> Result<TransformedResponse, AdapterError> {
        // Azure OpenAI responses are already OpenAI-compatible
        // Just need to potentially restore the model name
        let mut value: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidResponseBody(e.to_string()))?;

        // Azure might not include model in response, restore from context
        if let Some(obj) = value.as_object_mut() {
            if !obj.contains_key("model") {
                if let Some(model) = &ctx.model {
                    obj.insert("model".to_string(), Value::String(model.clone()));
                } else if let Some(deployment) = &ctx.deployment {
                    obj.insert("model".to_string(), Value::String(deployment.clone()));
                }
            }
        }

        let body = serde_json::to_vec(&value)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        Ok(TransformedResponse {
            body: Bytes::from(body),
            headers: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, ProviderSpec, UrlTransforms};
    use serde_json::json;

    fn create_test_provider() -> Provider {
        Provider {
            id: "test_azure".to_string(),
            name: "Test Azure".to_string(),
            spec: ProviderSpec {
                provider_type: "azure_openai".to_string(),
                endpoint: "https://myresource.openai.azure.com".to_string(),
                adapter: AdapterConfig {
                    url: UrlTransforms {
                        query_params: [(
                            "api-version".to_string(),
                            "2024-02-15-preview".to_string(),
                        )]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_transform_request_removes_model() {
        let provider = create_test_provider();
        let adapter = AzureOpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello!"}]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        // Model should be removed from body
        assert!(transformed.get("model").is_none());
        // But preserved in context
        assert_eq!(ctx.model, Some("gpt-4".to_string()));
        assert_eq!(ctx.deployment, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_transform_request_generates_path() {
        let provider = create_test_provider();
        let adapter = AzureOpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4-turbo",
            "messages": [{"role": "user", "content": "Hello!"}]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();

        assert_eq!(
            result.path,
            Some("/openai/deployments/gpt-4-turbo/chat/completions".to_string())
        );
        assert_eq!(
            result.query_params.get("api-version"),
            Some(&"2024-02-15-preview".to_string())
        );
    }

    #[test]
    fn test_transform_response_restores_model() {
        let provider = create_test_provider();
        let adapter = AzureOpenAIAdapter::new(&provider);

        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "choices": [{
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }]
        });

        let body = Bytes::from(serde_json::to_vec(&response).unwrap());
        let ctx = TransformContext {
            model: Some("gpt-4".to_string()),
            deployment: Some("gpt-4".to_string()),
            ..Default::default()
        };

        let result = adapter.transform_response(&body, &ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        // Model should be restored
        assert_eq!(transformed.get("model"), Some(&json!("gpt-4")));
    }
}
