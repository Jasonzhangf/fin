//! Provider request block.

use fin_config::ProviderProtocol;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub input: String,
    pub rendered_input: Option<String>,
    pub override_model: Option<String>,
    #[serde(default)]
    pub prompt_cache_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRequest {
    pub provider_name: String,
    pub protocol: ProviderProtocol,
    pub endpoint: String,
    pub model: String,
    pub input: String,
    pub rendered_input: String,
    #[serde(default)]
    pub prompt_cache_key: Option<String>,
    pub user_agent: Option<String>,
    pub sanitized_headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub usage_source: String,
}
