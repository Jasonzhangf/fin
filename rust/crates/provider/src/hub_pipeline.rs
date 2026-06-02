use super::*;
use crate::blocks::descriptor::{ProviderCapabilities, ProviderDescriptor, endpoint_for_protocol};
use crate::blocks::request::{PreparedRequest, ProviderRequest, TokenUsage};
use crate::blocks::response::ProviderResponse;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubReq01Inbound {
    pub operation: ProviderRequest,
    pub descriptor: ProviderDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubReq02Process {
    pub inbound: HubReq01Inbound,
    pub resolved_model: String,
    pub resolved_input: String,
    pub resolved_rendered_input: String,
    pub resolved_cache_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubReq03Outbound {
    pub process: HubReq02Process,
    pub prepared: PreparedRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubResp04Inbound {
    pub outbound: HubReq03Outbound,
    pub status: u16,
    pub body: String,
    pub transport: HubTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubTransport {
    pub kind: &'static str,
}

impl HubTransport {
    pub fn http() -> Self {
        Self { kind: "http" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubResp05Process {
    pub inbound: HubResp04Inbound,
    pub response: ProviderResponse,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubResp06Outbound {
    pub process: HubResp05Process,
    pub wire_status: u16,
    pub wire_body: String,
    pub wire_protocol: fin_config::ProviderProtocol,
}

#[derive(Debug, Default)]
pub struct HubReq01InboundBuilder;

impl HubReq01InboundBuilder {
    pub fn build(
        &self,
        descriptor: &ProviderDescriptor,
        operation: ProviderRequest,
    ) -> Result<HubReq01Inbound, ProviderError> {
        Ok(HubReq01Inbound {
            operation,
            descriptor: descriptor.clone(),
        })
    }
}

#[derive(Debug, Default)]
pub struct HubReq02ProcessBuilder;

impl HubReq02ProcessBuilder {
    pub fn build(&self, inbound: HubReq01Inbound) -> Result<HubReq02Process, ProviderError> {
        let resolved_model = inbound
            .operation
            .override_model
            .clone()
            .unwrap_or_else(|| inbound.descriptor.default_model.clone());
        let resolved_input = inbound.operation.input.clone();
        let resolved_rendered_input = inbound
            .operation
            .rendered_input
            .clone()
            .unwrap_or_else(|| resolved_input.clone());
        let resolved_cache_key = inbound.operation.prompt_cache_key.clone();
        Ok(HubReq02Process {
            inbound,
            resolved_model,
            resolved_input,
            resolved_rendered_input,
            resolved_cache_key,
        })
    }
}

#[derive(Debug, Default)]
pub struct HubReq03OutboundBuilder;

impl HubReq03OutboundBuilder {
    pub fn build(&self, process: HubReq02Process) -> Result<HubReq03Outbound, ProviderError> {
        let prepared = PreparedRequest {
            provider_name: process.inbound.descriptor.name.clone(),
            protocol: process.inbound.descriptor.protocol,
            endpoint: endpoint_for_protocol(
                &process.inbound.descriptor.base_url,
                process.inbound.descriptor.protocol,
            ),
            model: process.resolved_model.clone(),
            input: process.resolved_input.clone(),
            rendered_input: process.resolved_rendered_input.clone(),
            prompt_cache_key: process.resolved_cache_key.clone(),
            user_agent: None,
            sanitized_headers: BTreeMap::new(),
        };
        Ok(HubReq03Outbound { process, prepared })
    }

    pub fn rebuild_from_prepared(&self, prepared: PreparedRequest) -> HubReq03Outbound {
        let operation = ProviderRequest {
            input: prepared.input.clone(),
            rendered_input: Some(prepared.rendered_input.clone()),
            override_model: Some(prepared.model.clone()),
            prompt_cache_key: prepared.prompt_cache_key.clone(),
        };
        let descriptor = ProviderDescriptor {
            name: prepared.provider_name.clone(),
            protocol: prepared.protocol,
            base_url: prepared.endpoint.clone(),
            default_model: prepared.model.clone(),
            capabilities: ProviderCapabilities::for_protocol(prepared.protocol),
        };
        HubReq03Outbound {
            process: HubReq02Process {
                inbound: HubReq01Inbound {
                    operation,
                    descriptor,
                },
                resolved_model: prepared.model.clone(),
                resolved_input: prepared.input.clone(),
                resolved_rendered_input: prepared.rendered_input.clone(),
                resolved_cache_key: prepared.prompt_cache_key.clone(),
            },
            prepared,
        }
    }
}

#[derive(Debug, Default)]
pub struct HubResp05ProcessParser;

impl HubResp05ProcessParser {
    pub fn parse(&self, inbound: HubResp04Inbound) -> Result<HubResp05Process, ProviderError> {
        let parser = match inbound.outbound.prepared.protocol {
            fin_config::ProviderProtocol::AnthropicWire => parse_anthropic_response,
            _ => parse_openai_response,
        };
        let response = parser(&inbound.outbound.prepared, inbound.status, &inbound.body)?;
        let usage = response.usage.clone();
        Ok(HubResp05Process {
            inbound,
            response,
            usage,
        })
    }
}

#[derive(Debug, Default)]
pub struct HubResp06OutboundBuilder;

impl HubResp06OutboundBuilder {
    pub fn build(&self, process: HubResp05Process) -> HubResp06Outbound {
        HubResp06Outbound {
            wire_status: process.response.status,
            wire_body: process.inbound.body.clone(),
            wire_protocol: process.inbound.outbound.prepared.protocol,
            process,
        }
    }
}

pub fn hub_inbound_response(
    outbound: HubReq03Outbound,
    status: u16,
    body: String,
    transport: HubTransport,
) -> HubResp04Inbound {
    HubResp04Inbound {
        outbound,
        status,
        body,
        transport,
    }
}
