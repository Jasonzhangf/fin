//! Provider descriptor and capabilities block.

use fin_config::{ProviderProtocol, ResolvedProviderConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_tool_calls: bool,
}

impl ProviderCapabilities {
    pub fn for_protocol(protocol: ProviderProtocol) -> Self {
        match protocol {
            ProviderProtocol::OpenAiCompatible => Self {
                supports_streaming: true,
                supports_tool_calls: true,
            },
            ProviderProtocol::AnthropicWire => Self {
                supports_streaming: true,
                supports_tool_calls: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub name: String,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub capabilities: ProviderCapabilities,
}

impl ProviderDescriptor {
    pub fn from_resolved(config: &ResolvedProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            protocol: config.protocol,
            base_url: config.base_url.clone(),
            default_model: config.model.clone(),
            capabilities: ProviderCapabilities::for_protocol(config.protocol),
        }
    }
}

pub fn endpoint_for_protocol(base_url: &str, protocol: ProviderProtocol) -> String {
    let base = base_url.trim_end_matches('/');
    match protocol {
        ProviderProtocol::AnthropicWire => format!("{base}/messages"),
        ProviderProtocol::OpenAiCompatible => format!("{base}/chat/completions"),
    }
}
