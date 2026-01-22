//! OpenAI Protocol Adapter
//!
//! The canonical adapter - OpenAI format is the internal format,
//! so this adapter is essentially a pass-through with minimal transformation.

use super::traits::{
    AdapterError, ProtocolAdapter, TransformContext, TransformedRequest, TransformedResponse,
};
use crate::config::Provider;
use bytes::Bytes;
use std::collections::HashMap;

/// OpenAI protocol adapter.
///
/// Since OpenAI format is the canonical format, this adapter performs
/// minimal transformations, primarily applying configured defaults
/// and field mappings.
pub struct OpenAIAdapter {
    /// Default values to inject into requests.
    defaults: HashMap<String, serde_json::Value>,
    /// Fields to remove from requests.
    remove_fields: Vec<String>,
}

impl OpenAIAdapter {
    /// Create a new OpenAI adapter from provider configuration.
    pub fn new(provider: &Provider) -> Self {
        Self {
            defaults: provider.spec.adapter.request_body.defaults.clone(),
            remove_fields: provider.spec.adapter.request_body.remove_fields.clone(),
        }
    }
}

impl ProtocolAdapter for OpenAIAdapter {
    fn name(&self) -> &str {
        "openai"
    }

    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError> {
        // Parse the incoming body
        let mut value: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidRequestBody(e.to_string()))?;

        // Extract model for context
        if let Some(model) = value.get("model").and_then(|m| m.as_str()) {
            ctx.model = Some(model.to_string());
        }

        // Apply defaults
        if let Some(obj) = value.as_object_mut() {
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

        Ok(TransformedRequest {
            body: Bytes::from(body),
            headers: HashMap::new(),
            path: None,
            query_params: HashMap::new(),
            context: ctx.clone(),
        })
    }

    fn transform_response(
        &self,
        body: &Bytes,
        _ctx: &TransformContext,
    ) -> Result<TransformedResponse, AdapterError> {
        // OpenAI responses are already in canonical format
        Ok(TransformedResponse {
            body: body.clone(),
            headers: HashMap::new(),
        })
    }

    fn transform_stream_chunk(
        &self,
        chunk: &Bytes,
        _ctx: &TransformContext,
    ) -> Result<Bytes, AdapterError> {
        // OpenAI streaming chunks are already in canonical format
        Ok(chunk.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, BodyTransforms, ProviderSpec};
    use serde_json::json;

    fn create_test_provider() -> Provider {
        Provider {
            id: "test_openai".to_string(),
            name: "Test OpenAI".to_string(),
            spec: ProviderSpec {
                provider_type: "openai".to_string(),
                endpoint: "https://api.openai.com".to_string(),
                adapter: AdapterConfig {
                    request_body: BodyTransforms {
                        defaults: [("temperature".to_string(), json!(0.7))]
                            .into_iter()
                            .collect(),
                        remove_fields: vec!["internal_field".to_string()],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_transform_request_passthrough() {
        let provider = create_test_provider();
        let adapter = OpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: serde_json::Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(transformed.get("model"), Some(&json!("gpt-4")));
        assert_eq!(ctx.model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_transform_request_applies_defaults() {
        let provider = create_test_provider();
        let adapter = OpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello!"}]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: serde_json::Value = serde_json::from_slice(&result.body).unwrap();

        // Default temperature should be applied
        assert_eq!(transformed.get("temperature"), Some(&json!(0.7)));
    }

    #[test]
    fn test_transform_request_does_not_override_existing() {
        let provider = create_test_provider();
        let adapter = OpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello!"}],
            "temperature": 0.9
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: serde_json::Value = serde_json::from_slice(&result.body).unwrap();

        // Existing temperature should be preserved
        assert_eq!(transformed.get("temperature"), Some(&json!(0.9)));
    }

    #[test]
    fn test_transform_request_removes_fields() {
        let provider = create_test_provider();
        let adapter = OpenAIAdapter::new(&provider);

        let request = json!({
            "model": "gpt-4",
            "messages": [{"role": "user", "content": "Hello!"}],
            "internal_field": "should be removed"
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: serde_json::Value = serde_json::from_slice(&result.body).unwrap();

        // Field should be removed
        assert!(transformed.get("internal_field").is_none());
    }

    #[test]
    fn test_transform_response_passthrough() {
        let provider = create_test_provider();
        let adapter = OpenAIAdapter::new(&provider);

        let response = json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "choices": [{
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }]
        });

        let body = Bytes::from(serde_json::to_vec(&response).unwrap());
        let ctx = TransformContext::default();

        let result = adapter.transform_response(&body, &ctx).unwrap();

        // Response should be unchanged
        assert_eq!(result.body, body);
    }
}
