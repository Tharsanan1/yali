wit_bindgen::generate!({
    path: "../../policy-sdk/wit",
    world: "pre-upstream-policy",
});

use serde::Deserialize;

struct AddHeaderPolicy;

#[derive(Deserialize)]
struct HeaderOpCfg {
    name: String,
    value: String,
    overwrite: bool,
}

#[derive(Deserialize)]
struct DecisionCfg {
    #[serde(default)]
    headers: Vec<HeaderOpCfg>,
    #[serde(default)]
    request_headers: Vec<HeaderOpCfg>,
}

#[derive(Deserialize)]
struct WrappedDecisionCfg {
    #[serde(default)]
    policy_decision: Option<DecisionCfg>,
    #[serde(default)]
    headers: Vec<HeaderOpCfg>,
    #[serde(default)]
    request_headers: Vec<HeaderOpCfg>,
}

impl exports::yali::policy::policy::Guest for AddHeaderPolicy {
    fn evaluate_pre_upstream(
        _method: String,
        _path: String,
        _host: Option<String>,
        _headers_json: String,
        effective_config_json: String,
    ) -> Result<yali::policy::types::PolicyDecision, String> {
        let parsed = serde_json::from_str::<WrappedDecisionCfg>(&effective_config_json)
            .map_err(|err| format!("invalid effective config: {err}"))?;

        let cfg = parsed.policy_decision.unwrap_or(DecisionCfg {
            headers: parsed.headers,
            request_headers: parsed.request_headers,
        });

        let mut out_headers = cfg.request_headers;
        out_headers.extend(cfg.headers);
        if out_headers.is_empty() {
            return Err("headers must not be empty".to_string());
        }

        let request_headers = out_headers
            .into_iter()
            .map(|h| yali::policy::types::HeaderOp {
                name: h.name,
                value: h.value,
                overwrite: h.overwrite,
            })
            .collect();

        Ok(yali::policy::types::PolicyDecision {
            request_headers,
            request_rewrite: None,
            upstream_hint: None,
            direct_response: None,
            request_body_patch_json: None,
            response_headers: Vec::new(),
        })
    }
}

export!(AddHeaderPolicy);
