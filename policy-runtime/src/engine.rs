use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Url;
use sha2::{Digest, Sha256};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::errors::PolicyRuntimeError;
use crate::types::{PolicyArtifact, PolicyBinding, PolicyDecision, PolicyKey, PolicyStage, RequestView};

wasmtime::component::bindgen!({
    path: "../policy-sdk/wit",
    world: "pre-upstream-policy",
});

#[derive(Clone)]
pub struct PolicyEngine {
    engine: Arc<Engine>,
    loaded: Arc<HashMap<PolicyKey, LoadedPolicy>>,
}

#[derive(Clone)]
struct LoadedPolicy {
    component: Arc<Component>,
}

struct PolicyStoreData {
    table: ResourceTable,
    wasi: WasiCtx,
}

impl WasiView for PolicyStoreData {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        let engine = create_engine().expect("failed to create wasmtime engine");
        Self {
            engine: Arc::new(engine),
            loaded: Arc::new(HashMap::new()),
        }
    }
}

impl PolicyEngine {
    pub fn empty() -> Self {
        Self::default()
    }

    pub async fn preload(artifacts: &[PolicyArtifact]) -> Result<Self, PolicyRuntimeError> {
        let engine = create_engine()?;
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
            let component =
                Component::new(&engine, &bytes).map_err(|err| PolicyRuntimeError::ComponentCompile {
                    id: artifact.key.id.clone(),
                    version: artifact.key.version.clone(),
                    reason: err.to_string(),
                })?;

            loaded.insert(
                artifact.key.clone(),
                LoadedPolicy {
                    component: Arc::new(component),
                },
            );
        }

        Ok(Self {
            engine: Arc::new(engine),
            loaded: Arc::new(loaded),
        })
    }

    pub fn evaluate_pre_upstream(
        &self,
        binding: &PolicyBinding,
        request: &RequestView,
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

        let mut linker = Linker::new(self.engine.as_ref());
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|err| {
            PolicyRuntimeError::ComponentInstantiate {
                id: binding.key.id.clone(),
                version: binding.key.version.clone(),
                reason: format!("failed to link wasi imports: {err}"),
            }
        })?;

        let mut store = Store::new(
            self.engine.as_ref(),
            PolicyStoreData {
                table: ResourceTable::new(),
                wasi: WasiCtxBuilder::new().build(),
            },
        );
        let bindings = PreUpstreamPolicy::instantiate(
            &mut store,
            loaded.component.as_ref(),
            &linker,
        )
        .map_err(|err: wasmtime::Error| PolicyRuntimeError::ComponentInstantiate {
            id: binding.key.id.clone(),
            version: binding.key.version.clone(),
            reason: err.to_string(),
        })?;

        let headers_json = serde_json::to_string(&request.headers).map_err(|err| {
            PolicyRuntimeError::GuestExecution {
                id: binding.key.id.clone(),
                version: binding.key.version.clone(),
                reason: format!("failed to serialize request headers: {err}"),
            }
        })?;
        let effective_config_json =
            serde_json::to_string(&binding.effective_config).map_err(|err| {
                PolicyRuntimeError::GuestExecution {
                    id: binding.key.id.clone(),
                    version: binding.key.version.clone(),
                    reason: format!("failed to serialize effective config: {err}"),
                }
            })?;

        let guest_result: Result<yali::policy::types::PolicyDecision, String> = bindings
            .yali_policy_policy()
            .call_evaluate_pre_upstream(
                &mut store,
                &request.method,
                &request.path,
                request.host.as_deref(),
                &headers_json,
                &effective_config_json,
            )
            .map_err(|err: wasmtime::Error| PolicyRuntimeError::GuestExecution {
                id: binding.key.id.clone(),
                version: binding.key.version.clone(),
                reason: err.to_string(),
            })?;

        let decision = guest_result.map_err(|reason| PolicyRuntimeError::GuestRejected {
            id: binding.key.id.clone(),
            version: binding.key.version.clone(),
            reason,
        })?;

        Ok(component_decision_to_runtime(decision))
    }
}

fn component_decision_to_runtime(decision: yali::policy::types::PolicyDecision) -> PolicyDecision {
    PolicyDecision {
        request_headers: decision
            .request_headers
            .into_iter()
            .map(component_header_to_runtime)
            .collect(),
        request_rewrite: decision
            .request_rewrite
            .map(|rewrite| crate::types::RequestRewrite {
                method: rewrite.method,
                path: rewrite.path,
            }),
        upstream_hint: decision.upstream_hint,
        direct_response: decision
            .direct_response
            .map(|response| crate::types::DirectResponse {
                status: response.status,
                headers: response
                    .headers
                    .into_iter()
                    .map(component_header_to_runtime)
                    .collect(),
                body: response.body,
            }),
        request_body_patch: decision
            .request_body_patch_json
            .and_then(|raw| serde_json::from_str(&raw).ok()),
        response_headers: decision
            .response_headers
            .into_iter()
            .map(component_header_to_runtime)
            .collect(),
    }
}

fn component_header_to_runtime(header: yali::policy::types::HeaderOp) -> crate::types::HeaderMutation {
    crate::types::HeaderMutation {
        name: header.name,
        value: header.value,
        overwrite: header.overwrite,
    }
}

fn create_engine() -> Result<Engine, PolicyRuntimeError> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    Engine::new(&config).map_err(|err| PolicyRuntimeError::ComponentCompile {
        id: "engine".to_string(),
        version: "n/a".to_string(),
        reason: err.to_string(),
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
