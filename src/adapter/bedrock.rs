//! AWS Bedrock Protocol Adapter
//!
//! Transforms between OpenAI format (canonical) and AWS Bedrock format.
//!
//! Key differences:
//! - Uses AWS SigV4 authentication
//! - Model is specified in URL path
//! - Uses Anthropic-like format for Claude models on Bedrock
//! - URL pattern: /model/{model}/invoke

use super::traits::{
    AdapterError, ProtocolAdapter, TransformContext, TransformedRequest, TransformedResponse,
};
use crate::config::Provider;
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::HashMap;

/// AWS Bedrock protocol adapter.
///
/// Currently supports Claude models on Bedrock which use an Anthropic-like format.
pub struct BedrockAdapter {
    /// Default values to inject into requests.
    defaults: HashMap<String, Value>,
    /// The Anthropic version for Bedrock Claude.
    anthropic_version: String,
}

impl BedrockAdapter {
    /// Create a new Bedrock adapter from provider configuration.
    pub fn new(provider: &Provider) -> Self {
        Self {
            defaults: provider.spec.adapter.request_body.defaults.clone(),
            anthropic_version: "bedrock-2023-05-31".to_string(),
        }
    }

    /// Extract system message from messages array.
    fn extract_system_message(messages: &mut Vec<Value>) -> Option<String> {
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

impl ProtocolAdapter for BedrockAdapter {
    fn name(&self) -> &str {
        "bedrock"
    }

    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError> {
        let mut value: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidRequestBody(e.to_string()))?;

        // Extract model for URL
        let model = value
            .get("model")
            .and_then(|m| m.as_str())
            .ok_or_else(|| AdapterError::MissingField("model".to_string()))?
            .to_string();
        ctx.model = Some(model.clone());

        // Build Bedrock request (Anthropic-like for Claude models)
        let mut bedrock_request = json!({
            "anthropic_version": self.anthropic_version
        });

        // Transform messages
        if let Some(messages) = value.get_mut("messages").and_then(|m| m.as_array_mut()) {
            // Extract system message
            if let Some(system) = Self::extract_system_message(messages) {
                bedrock_request["system"] = json!(system);
            }
            bedrock_request["messages"] = json!(messages);
        }

        // max_tokens is required
        if let Some(max_tokens) = value.get("max_tokens") {
            bedrock_request["max_tokens"] = max_tokens.clone();
        } else if let Some(default) = self.defaults.get("max_tokens") {
            bedrock_request["max_tokens"] = default.clone();
        } else {
            bedrock_request["max_tokens"] = json!(1024);
        }

        // Transform stop to stop_sequences
        if let Some(stop) = value.get("stop") {
            if stop.is_array() {
                bedrock_request["stop_sequences"] = stop.clone();
            } else if stop.is_string() {
                bedrock_request["stop_sequences"] = json!([stop]);
            }
        }

        // Copy temperature and top_p
        if let Some(temp) = value.get("temperature") {
            bedrock_request["temperature"] = temp.clone();
        }
        if let Some(top_p) = value.get("top_p") {
            bedrock_request["top_p"] = top_p.clone();
        }

        // Apply remaining defaults
        if let Some(obj) = bedrock_request.as_object_mut() {
            for (key, default_value) in &self.defaults {
                if !obj.contains_key(key) {
                    obj.insert(key.clone(), default_value.clone());
                }
            }
        }

        let body = serde_json::to_vec(&bedrock_request)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        // Build the Bedrock path
        let path = format!("/model/{}/invoke", model);

        Ok(TransformedRequest {
            body: Bytes::from(body),
            headers: HashMap::new(),
            path: Some(path),
            query_params: HashMap::new(),
            context: ctx.clone(),
        })
    }

    fn transform_response(
        &self,
        body: &Bytes,
        ctx: &TransformContext,
    ) -> Result<TransformedResponse, AdapterError> {
        let bedrock_response: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidResponseBody(e.to_string()))?;

        // Check for error
        if bedrock_response.get("error").is_some() {
            return Ok(TransformedResponse {
                body: body.clone(),
                headers: HashMap::new(),
            });
        }

        // Transform to OpenAI format (similar to Anthropic transform)
        let mut openai_response = json!({
            "object": "chat.completion"
        });

        // Use model from context
        if let Some(model) = &ctx.model {
            openai_response["model"] = json!(model);
        }

        // Generate an ID
        openai_response["id"] = json!(format!(
            "chatcmpl-bedrock-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));

        // Transform content to choices
        let mut choices = Vec::new();
        if let Some(content) = bedrock_response.get("content").and_then(|c| c.as_array()) {
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

            let finish_reason = match bedrock_response.get("stop_reason").and_then(|r| r.as_str()) {
                Some("end_turn") => "stop",
                Some("stop_sequence") => "stop",
                Some("max_tokens") => "length",
                _ => "stop",
            };

            choices.push(json!({
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": combined_content
                },
                "finish_reason": finish_reason
            }));
        }
        openai_response["choices"] = json!(choices);

        // Transform usage
        if let Some(usage) = bedrock_response.get("usage") {
            let mut openai_usage = json!({});
            if let Some(input) = usage.get("input_tokens") {
                openai_usage["prompt_tokens"] = input.clone();
            }
            if let Some(output) = usage.get("output_tokens") {
                openai_usage["completion_tokens"] = output.clone();
            }
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

    fn supports_streaming(&self) -> bool {
        // Bedrock streaming uses a different endpoint
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, ProviderSpec};

    fn create_test_provider() -> Provider {
        Provider {
            id: "test_bedrock".to_string(),
            name: "Test Bedrock".to_string(),
            spec: ProviderSpec {
                provider_type: "bedrock".to_string(),
                endpoint: "https://bedrock-runtime.us-east-1.amazonaws.com".to_string(),
                adapter: AdapterConfig::default(),
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_transform_request_basic() {
        let provider = create_test_provider();
        let adapter = BedrockAdapter::new(&provider);

        let request = json!({
            "model": "anthropic.claude-3-sonnet-20240229-v1:0",
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
            transformed.get("anthropic_version"),
            Some(&json!("bedrock-2023-05-31"))
        );
        assert_eq!(transformed.get("max_tokens"), Some(&json!(100)));

        // Check path
        assert_eq!(
            result.path,
            Some("/model/anthropic.claude-3-sonnet-20240229-v1:0/invoke".to_string())
        );
    }

    #[test]
    fn test_transform_request_extracts_system() {
        let provider = create_test_provider();
        let adapter = BedrockAdapter::new(&provider);

        let request = json!({
            "model": "anthropic.claude-3-sonnet-20240229-v1:0",
            "messages": [
                {"role": "system", "content": "You are helpful."},
                {"role": "user", "content": "Hello!"}
            ],
            "max_tokens": 100
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(transformed.get("system"), Some(&json!("You are helpful.")));

        let messages = transformed.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get("role"), Some(&json!("user")));
    }

    #[test]
    fn test_transform_response_basic() {
        let provider = create_test_provider();
        let adapter = BedrockAdapter::new(&provider);

        let response = json!({
            "content": [
                {"type": "text", "text": "Hello! How can I help?"}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8
            }
        });

        let body = Bytes::from(serde_json::to_vec(&response).unwrap());
        let ctx = TransformContext {
            model: Some("anthropic.claude-3-sonnet-20240229-v1:0".to_string()),
            ..Default::default()
        };

        let result = adapter.transform_response(&body, &ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(transformed.get("object"), Some(&json!("chat.completion")));

        let choices = transformed.get("choices").unwrap().as_array().unwrap();
        assert_eq!(choices.len(), 1);

        let message = choices[0].get("message").unwrap();
        assert_eq!(message.get("role"), Some(&json!("assistant")));
        assert_eq!(
            message.get("content"),
            Some(&json!("Hello! How can I help?"))
        );

        assert_eq!(choices[0].get("finish_reason"), Some(&json!("stop")));

        let usage = transformed.get("usage").unwrap();
        assert_eq!(usage.get("prompt_tokens"), Some(&json!(10)));
        assert_eq!(usage.get("completion_tokens"), Some(&json!(8)));
        assert_eq!(usage.get("total_tokens"), Some(&json!(18)));
    }
}
