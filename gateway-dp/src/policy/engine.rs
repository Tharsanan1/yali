use policy_runtime::PolicyEngine;

use super::errors::PolicyRuntimeError;
use super::types::{PolicyArtifact, PolicyBinding, PolicyDecision, PolicyStage, RequestView};

#[derive(Clone, Default)]
pub struct PolicyRegistry {
    runtime: PolicyEngine,
}

impl PolicyRegistry {
    pub fn empty() -> Self {
        Self {
            runtime: PolicyEngine::empty(),
        }
    }

    pub async fn preload(artifacts: &[PolicyArtifact]) -> Result<Self, PolicyRuntimeError> {
        let runtime = PolicyEngine::preload(artifacts).await?;
        Ok(Self { runtime })
    }

    pub fn evaluate_pre_upstream(
        &self,
        bindings: &[PolicyBinding],
        request: &RequestView,
    ) -> Result<PolicyDecision, PolicyRuntimeError> {
        let mut combined = PolicyDecision::default();
        for binding in bindings {
            if binding.stage != PolicyStage::PreUpstream {
                continue;
            }
            let decision = self.runtime.evaluate_pre_upstream(binding, request)?;
            combined.merge_from(decision);
        }
        Ok(combined)
    }
}
