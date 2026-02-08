use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::errors::PolicyRuntimeError;
use crate::types::{
    AddHeaderConfig, PolicyArtifact, PolicyBinding, PolicyDecision, PolicyKey, PolicyStage,
    RequestView,
};

#[derive(Clone, Default)]
pub struct PolicyEngine {
    loaded: Arc<HashMap<PolicyKey, LoadedPolicy>>,
}

#[derive(Clone)]
struct LoadedPolicy {
    key: PolicyKey,
    #[allow(dead_code)]
    wasm_bytes: Arc<Vec<u8>>,
}

impl PolicyEngine {
    pub fn empty() -> Self {
        Self::default()
    }

    pub async fn preload(artifacts: &[PolicyArtifact]) -> Result<Self, PolicyRuntimeError> {
        let mut loaded = HashMap::new();
        for artifact in artifacts {
            let bytes = load_module_bytes(&artifact.wasm_uri).await?;
            verify_sha256(&artifact.sha256, &bytes).map_err(|_| {
                PolicyRuntimeError::ShaMismatch {
                    id: artifact.key.id.clone(),
                    version: artifact.key.version.clone(),
                }
            })?;
            validate_wasm_component(&artifact.key.id, &artifact.key.version, &bytes)?;

            loaded.insert(
                artifact.key.clone(),
                LoadedPolicy {
                    key: artifact.key.clone(),
                    wasm_bytes: Arc::new(bytes),
                },
            );
        }

        Ok(Self {
            loaded: Arc::new(loaded),
        })
    }

    pub fn evaluate_pre_upstream(
        &self,
        binding: &PolicyBinding,
        _request: &RequestView,
    ) -> Result<PolicyDecision, PolicyRuntimeError> {
        let loaded =
            self.loaded
                .get(&binding.key)
                .ok_or_else(|| PolicyRuntimeError::UnknownPolicy {
                    id: binding.key.id.clone(),
                    version: binding.key.version.clone(),
                })?;

        if binding.stage != PolicyStage::PreUpstream {
            return Err(PolicyRuntimeError::UnsupportedStage {
                stage: binding.stage.as_str().to_string(),
                id: binding.key.id.clone(),
                version: binding.key.version.clone(),
            });
        }

        match loaded.key.id.as_str() {
            "add-header" => evaluate_add_header(binding),
            other => Err(PolicyRuntimeError::UnsupportedPolicy {
                id: other.to_string(),
            }),
        }
    }
}

fn evaluate_add_header(binding: &PolicyBinding) -> Result<PolicyDecision, PolicyRuntimeError> {
    let parsed: AddHeaderConfig = serde_json::from_value(binding.effective_config.clone())
        .map_err(|err| PolicyRuntimeError::InvalidConfig {
            id: binding.key.id.clone(),
            version: binding.key.version.clone(),
            reason: err.to_string(),
        })?;

    if parsed.headers.is_empty() {
        return Err(PolicyRuntimeError::InvalidConfig {
            id: binding.key.id.clone(),
            version: binding.key.version.clone(),
            reason: "headers must not be empty".to_string(),
        });
    }

    Ok(PolicyDecision {
        request_headers: parsed.headers,
        ..PolicyDecision::default()
    })
}

async fn load_module_bytes(uri: &str) -> Result<Vec<u8>, PolicyRuntimeError> {
    let parsed =
        Url::parse(uri).map_err(|_| PolicyRuntimeError::UnsupportedUri(uri.to_string()))?;
    match parsed.scheme() {
        "file" => {
            let path = parsed
                .to_file_path()
                .map_err(|_| PolicyRuntimeError::UnsupportedUri(uri.to_string()))?;
            tokio::fs::read(&path)
                .await
                .map_err(|err| PolicyRuntimeError::ModuleRead {
                    uri: uri.to_string(),
                    source: Box::new(err),
                })
        }
        "http" | "https" => {
            let response =
                reqwest::get(uri)
                    .await
                    .map_err(|err| PolicyRuntimeError::ModuleRead {
                        uri: uri.to_string(),
                        source: Box::new(err),
                    })?;
            if !response.status().is_success() {
                return Err(PolicyRuntimeError::ModuleRead {
                    uri: uri.to_string(),
                    source: format!("non-success status {}", response.status()).into(),
                });
            }
            response.bytes().await.map(|b| b.to_vec()).map_err(|err| {
                PolicyRuntimeError::ModuleRead {
                    uri: uri.to_string(),
                    source: Box::new(err),
                }
            })
        }
        "oci" => Err(PolicyRuntimeError::UnsupportedUri(
            "oci:// uri is not implemented yet".to_string(),
        )),
        _ => Err(PolicyRuntimeError::UnsupportedUri(uri.to_string())),
    }
}

fn verify_sha256(expected: &str, bytes: &[u8]) -> Result<(), ()> {
    let expected_normalized = expected
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(expected.trim())
        .to_ascii_lowercase();
    let actual = hex::encode(Sha256::digest(bytes));
    if actual == expected_normalized {
        Ok(())
    } else {
        Err(())
    }
}

fn validate_wasm_component(
    id: &str,
    version: &str,
    bytes: &[u8],
) -> Result<(), PolicyRuntimeError> {
    let mut validator = wasmparser::Validator::new();

    validator
        .validate_all(bytes)
        .map(|_| ())
        .map_err(|err| PolicyRuntimeError::InvalidWasm {
            id: id.to_string(),
            version: version.to_string(),
            reason: err.to_string(),
        })
}
