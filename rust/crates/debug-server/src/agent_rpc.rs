use crate::DebugDataError;
use fin_runtime::{
    AgentControlStore, AgentKind, CapabilityDescriptor, RegisterPrimaryAgentInput, SendAgentInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRpcConfig {
    pub bind_addr: String,
    pub bearer_token: String,
    pub lease_ttl_ms: u64,
    pub heartbeat_ttl_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLeaseRecord {
    pub lease_id: String,
    pub agent_id: String,
    pub auth_subject: String,
    pub issued_at_ms: u128,
    pub expires_at_ms: u128,
    pub last_heartbeat_at_ms: u128,
    pub remote_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentLeaseRegistry {
    pub leases: Vec<AgentLeaseRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AgentHandshakeRequest {
    agent_id: String,
    kind: AgentKind,
    #[serde(default)]
    project_id: Option<String>,
    machine: String,
    agent_name: String,
    #[serde(default)]
    auth_subject: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AgentHeartbeatRequest {
    agent_id: String,
    lease_id: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct AgentMailboxSendRequest {
    message_id: String,
    from_agent_id: String,
    to_agent_id: String,
    lease_id: String,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    trigger_turn: bool,
    payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AgentRunStatusRequest {
    agent_id: String,
    lease_id: String,
    agent_run_id: String,
    status: String,
    #[serde(default)]
    result_refs: Vec<String>,
}

pub fn serve_agent_rpc(
    runtime_home: PathBuf,
    config: AgentRpcConfig,
) -> Result<(), DebugDataError> {
    let listener =
        TcpListener::bind(config.bind_addr.as_str()).map_err(|source| DebugDataError::Io {
            path: config.bind_addr.clone(),
            source,
        })?;
    serve_agent_rpc_on_listener(runtime_home, listener, config)
}

pub fn serve_agent_rpc_on_listener(
    runtime_home: PathBuf,
    listener: TcpListener,
    config: AgentRpcConfig,
) -> Result<(), DebugDataError> {
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|source| DebugDataError::Io {
            path: "agent-rpc-listener".into(),
            source,
        })?;
        let runtime_home = runtime_home.clone();
        let config = config.clone();
        thread::spawn(move || {
            let _ = handle_agent_rpc_connection(&mut stream, &runtime_home, &config);
        });
    }
    Ok(())
}

fn handle_agent_rpc_connection(
    stream: &mut TcpStream,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> Result<(), DebugDataError> {
    let request = read_request(stream)?;
    let response = response_for_agent_rpc_request(&request, runtime_home, config);
    write_response(
        stream,
        response.status_code,
        "application/json; charset=utf-8",
        &response.body,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentRpcRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Vec<u8>,
    remote_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentRpcResponse {
    status_code: u16,
    body: Vec<u8>,
}

fn response_for_agent_rpc_request(
    request: &AgentRpcRequest,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> AgentRpcResponse {
    if !auth_ok(request.authorization.as_deref(), &config.bearer_token) {
        return json_response(401, json!({"ok": false, "error": "auth_failed"}));
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/agent/v1/handshake") => handshake_response(request, runtime_home, config),
        ("POST", "/agent/v1/heartbeat") => heartbeat_response(request, runtime_home, config),
        ("GET", "/agent/v1/agents") => agents_response(runtime_home, config),
        ("POST", "/agent/v1/mailbox/send") => mailbox_send_response(request, runtime_home, config),
        ("POST", "/agent/v1/run/status") => run_status_response(request, runtime_home, config),
        _ => json_response(404, json!({"ok": false, "error": "not_found"})),
    }
}

fn handshake_response(
    request: &AgentRpcRequest,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> AgentRpcResponse {
    let payload = match serde_json::from_slice::<AgentHandshakeRequest>(&request.body) {
        Ok(value) => value,
        Err(err) => {
            return json_response(
                400,
                json!({"ok": false, "error": format!("invalid_body: {err}")}),
            );
        }
    };
    let expected_agent_id = format!("{}.{}", payload.machine.trim(), payload.agent_name.trim());
    if payload.agent_id != expected_agent_id {
        return json_response(
            400,
            json!({"ok": false, "error": "agent_id_must_match_machine_agentname"}),
        );
    }
    if matches!(payload.kind, AgentKind::Subagent) {
        return json_response(
            400,
            json!({"ok": false, "error": "network_handshake_accepts_primary_agents_only"}),
        );
    }
    if matches!(payload.kind, AgentKind::ProjectAgent)
        && payload
            .project_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return json_response(
            400,
            json!({"ok": false, "error": "project_agent_requires_project_id"}),
        );
    }
    if let Err(err) = reject_online_conflict(runtime_home, payload.agent_id.as_str()) {
        return json_response(409, json!({"ok": false, "error": err}));
    }
    let now = now_ms();
    let lease_id = format!("lease-{}-{}", sanitize_id(payload.agent_id.as_str()), now);
    let auth_subject = payload
        .auth_subject
        .clone()
        .unwrap_or_else(|| format!("agent:{}", payload.agent_id));
    let store = AgentControlStore::new(runtime_home);
    let identity = match store.register_primary_agent(RegisterPrimaryAgentInput {
        agent_id: payload.agent_id.clone(),
        kind: payload.kind.clone(),
        project_id: payload.project_id.clone(),
        device_binding: payload.machine.clone(),
        auth_subject: auth_subject.clone(),
        auth_lease_id: lease_id.clone(),
        capability_descriptor: CapabilityDescriptor {
            capability_ids: payload.capabilities.clone(),
            tool_allowlist: payload.tools.clone(),
        },
        now: now.to_string(),
    }) {
        Ok(value) => value,
        Err(err) => return json_response(400, json!({"ok": false, "error": err})),
    };
    if let Err(err) = upsert_lease(
        runtime_home,
        AgentLeaseRecord {
            lease_id: lease_id.clone(),
            agent_id: payload.agent_id.clone(),
            auth_subject,
            issued_at_ms: now,
            expires_at_ms: now.saturating_add(config.lease_ttl_ms as u128),
            last_heartbeat_at_ms: now,
            remote_addr: request.remote_addr.clone().or(payload.endpoint.clone()),
        },
    ) {
        return json_response(500, json!({"ok": false, "error": err}));
    }
    if let Err(err) =
        upsert_agent_presence(runtime_home, &payload, "idle", "network_registered", now)
    {
        return json_response(500, json!({"ok": false, "error": err}));
    }
    if let Err(err) =
        upsert_peer_registry(runtime_home, &payload, "online", "network_registered", now)
    {
        return json_response(500, json!({"ok": false, "error": err}));
    }
    json_response(
        200,
        json!({
            "ok": true,
            "lease_id": lease_id,
            "agent_id": identity.agent_id,
            "path": identity.path,
            "expires_at_ms": now.saturating_add(config.lease_ttl_ms as u128),
        }),
    )
}

fn heartbeat_response(
    request: &AgentRpcRequest,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> AgentRpcResponse {
    let payload = match serde_json::from_slice::<AgentHeartbeatRequest>(&request.body) {
        Ok(value) => value,
        Err(err) => {
            return json_response(
                400,
                json!({"ok": false, "error": format!("invalid_body: {err}")}),
            );
        }
    };
    let now = now_ms();
    match refresh_lease(
        runtime_home,
        payload.agent_id.as_str(),
        payload.lease_id.as_str(),
        now,
        config,
    ) {
        Ok(lease) => json_response(
            200,
            json!({"ok": true, "agent_id": payload.agent_id, "expires_at_ms": lease.expires_at_ms}),
        ),
        Err(err) => json_response(403, json!({"ok": false, "error": err})),
    }
}

fn agents_response(runtime_home: &Path, config: &AgentRpcConfig) -> AgentRpcResponse {
    let now = now_ms();
    let registry = read_lease_registry(runtime_home).unwrap_or_default();
    let agents = registry
        .leases
        .into_iter()
        .map(|lease| {
            let status = if lease.expires_at_ms >= now
                && now.saturating_sub(lease.last_heartbeat_at_ms) <= config.heartbeat_ttl_ms as u128
            {
                "online"
            } else {
                "offline"
            };
            json!({
                "agent_id": lease.agent_id,
                "status": status,
                "lease_id": lease.lease_id,
                "auth_subject": lease.auth_subject,
                "last_heartbeat_at_ms": lease.last_heartbeat_at_ms,
                "expires_at_ms": lease.expires_at_ms,
            })
        })
        .collect::<Vec<_>>();
    json_response(200, json!({"ok": true, "agents": agents}))
}

fn mailbox_send_response(
    request: &AgentRpcRequest,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> AgentRpcResponse {
    let payload = match serde_json::from_slice::<AgentMailboxSendRequest>(&request.body) {
        Ok(value) => value,
        Err(err) => {
            return json_response(
                400,
                json!({"ok": false, "error": format!("invalid_body: {err}")}),
            );
        }
    };
    let now = now_ms();
    if let Err(err) = refresh_lease(
        runtime_home,
        payload.from_agent_id.as_str(),
        payload.lease_id.as_str(),
        now,
        config,
    ) {
        return json_response(403, json!({"ok": false, "error": err}));
    }
    let store = AgentControlStore::new(runtime_home);
    match store.send_agent_input(SendAgentInput {
        message_id: payload.message_id,
        from_agent_id: payload.from_agent_id,
        to_agent_id: payload.to_agent_id,
        thread_id: payload.thread_id,
        task_id: payload.task_id,
        trigger_turn: payload.trigger_turn,
        payload: payload.payload,
    }) {
        Ok(message) => json_response(
            200,
            json!({"ok": true, "message_id": message.message_id, "seq": message.seq}),
        ),
        Err(err) => json_response(400, json!({"ok": false, "error": err})),
    }
}

fn run_status_response(
    request: &AgentRpcRequest,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> AgentRpcResponse {
    let payload = match serde_json::from_slice::<AgentRunStatusRequest>(&request.body) {
        Ok(value) => value,
        Err(err) => {
            return json_response(
                400,
                json!({"ok": false, "error": format!("invalid_body: {err}")}),
            );
        }
    };
    if !matches!(
        payload.status.as_str(),
        "running" | "completed" | "failed" | "timeout" | "closed"
    ) {
        return json_response(400, json!({"ok": false, "error": "invalid_run_status"}));
    }
    let now = now_ms();
    if let Err(err) = refresh_lease(
        runtime_home,
        payload.agent_id.as_str(),
        payload.lease_id.as_str(),
        now,
        config,
    ) {
        return json_response(403, json!({"ok": false, "error": err}));
    }
    let store = AgentControlStore::new(runtime_home);
    if store.wait_agent(payload.agent_run_id.as_str()).is_err() {
        if let Err(err) = store.resume_agent(
            payload.agent_id.as_str(),
            Some(payload.agent_run_id.as_str()),
            &now.to_string(),
        ) {
            return json_response(400, json!({"ok": false, "error": err}));
        }
    }
    match store.update_run_status(
        payload.agent_run_id.as_str(),
        payload.status.as_str(),
        payload.result_refs,
        &now.to_string(),
    ) {
        Ok(run) => json_response(
            200,
            json!({"ok": true, "agent_run_id": run.agent_run_id, "status": run.status, "result_refs": run.result_refs}),
        ),
        Err(err) => json_response(400, json!({"ok": false, "error": err})),
    }
}

fn auth_ok(authorization: Option<&str>, token: &str) -> bool {
    authorization
        .and_then(|value| value.trim().strip_prefix("Bearer "))
        .map(|value| value == token)
        .unwrap_or(false)
}

fn reject_online_conflict(runtime_home: &Path, agent_id: &str) -> Result<(), String> {
    let now = now_ms();
    let registry = read_lease_registry(runtime_home)?;
    if registry
        .leases
        .iter()
        .any(|lease| lease.agent_id == agent_id && lease.expires_at_ms >= now)
    {
        return Err("agent_already_online".into());
    }
    Ok(())
}

fn refresh_lease(
    runtime_home: &Path,
    agent_id: &str,
    lease_id: &str,
    now: u128,
    config: &AgentRpcConfig,
) -> Result<AgentLeaseRecord, String> {
    let mut registry = read_lease_registry(runtime_home)?;
    let Some(lease) = registry
        .leases
        .iter_mut()
        .find(|lease| lease.agent_id == agent_id && lease.lease_id == lease_id)
    else {
        return Err("unknown_lease".into());
    };
    if lease.expires_at_ms < now {
        return Err("lease_expired".into());
    }
    lease.last_heartbeat_at_ms = now;
    lease.expires_at_ms = now.saturating_add(config.lease_ttl_ms as u128);
    let updated = lease.clone();
    write_lease_registry(runtime_home, &registry)?;
    mark_existing_agent_online(runtime_home, agent_id, now)?;
    Ok(updated)
}

fn mark_existing_agent_online(
    runtime_home: &Path,
    agent_id: &str,
    now: u128,
) -> Result<(), String> {
    let presence_path = runtime_home.join("runtime/current/current_agent_presence_registry.json");
    if let Some(mut value) = read_json::<Value>(&presence_path)? {
        if let Some(agents) = value
            .as_object_mut()
            .and_then(|object| object.get_mut("agents"))
            .and_then(Value::as_array_mut)
            && let Some(agent) = agents
                .iter_mut()
                .find(|agent| agent.get("agent_id").and_then(Value::as_str) == Some(agent_id))
        {
            agent["status"] = json!("idle");
            agent["current_phase"] = json!("network_heartbeat");
            agent["updated_at"] = json!(now.to_string());
            write_json(&presence_path, &value)?;
        }
    }

    let peer_path = runtime_home.join("runtime/peers/registry.json");
    if let Some(mut value) = read_json::<Value>(&peer_path)? {
        if let Some(peers) = value
            .as_object_mut()
            .and_then(|object| object.get_mut("peers"))
            .and_then(Value::as_array_mut)
            && let Some(peer) = peers
                .iter_mut()
                .find(|peer| peer.get("peer_id").and_then(Value::as_str) == Some(agent_id))
        {
            peer["presence_state"] = json!("online");
            peer["runtime_state"] = json!("network_heartbeat");
            peer["lifecycle_state"] = json!("online");
            peer["last_heartbeat_at"] = json!(now.to_string());
            peer["updated_at"] = json!(now.to_string());
            write_json(&peer_path, &value)?;
        }
    }
    Ok(())
}

fn upsert_lease(runtime_home: &Path, lease: AgentLeaseRecord) -> Result<(), String> {
    let mut registry = read_lease_registry(runtime_home)?;
    registry
        .leases
        .retain(|item| item.agent_id != lease.agent_id);
    registry.leases.push(lease);
    registry
        .leases
        .sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
    write_lease_registry(runtime_home, &registry)
}

fn read_lease_registry(runtime_home: &Path) -> Result<AgentLeaseRegistry, String> {
    read_json(&runtime_home.join("runtime/agents/network_leases.json"))
        .map(|value| value.unwrap_or_default())
}

fn write_lease_registry(runtime_home: &Path, registry: &AgentLeaseRegistry) -> Result<(), String> {
    write_json(
        &runtime_home.join("runtime/agents/network_leases.json"),
        registry,
    )
}

fn upsert_agent_presence(
    runtime_home: &Path,
    payload: &AgentHandshakeRequest,
    status: &str,
    phase: &str,
    now: u128,
) -> Result<(), String> {
    let path = runtime_home.join("runtime/current/current_agent_presence_registry.json");
    let mut value = read_json::<Value>(&path)?.unwrap_or_else(|| json!({"agents": []}));
    let agents = value
        .as_object_mut()
        .and_then(|object| object.get_mut("agents"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "invalid presence registry shape".to_string())?;
    agents.retain(|agent| {
        agent.get("agent_id").and_then(Value::as_str) != Some(payload.agent_id.as_str())
    });
    agents.push(json!({
        "agent_id": payload.agent_id,
        "agent_name": payload.agent_name,
        "device_name": payload.machine,
        "role": if matches!(payload.kind, AgentKind::SystemAgent) { "system" } else { "project" },
        "agent_kind": if matches!(payload.kind, AgentKind::SystemAgent) { "system_agent" } else { "project_agent" },
        "status": status,
        "current_phase": phase,
        "current_task_id": null,
        "updated_at": now.to_string(),
        "project_id": payload.project_id,
    }));
    write_json(&path, &value)
}

fn upsert_peer_registry(
    runtime_home: &Path,
    payload: &AgentHandshakeRequest,
    presence_state: &str,
    runtime_state: &str,
    now: u128,
) -> Result<(), String> {
    let path = runtime_home.join("runtime/peers/registry.json");
    let mut value = read_json::<Value>(&path)?.unwrap_or_else(|| json!({"peers": []}));
    let peers = value
        .as_object_mut()
        .and_then(|object| object.get_mut("peers"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "invalid peer registry shape".to_string())?;
    peers.retain(|peer| {
        peer.get("peer_id").and_then(Value::as_str) != Some(payload.agent_id.as_str())
    });
    peers.push(json!({
        "peer_id": payload.agent_id,
        "peer_kind": if matches!(payload.kind, AgentKind::SystemAgent) { "system_agent" } else { "project_agent" },
        "presence_state": presence_state,
        "runtime_state": runtime_state,
        "connectivity_state": "network_connected",
        "binding_state": "agent_rpc_lease",
        "lifecycle_state": presence_state,
        "updated_at": now.to_string(),
        "last_heartbeat_at": now.to_string(),
        "project_id": payload.project_id,
    }));
    write_json(&path, &value)
}

fn read_request(stream: &mut TcpStream) -> Result<AgentRpcRequest, DebugDataError> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0usize;
    loop {
        let bytes = stream
            .read(&mut chunk)
            .map_err(|source| DebugDataError::Io {
                path: "agent-rpc-read".into(),
                source,
            })?;
        if bytes == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes]);
        if header_end.is_none() {
            header_end = find_header_end(&buffer);
            if let Some(end) = header_end {
                let head = String::from_utf8_lossy(&buffer[..end]);
                content_length = parse_content_length(&head);
            }
        }
        if let Some(end) = header_end {
            let body_bytes = buffer.len().saturating_sub(end + 4);
            if body_bytes >= content_length {
                break;
            }
        }
    }
    let request = String::from_utf8_lossy(&buffer);
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut first = head
        .lines()
        .next()
        .unwrap_or("GET / HTTP/1.1")
        .split_whitespace();
    let authorization = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_string())
    });
    Ok(AgentRpcRequest {
        method: first.next().unwrap_or("GET").to_string(),
        path: first.next().unwrap_or("/").to_string(),
        authorization,
        body: body
            .as_bytes()
            .iter()
            .take(content_length)
            .copied()
            .collect(),
        remote_addr: stream.peer_addr().ok().map(|value| value.to_string()),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

fn write_response(
    stream: &mut TcpStream,
    status_code: u16,
    content_type: &'static str,
    body: &[u8],
) -> Result<(), DebugDataError> {
    let status_text = match status_code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        status_code,
        status_text,
        content_type,
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(body))
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "agent-rpc-write".into(),
            source,
        })
}

fn json_response(status_code: u16, value: Value) -> AgentRpcResponse {
    AgentRpcResponse {
        status_code,
        body: serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0)
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn test_response_for_agent_rpc_request(
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: Value,
    runtime_home: &Path,
    config: &AgentRpcConfig,
) -> (u16, Value) {
    let request = AgentRpcRequest {
        method: method.into(),
        path: path.into(),
        authorization: authorization.map(str::to_string),
        body: serde_json::to_vec(&body).expect("body"),
        remote_addr: Some("127.0.0.1:1".into()),
    };
    let response = response_for_agent_rpc_request(&request, runtime_home, config);
    let value = serde_json::from_slice(&response.body).expect("json response");
    (response.status_code, value)
}
