//! Anthropic Protocol Adapter
//!
//! Transforms between OpenAI format (canonical) and Anthropic Claude API format.
//!
//! Key differences:
//! - Anthropic uses `x-api-key` header instead of Bearer token
//! - System message must be extracted to a top-level `system` field
//! - `max_tokens` is required in Anthropic
//! - `stop` becomes `stop_sequences`
//! - Response format differs significantly

use super::traits::{
    AdapterError, ProtocolAdapter, TransformContext, TransformedRequest, TransformedResponse,
};
use crate::config::Provider;
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Anthropic Claude protocol adapter.
pub struct AnthropicAdapter {
    /// Default values to inject into requests.
    defaults: HashMap<String, Value>,
    /// The Anthropic API version header value.
    api_version: String,
}

impl AnthropicAdapter {
    /// Create a new Anthropic adapter from provider configuration.
    pub fn new(provider: &Provider) -> Self {
        Self {
            defaults: provider.spec.adapter.request_body.defaults.clone(),
            api_version: provider
                .spec
                .adapter
                .headers
                .add
                .get("anthropic-version")
                .cloned()
                .unwrap_or_else(|| "2023-06-01".to_string()),
        }
    }

    /// Extract system message from messages array.
    fn extract_system_message(messages: &mut Vec<Value>) -> Option<String> {
        // Find and remove system messages
        let system_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter_map(|(i, m)| {
                if m.get("role").and_then(|r| r.as_str()) == Some("system") {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        if system_indices.is_empty() {
            return None;
        }

        // Collect all system messages (in reverse order to maintain indices)
        let mut system_content = Vec::new();
        for &idx in system_indices.iter().rev() {
            if let Some(msg) = messages.remove(idx).get("content") {
                if let Some(content) = msg.as_str() {
                    system_content.insert(0, content.to_string());
                }
            }
        }

        if system_content.is_empty() {
            None
        } else {
            Some(system_content.join("\n\n"))
        }
    }
}

impl ProtocolAdapter for AnthropicAdapter {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError> {
        let mut value: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidRequestBody(e.to_string()))?;

        // Extract model for context
        if let Some(model) = value.get("model").and_then(|m| m.as_str()) {
            ctx.model = Some(model.to_string());
        }

        // Create Anthropic request structure
        let mut anthropic_request = json!({});

        // Copy model
        if let Some(model) = value.get("model") {
            anthropic_request["model"] = model.clone();
        }

        // Extract and transform messages
        if let Some(messages) = value.get_mut("messages").and_then(|m| m.as_array_mut()) {
            // Extract system message to top level
            if let Some(system) = Self::extract_system_message(messages) {
                anthropic_request["system"] = json!(system);
            }

            // Transform remaining messages (removing system messages)
            anthropic_request["messages"] = json!(messages);
        }

        // max_tokens is required for Anthropic (use default if not provided)
        if let Some(max_tokens) = value.get("max_tokens") {
            anthropic_request["max_tokens"] = max_tokens.clone();
        } else if let Some(default) = self.defaults.get("max_tokens") {
            anthropic_request["max_tokens"] = default.clone();
        } else {
            anthropic_request["max_tokens"] = json!(1024);
        }

        // Transform stop to stop_sequences
        if let Some(stop) = value.get("stop") {
            if stop.is_array() {
                anthropic_request["stop_sequences"] = stop.clone();
            } else if stop.is_string() {
                anthropic_request["stop_sequences"] = json!([stop]);
            }
        }

        // Copy temperature and top_p if present
        if let Some(temp) = value.get("temperature") {
            anthropic_request["temperature"] = temp.clone();
        }
        if let Some(top_p) = value.get("top_p") {
            anthropic_request["top_p"] = top_p.clone();
        }

        // Handle streaming
        if let Some(stream) = value.get("stream") {
            anthropic_request["stream"] = stream.clone();
        }

        // Apply any remaining defaults
        if let Some(obj) = anthropic_request.as_object_mut() {
            for (key, default_value) in &self.defaults {
                if !obj.contains_key(key) {
                    obj.insert(key.clone(), default_value.clone());
                }
            }
        }

        let body = serde_json::to_vec(&anthropic_request)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        // Add Anthropic-specific headers
        let mut headers = HashMap::new();
        headers.insert("anthropic-version".to_string(), self.api_version.clone());
        headers.insert("content-type".to_string(), "application/json".to_string());

        Ok(TransformedRequest {
            body: Bytes::from(body),
            headers,
            path: Some("/v1/messages".to_string()),
            query_params: HashMap::new(),
            context: ctx.clone(),
        })
    }

    fn transform_response(
        &self,
        body: &Bytes,
        _ctx: &TransformContext,
    ) -> Result<TransformedResponse, AdapterError> {
        let anthropic_response: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidResponseBody(e.to_string()))?;

        // Check for error response
        if anthropic_response.get("error").is_some() {
            // Pass through error responses
            return Ok(TransformedResponse {
                body: body.clone(),
                headers: HashMap::new(),
            });
        }

        // Transform to OpenAI format
        let mut openai_response = json!({
            "object": "chat.completion"
        });

        // Copy id
        if let Some(id) = anthropic_response.get("id") {
            openai_response["id"] = id.clone();
        }

        // Copy model
        if let Some(model) = anthropic_response.get("model") {
            openai_response["model"] = model.clone();
        }

        // Transform content to choices
        let mut choices = Vec::new();
        if let Some(content) = anthropic_response.get("content").and_then(|c| c.as_array()) {
            // Combine all text blocks into one message
            let text_content: Vec<&str> = content
                .iter()
                .filter_map(|block| {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        block.get("text").and_then(|t| t.as_str())
                    } else {
                        None
                    }
                })
                .collect();

            let combined_content = text_content.join("");

            choices.push(json!({
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": combined_content
                },
                "finish_reason": Self::map_stop_reason(
                    anthropic_response.get("stop_reason").and_then(|r| r.as_str())
                )
            }));
        }
        openai_response["choices"] = json!(choices);

