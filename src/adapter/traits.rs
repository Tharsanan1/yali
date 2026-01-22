//! Protocol Adapter Trait Definitions
//!
//! Defines the core trait that all protocol adapters must implement.

use bytes::Bytes;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during protocol adaptation.
#[derive(Debug, Clone)]
pub enum AdapterError {
    /// The request body could not be parsed.
    InvalidRequestBody(String),
    /// The response body could not be parsed.
    InvalidResponseBody(String),
    /// A required field is missing.
    MissingField(String),
    /// The transformation failed.
    TransformationFailed(String),
    /// Unsupported format or operation.
    Unsupported(String),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::InvalidRequestBody(msg) => write!(f, "Invalid request body: {}", msg),
            AdapterError::InvalidResponseBody(msg) => write!(f, "Invalid response body: {}", msg),
            AdapterError::MissingField(field) => write!(f, "Missing required field: {}", field),
            AdapterError::TransformationFailed(msg) => write!(f, "Transformation failed: {}", msg),
            AdapterError::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Context for request/response transformation.
#[derive(Debug, Clone, Default)]
pub struct TransformContext {
    /// The original model name from the request
    pub model: Option<String>,
    /// Deployment name (for Azure)
    pub deployment: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Result of request transformation.
#[derive(Debug, Clone)]
pub struct TransformedRequest {
    /// The transformed request body.
    pub body: Bytes,
    /// Additional headers to add.
    pub headers: HashMap<String, String>,
    /// The transformed URL path (if changed).
    pub path: Option<String>,
    /// Query parameters to add.
    pub query_params: HashMap<String, String>,
    /// Context to pass to response transformation.
    pub context: TransformContext,
}

/// Result of response transformation.
#[derive(Debug, Clone)]
pub struct TransformedResponse {
    /// The transformed response body.
    pub body: Bytes,
    /// Additional headers to add.
    pub headers: HashMap<String, String>,
}

/// Protocol adapter trait for transforming between AI provider formats.
///
/// All adapters transform to/from the canonical OpenAI-compatible format.
pub trait ProtocolAdapter: Send + Sync {
    /// Returns the name of this adapter (e.g., "openai", "anthropic").
    fn name(&self) -> &str;

    /// Transform an incoming request body to the provider's format.
    ///
    /// # Arguments
    /// * `body` - The incoming request body in canonical (OpenAI) format
    /// * `ctx` - Mutable context that can be used to pass information to response transform
    ///
    /// # Returns
    /// The transformed request ready to send to the provider.
    fn transform_request(
        &self,
        body: &Bytes,
        ctx: &mut TransformContext,
    ) -> Result<TransformedRequest, AdapterError>;

    /// Transform a provider's response body to canonical (OpenAI) format.
    ///
    /// # Arguments
    /// * `body` - The response body from the provider
    /// * `ctx` - Context from the request transformation
    ///
    /// # Returns
    /// The transformed response in canonical format.
    fn transform_response(
        &self,
        body: &Bytes,
        ctx: &TransformContext,
    ) -> Result<TransformedResponse, AdapterError>;

    /// Check if this adapter supports streaming responses.
    fn supports_streaming(&self) -> bool {
        true
    }

    /// Transform a streaming chunk from the provider to canonical format.
    ///
    /// # Arguments
    /// * `chunk` - A single SSE data chunk from the provider
    /// * `ctx` - Context from the request transformation
    ///
    /// # Returns
    /// The transformed chunk in canonical SSE format.
    fn transform_stream_chunk(
        &self,
        chunk: &Bytes,
        ctx: &TransformContext,
    ) -> Result<Bytes, AdapterError> {
        // Default implementation: pass through unchanged
        let _ = ctx;
        Ok(chunk.clone())
    }

    /// Get the expected Content-Type for requests to this provider.
    fn request_content_type(&self) -> &str {
        "application/json"
    }

    /// Get the expected Content-Type for responses from this provider.
    fn response_content_type(&self) -> &str {
        "application/json"
    }
}

/// Helper function to extract a string field from JSON.
#[allow(dead_code)]
pub fn extract_string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(|v| v.as_str()).map(String::from)
}

/// Helper function to extract an array field from JSON.
#[allow(dead_code)]
pub fn extract_array<'a>(value: &'a Value, field: &str) -> Option<&'a Vec<Value>> {
    value.get(field).and_then(|v| v.as_array())
}

/// Helper function to set a field in JSON if not present.
#[allow(dead_code)]
pub fn set_default(value: &mut Value, field: &str, default: Value) {
    if let Some(obj) = value.as_object_mut() {
        if !obj.contains_key(field) {
            obj.insert(field.to_string(), default);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_string() {
        let value = json!({"model": "gpt-4", "temperature": 0.7});
        assert_eq!(extract_string(&value, "model"), Some("gpt-4".to_string()));
        assert_eq!(extract_string(&value, "temperature"), None);
        assert_eq!(extract_string(&value, "missing"), None);
    }

    #[test]
    fn test_extract_array() {
        let value = json!({"messages": [{"role": "user", "content": "hi"}]});
        let arr = extract_array(&value, "messages");
        assert!(arr.is_some());
        assert_eq!(arr.unwrap().len(), 1);
    }

    #[test]
    fn test_set_default() {
        let mut value = json!({"model": "gpt-4"});
        set_default(&mut value, "temperature", json!(0.7));
        assert_eq!(value.get("temperature"), Some(&json!(0.7)));

        // Should not override existing value
        set_default(&mut value, "model", json!("gpt-3.5"));
        assert_eq!(value.get("model"), Some(&json!("gpt-4")));
    }

    #[test]
    fn test_adapter_error_display() {
        let err = AdapterError::MissingField("model".to_string());
        assert_eq!(err.to_string(), "Missing required field: model");
    }
}
