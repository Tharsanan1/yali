use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PolicyKey {
    pub id: String,
    pub version: String,
}

impl PolicyKey {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolicyArtifact {
    pub key: PolicyKey,
    pub wasm_uri: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyStage {
    PreRoute,
    PreUpstream,
    PostResponse,
}

impl PolicyStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreRoute => "pre_route",
            Self::PreUpstream => "pre_upstream",
            Self::PostResponse => "post_response",
        }
    }
}

impl std::str::FromStr for PolicyStage {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pre_route" => Ok(Self::PreRoute),
            "pre_upstream" => Ok(Self::PreUpstream),
            "post_response" => Ok(Self::PostResponse),
            _ => Err(format!("unsupported stage {value}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PolicyBinding {
    pub stage: PolicyStage,
    pub key: PolicyKey,
    pub effective_config: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
pub struct RequestView {
    pub method: String,
    pub path: String,
    pub host: Option<String>,
    pub headers: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct PolicyDecision {
    pub request_headers: Vec<HeaderMutation>,
    pub request_rewrite: Option<RequestRewrite>,
    pub upstream_hint: Option<String>,
    pub direct_response: Option<DirectResponse>,
    pub request_body_patch: Option<serde_json::Value>,
    pub response_headers: Vec<HeaderMutation>,
}

impl PolicyDecision {
    pub fn merge_from(&mut self, mut other: PolicyDecision) {
        self.request_headers.append(&mut other.request_headers);
        self.response_headers.append(&mut other.response_headers);
        if other.request_rewrite.is_some() {
            self.request_rewrite = other.request_rewrite;
        }
        if other.upstream_hint.is_some() {
            self.upstream_hint = other.upstream_hint;
        }
        if other.direct_response.is_some() {
            self.direct_response = other.direct_response;
        }
        if other.request_body_patch.is_some() {
            self.request_body_patch = other.request_body_patch;
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeaderMutation {
    pub name: String,
    pub value: String,
    pub overwrite: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestRewrite {
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HeaderMutation>,
    #[serde(default)]
    pub body: Option<String>,
}
