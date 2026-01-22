//! Protocol Adapter Module
//!
//! This module provides protocol adaptation capabilities for transforming
//! requests and responses between different AI provider formats.
//!
//! The gateway uses OpenAI-compatible format as the canonical internal format.
//! Adapters transform to/from provider-specific formats.

mod anthropic;
mod azure;
mod bedrock;
mod google_ai;
mod openai;
mod traits;

pub use anthropic::AnthropicAdapter;
pub use azure::AzureOpenAIAdapter;
pub use bedrock::BedrockAdapter;
pub use google_ai::GoogleAIAdapter;
pub use openai::OpenAIAdapter;
pub use traits::{AdapterError, ProtocolAdapter};

use crate::config::Provider;
use std::sync::Arc;

/// Factory function to create the appropriate adapter based on provider type.
pub fn create_adapter(provider: &Provider) -> Arc<dyn ProtocolAdapter> {
    match provider.spec.provider_type.as_str() {
        "openai" => Arc::new(OpenAIAdapter::new(provider)),
        "azure_openai" => Arc::new(AzureOpenAIAdapter::new(provider)),
        "anthropic" => Arc::new(AnthropicAdapter::new(provider)),
        "google_ai" => Arc::new(GoogleAIAdapter::new(provider)),
        "bedrock" => Arc::new(BedrockAdapter::new(provider)),
        _ => Arc::new(OpenAIAdapter::new(provider)), // Default to OpenAI for custom/unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdapterConfig, Provider, ProviderSpec};

    fn create_test_provider(provider_type: &str) -> Provider {
        Provider {
            id: format!("provider_{}", provider_type),
            name: format!("{} Provider", provider_type),
            spec: ProviderSpec {
                provider_type: provider_type.to_string(),
                endpoint: "https://example.com".to_string(),
                adapter: AdapterConfig::default(),
                connection_pool: Default::default(),
            },
        }
    }

    #[test]
    fn test_create_adapter_openai() {
        let provider = create_test_provider("openai");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "openai");
    }

    #[test]
    fn test_create_adapter_anthropic() {
        let provider = create_test_provider("anthropic");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "anthropic");
    }

    #[test]
    fn test_create_adapter_azure() {
        let provider = create_test_provider("azure_openai");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "azure_openai");
    }

    #[test]
    fn test_create_adapter_google_ai() {
        let provider = create_test_provider("google_ai");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "google_ai");
    }

    #[test]
    fn test_create_adapter_bedrock() {
        let provider = create_test_provider("bedrock");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "bedrock");
    }

    #[test]
    fn test_create_adapter_custom_defaults_to_openai() {
        let provider = create_test_provider("custom");
        let adapter = create_adapter(&provider);
        assert_eq!(adapter.name(), "openai");
    }
}