        // Transform usage
        if let Some(usage) = anthropic_response.get("usage") {
            let mut openai_usage = json!({});
            if let Some(input) = usage.get("input_tokens") {
                openai_usage["prompt_tokens"] = input.clone();
            }
            if let Some(output) = usage.get("output_tokens") {
                openai_usage["completion_tokens"] = output.clone();
            }
            // Calculate total
            let prompt = usage
                .get("input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let completion = usage
                .get("output_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            openai_usage["total_tokens"] = json!(prompt + completion);
            openai_response["usage"] = openai_usage;
        }

        let body = serde_json::to_vec(&openai_response)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        Ok(TransformedResponse {
            body: Bytes::from(body),
            headers: HashMap::new(),
        })
    }
}

impl AnthropicAdapter {
    /// Map Anthropic stop_reason to OpenAI finish_reason.
    fn map_stop_reason(reason: Option<&str>) -> &'static str {
        match reason {
            Some("end_turn") => "stop",
            Some("stop_sequence") => "stop",
            Some("max_tokens") => "length",
            _ => "stop",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, ProviderSpec};

    fn create_test_provider() -> Provider {
        Provider {
            id: "test_anthropic".to_string(),
            name: "Test Anthropic".to_string(),
            spec: ProviderSpec {
                provider_type: "anthropic".to_string(),
                endpoint: "https://api.anthropic.com".to_string(),
                adapter: AdapterConfig::default(),
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_transform_request_basic() {
        let provider = create_test_provider();
        let adapter = AnthropicAdapter::new(&provider);

        let request = json!({
            "model": "claude-3-opus-20240229",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ],
            "max_tokens": 100
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(
            transformed.get("model"),
            Some(&json!("claude-3-opus-20240229"))
        );
        assert_eq!(transformed.get("max_tokens"), Some(&json!(100)));
        assert!(transformed.get("messages").is_some());
    }

    #[test]
    fn test_transform_request_extracts_system() {
        let provider = create_test_provider();
        let adapter = AnthropicAdapter::new(&provider);

        let request = json!({
            "model": "claude-3-opus-20240229",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ],
            "max_tokens": 100
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        // System should be at top level
        assert_eq!(
            transformed.get("system"),
            Some(&json!("You are a helpful assistant."))
        );

        // Messages should only contain non-system messages
        let messages = transformed.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get("role"), Some(&json!("user")));
    }

    #[test]
    fn test_transform_request_stop_to_stop_sequences() {
        let provider = create_test_provider();
        let adapter = AnthropicAdapter::new(&provider);

        let request = json!({
            "model": "claude-3-opus-20240229",
            "messages": [{"role": "user", "content": "Hello!"}],
            "max_tokens": 100,
            "stop": ["END", "STOP"]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(
            transformed.get("stop_sequences"),
            Some(&json!(["END", "STOP"]))
        );
    }

    #[test]
    fn test_transform_response_basic() {
        let provider = create_test_provider();
        let adapter = AnthropicAdapter::new(&provider);

        let response = json!({
            "id": "msg_123",
            "type": "message",
            "model": "claude-3-opus-20240229",
            "content": [
                {"type": "text", "text": "Hello! How can I help you?"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8
            }
        });

        let body = Bytes::from(serde_json::to_vec(&response).unwrap());
        let ctx = TransformContext::default();

        let result = adapter.transform_response(&body, &ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(transformed.get("object"), Some(&json!("chat.completion")));
        assert_eq!(transformed.get("id"), Some(&json!("msg_123")));

        let choices = transformed.get("choices").unwrap().as_array().unwrap();
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].get("finish_reason"), Some(&json!("stop")));

        let message = choices[0].get("message").unwrap();
        assert_eq!(message.get("role"), Some(&json!("assistant")));
        assert_eq!(
            message.get("content"),
            Some(&json!("Hello! How can I help you?"))
        );

        let usage = transformed.get("usage").unwrap();
        assert_eq!(usage.get("prompt_tokens"), Some(&json!(10)));
        assert_eq!(usage.get("completion_tokens"), Some(&json!(8)));
        assert_eq!(usage.get("total_tokens"), Some(&json!(18)));
    }
}
