//! Google AI (Gemini) Protocol Adapter
//!
//! Transforms between OpenAI format (canonical) and Google AI (Gemini) API format.
//!
//! Key differences:
//! - Uses `key` query parameter for authentication
//! - Messages become `contents` with different structure
//! - `messages[].content` becomes `contents[].parts[].text`
//! - Role `assistant` becomes `model`
//! - `temperature`, `top_p`, `max_tokens`, `stop` go into `generationConfig`
//! - URL pattern: /v1beta/models/{model}:generateContent

use super::traits::{
    AdapterError, ProtocolAdapter, TransformContext, TransformedRequest, TransformedResponse,
};
use crate::config::Provider;
use bytes::Bytes;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Google AI (Gemini) protocol adapter.
pub struct GoogleAIAdapter {
    /// Default values to inject into requests.
    defaults: HashMap<String, Value>,
}

impl GoogleAIAdapter {
    /// Create a new Google AI adapter from provider configuration.
    pub fn new(provider: &Provider) -> Self {
        Self {
            defaults: provider.spec.adapter.request_body.defaults.clone(),
        }
    }

    /// Map OpenAI role to Google AI role.
    fn map_role(role: &str) -> &str {
        match role {
            "assistant" => "model",
            "system" => "user", // Google handles system differently
            _ => role,
        }
    }

    /// Map Google AI role back to OpenAI role.
    fn unmap_role(role: &str) -> &str {
        match role {
            "model" => "assistant",
            _ => role,
        }
    }
}

impl ProtocolAdapter for GoogleAIAdapter {
    fn name(&self) -> &str {
        "google_ai"
    }

    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError> {
        let value: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidRequestBody(e.to_string()))?;

        // Extract model for URL
        let model = value
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("gemini-pro");
        ctx.model = Some(model.to_string());

        // Build Google AI request
        let mut google_request = json!({});

        // Transform messages to contents
        if let Some(messages) = value.get("messages").and_then(|m| m.as_array()) {
            let mut contents: Vec<Value> = Vec::new();
            let mut system_instruction: Option<String> = None;

            for msg in messages {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
                let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

                // Handle system message specially
                if role == "system" {
                    system_instruction = Some(content.to_string());
                    continue;
                }

                contents.push(json!({
                    "role": Self::map_role(role),
                    "parts": [{"text": content}]
                }));
            }

            google_request["contents"] = json!(contents);

            // Add system instruction if present
            if let Some(sys) = system_instruction {
                google_request["systemInstruction"] = json!({
                    "parts": [{"text": sys}]
                });
            }
        }

        // Build generationConfig
        let mut gen_config = json!({});

        if let Some(temp) = value.get("temperature") {
            gen_config["temperature"] = temp.clone();
        } else if let Some(temp) = self.defaults.get("temperature") {
            gen_config["temperature"] = temp.clone();
        }

        if let Some(top_p) = value.get("top_p") {
            gen_config["topP"] = top_p.clone();
        }

        if let Some(max_tokens) = value.get("max_tokens") {
            gen_config["maxOutputTokens"] = max_tokens.clone();
        } else if let Some(max_tokens) = self.defaults.get("max_tokens") {
            gen_config["maxOutputTokens"] = max_tokens.clone();
        }

        if let Some(stop) = value.get("stop") {
            if stop.is_array() {
                gen_config["stopSequences"] = stop.clone();
            } else if stop.is_string() {
                gen_config["stopSequences"] = json!([stop]);
            }
        }

        if gen_config
            .as_object()
            .map(|o| !o.is_empty())
            .unwrap_or(false)
        {
            google_request["generationConfig"] = gen_config;
        }

        let body = serde_json::to_vec(&google_request)
            .map_err(|e| AdapterError::TransformationFailed(e.to_string()))?;

        // Build the path
        let path = format!("/v1beta/models/{}:generateContent", model);

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
        let google_response: Value = serde_json::from_slice(body)
            .map_err(|e| AdapterError::InvalidResponseBody(e.to_string()))?;

        // Check for error response
        if google_response.get("error").is_some() {
            return Ok(TransformedResponse {
                body: body.clone(),
                headers: HashMap::new(),
            });
        }

        // Transform to OpenAI format
        let mut openai_response = json!({
            "object": "chat.completion"
        });

        // Use model from context
        if let Some(model) = &ctx.model {
            openai_response["model"] = json!(model);
        }

