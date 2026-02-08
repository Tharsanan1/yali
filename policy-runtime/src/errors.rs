use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyRuntimeError {
    #[error("unsupported wasm uri: {0}")]
    UnsupportedUri(String),
    #[error("failed to read policy module from {uri}: {source}")]
    ModuleRead {
        uri: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("sha256 mismatch for {id}@{version}")]
    ShaMismatch { id: String, version: String },
    #[error("invalid wasm binary for {id}@{version}: {reason}")]
    InvalidWasm {
        id: String,
        version: String,
        reason: String,
    },
    #[error("failed to compile wasm component for {id}@{version}: {reason}")]
    ComponentCompile {
        id: String,
        version: String,
        reason: String,
    },
    #[error("failed to instantiate wasm component for {id}@{version}: {reason}")]
    ComponentInstantiate {
        id: String,
        version: String,
        reason: String,
    },
    #[error("guest policy execution failed for {id}@{version}: {reason}")]
    GuestExecution {
        id: String,
        version: String,
        reason: String,
    },
    #[error("policy guest rejected request for {id}@{version}: {reason}")]
    GuestRejected {
        id: String,
        version: String,
        reason: String,
    },
    #[error("unknown policy: {id}@{version}")]
    UnknownPolicy { id: String, version: String },
    #[error("unsupported policy stage {stage} for {id}@{version}")]
    UnsupportedStage {
        stage: String,
        id: String,
        version: String,
    },
    #[error("invalid policy config for {id}@{version}: {reason}")]
    InvalidConfig {
        id: String,
        version: String,
        reason: String,
    },
    #[error("policy decision uses unsupported action: {reason}")]
    UnsupportedDecisionAction { reason: String },
}