        // Transform candidates to choices
        let mut choices = Vec::new();
        if let Some(candidates) = google_response.get("candidates").and_then(|c| c.as_array()) {
            for (i, candidate) in candidates.iter().enumerate() {
                // Extract content
                let content = candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                    .and_then(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .first()
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();

                let role = candidate
                    .get("content")
                    .and_then(|c| c.get("role"))
                    .and_then(|r| r.as_str())
                    .map(Self::unmap_role)
                    .unwrap_or("assistant");

                // Map finish reason
                let finish_reason = candidate
                    .get("finishReason")
                    .and_then(|r| r.as_str())
                    .map(|r| match r {
                        "STOP" => "stop",
                        "MAX_TOKENS" => "length",
                        "SAFETY" => "content_filter",
                        _ => "stop",
                    })
                    .unwrap_or("stop");

                choices.push(json!({
                    "index": i,
                    "message": {
                        "role": role,
                        "content": content
                    },
                    "finish_reason": finish_reason
                }));
            }
        }
        openai_response["choices"] = json!(choices);

        // Transform usage metadata
        if let Some(usage) = google_response.get("usageMetadata") {
            let mut openai_usage = json!({});
            if let Some(prompt) = usage.get("promptTokenCount") {
                openai_usage["prompt_tokens"] = prompt.clone();
            }
            if let Some(completion) = usage.get("candidatesTokenCount") {
                openai_usage["completion_tokens"] = completion.clone();
            }
            if let Some(total) = usage.get("totalTokenCount") {
                openai_usage["total_tokens"] = total.clone();
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, ProviderSpec};

    fn create_test_provider() -> Provider {
        Provider {
            id: "test_google".to_string(),
            name: "Test Google AI".to_string(),
            spec: ProviderSpec {
                provider_type: "google_ai".to_string(),
                endpoint: "https://generativelanguage.googleapis.com".to_string(),
                adapter: AdapterConfig::default(),
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_transform_request_basic() {
        let provider = create_test_provider();
        let adapter = GoogleAIAdapter::new(&provider);

        let request = json!({
            "model": "gemini-pro",
            "messages": [
                {"role": "user", "content": "Hello!"}
            ]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        // Check contents structure
        let contents = transformed.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].get("role"), Some(&json!("user")));

        let parts = contents[0].get("parts").unwrap().as_array().unwrap();
        assert_eq!(parts[0].get("text"), Some(&json!("Hello!")));

        // Check path
        assert_eq!(
            result.path,
            Some("/v1beta/models/gemini-pro:generateContent".to_string())
        );
    }

    #[test]
    fn test_transform_request_with_system() {
        let provider = create_test_provider();
        let adapter = GoogleAIAdapter::new(&provider);

        let request = json!({
            "model": "gemini-pro",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        // System should be in systemInstruction
        let sys = transformed.get("systemInstruction").unwrap();
        let parts = sys.get("parts").unwrap().as_array().unwrap();
        assert_eq!(
            parts[0].get("text"),
            Some(&json!("You are a helpful assistant."))
        );

        // Contents should not include system message
        let contents = transformed.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
    }

    #[test]
    fn test_transform_request_role_mapping() {
        let provider = create_test_provider();
        let adapter = GoogleAIAdapter::new(&provider);

        let request = json!({
            "model": "gemini-pro",
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": "Hello!"},
                {"role": "user", "content": "How are you?"}
            ]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        let contents = transformed.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents[0].get("role"), Some(&json!("user")));
        assert_eq!(contents[1].get("role"), Some(&json!("model"))); // assistant -> model
        assert_eq!(contents[2].get("role"), Some(&json!("user")));
    }

    #[test]
    fn test_transform_request_generation_config() {
        let provider = create_test_provider();
        let adapter = GoogleAIAdapter::new(&provider);

        let request = json!({
            "model": "gemini-pro",
            "messages": [{"role": "user", "content": "Hi"}],
            "temperature": 0.7,
            "top_p": 0.9,
            "max_tokens": 100,
            "stop": ["END"]
        });

        let body = Bytes::from(serde_json::to_vec(&request).unwrap());
        let mut ctx = TransformContext::default();

        let result = adapter.transform_request(&body, &mut ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        let config = transformed.get("generationConfig").unwrap();
        assert_eq!(config.get("temperature"), Some(&json!(0.7)));
        assert_eq!(config.get("topP"), Some(&json!(0.9)));
        assert_eq!(config.get("maxOutputTokens"), Some(&json!(100)));
        assert_eq!(config.get("stopSequences"), Some(&json!(["END"])));
    }

    #[test]
    fn test_transform_response_basic() {
        let provider = create_test_provider();
        let adapter = GoogleAIAdapter::new(&provider);

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello! How can I help you?"}],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 8,
                "totalTokenCount": 18
            }
        });

        let body = Bytes::from(serde_json::to_vec(&response).unwrap());
        let ctx = TransformContext {
            model: Some("gemini-pro".to_string()),
            ..Default::default()
        };

        let result = adapter.transform_response(&body, &ctx).unwrap();
        let transformed: Value = serde_json::from_slice(&result.body).unwrap();

        assert_eq!(transformed.get("object"), Some(&json!("chat.completion")));
        assert_eq!(transformed.get("model"), Some(&json!("gemini-pro")));

        let choices = transformed.get("choices").unwrap().as_array().unwrap();
        assert_eq!(choices.len(), 1);

        let message = choices[0].get("message").unwrap();
        assert_eq!(message.get("role"), Some(&json!("assistant")));
        assert_eq!(
            message.get("content"),
            Some(&json!("Hello! How can I help you?"))
        );

        assert_eq!(choices[0].get("finish_reason"), Some(&json!("stop")));

        let usage = transformed.get("usage").unwrap();
        assert_eq!(usage.get("prompt_tokens"), Some(&json!(10)));
        assert_eq!(usage.get("completion_tokens"), Some(&json!(8)));
        assert_eq!(usage.get("total_tokens"), Some(&json!(18)));
    }
}
