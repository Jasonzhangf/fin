use crate::CliError;
use fin_debug_server::agent_rpc::{AgentRpcConfig, serve_agent_rpc_on_listener};
use fin_shared::{DEFAULT_RETRY_ATTEMPTS, exponential_backoff};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    thread,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentRpcHarnessServer {
    pub(crate) endpoint: String,
    pub(crate) bearer_token: String,
}

pub(crate) fn start_agent_rpc_server(
    runtime_home: &Path,
) -> Result<AgentRpcHarnessServer, CliError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|source| CliError::ReadFile {
        path: "bind agent rpc listener".into(),
        source,
    })?;
    let endpoint = listener
        .local_addr()
        .map_err(|source| CliError::ReadFile {
            path: "agent rpc listener addr".into(),
            source,
        })?
        .to_string();
    let config = AgentRpcConfig {
        bind_addr: endpoint.clone(),
        bearer_token: "local-harness-token".into(),
        lease_ttl_ms: 300_000,
        heartbeat_ttl_ms: 30_000,
    };
    let server_home = runtime_home.to_path_buf();
    let server_config = config.clone();
    thread::spawn(move || {
        let _ = serve_agent_rpc_on_listener(server_home, listener, server_config);
    });
    Ok(AgentRpcHarnessServer {
        endpoint,
        bearer_token: config.bearer_token,
    })
}

pub(crate) fn rpc_handshake_system(server: &AgentRpcHarnessServer) -> Result<String, CliError> {
    let body = rpc_post(
        server,
        "/agent/v1/handshake",
        json!({
            "agent_id": "local.system",
            "kind": "system_agent",
            "machine": "local",
            "agent_name": "system",
            "capabilities": ["mailbox", "agent-control"],
            "tools": ["mailbox.send", "agent.list"]
        }),
    )?;
    lease_id_from_body(body)
}

pub(crate) fn rpc_handshake_project(server: &AgentRpcHarnessServer) -> Result<String, CliError> {
    let body = rpc_post(
        server,
        "/agent/v1/handshake",
        json!({
            "agent_id": "local.project-fin",
            "kind": "project_agent",
            "project_id": "fin",
            "machine": "local",
            "agent_name": "project-fin",
            "auth_subject": "user:jason/project:fin",
            "capabilities": ["exec", "mailbox", "provider"],
            "tools": ["exec_command", "mailbox.send", "provider.call"]
        }),
    )?;
    lease_id_from_body(body)
}

pub(crate) fn rpc_agents(server: &AgentRpcHarnessServer) -> Result<Value, CliError> {
    rpc_get(server, "/agent/v1/agents")
}

pub(crate) fn rpc_send_dispatch(
    server: &AgentRpcHarnessServer,
    system_lease_id: &str,
    project_cwd: &Path,
    user_request: &str,
) -> Result<Value, CliError> {
    retry_rpc_request("rpc_send_dispatch", || {
        rpc_post(
            server,
            "/agent/v1/mailbox/send",
            json!({
                "message_id": "msg-dispatch-1",
                "from_agent_id": "local.system",
                "to_agent_id": "local.system",
                "lease_id": system_lease_id,
                "thread_id": "thread-local-multi-agent",
                "task_id": "task-local-multi-agent",
                "trigger_turn": true,
                "payload": {
                    "kind": "user_request",
                    "content": user_request,
                    "agent_run_id": "project-run-local-multi-agent",
                    "cwd": project_cwd
                }
            }),
        )
    })
}

fn retry_rpc_request<F>(label: &str, mut action: F) -> Result<Value, CliError>
where
    F: FnMut() -> Result<Value, CliError>,
{
    let mut last_err = None;
    for attempt in 1..=DEFAULT_RETRY_ATTEMPTS {
        match action() {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_err = Some(err);
                if attempt < DEFAULT_RETRY_ATTEMPTS {
                    thread::sleep(exponential_backoff(attempt));
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        CliError::Runtime(fin_runtime::RuntimeError::State(format!(
            "{label} failed without concrete error"
        )))
    }))
}

#[cfg(test)]
mod tests {
    use fin_shared::exponential_backoff;
    use std::time::Duration;

    #[test]
    fn rpc_retry_uses_exponential_backoff_schedule() {
        assert_eq!(exponential_backoff(1), Duration::from_secs(1));
        assert_eq!(exponential_backoff(2), Duration::from_secs(2));
        assert_eq!(exponential_backoff(3), Duration::from_secs(4));
        assert_eq!(exponential_backoff(4), Duration::from_secs(8));
        assert_eq!(exponential_backoff(5), Duration::from_secs(16));
    }
}

fn rpc_get(server: &AgentRpcHarnessServer, path: &str) -> Result<Value, CliError> {
    rpc_request(server, "GET", path, None)
}

fn rpc_post(server: &AgentRpcHarnessServer, path: &str, body: Value) -> Result<Value, CliError> {
    rpc_request(server, "POST", path, Some(body))
}

fn rpc_request(
    server: &AgentRpcHarnessServer,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, CliError> {
    rpc_request_raw(&server.endpoint, &server.bearer_token, method, path, body)
}

fn rpc_request_raw(
    endpoint: &str,
    bearer_token: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, CliError> {
    let mut stream = TcpStream::connect(endpoint).map_err(|source| CliError::ReadFile {
        path: format!("connect agent rpc {endpoint}"),
        source,
    })?;
    let body_bytes = body
        .map(|value| serde_json::to_vec(&value))
        .transpose()?
        .unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        endpoint,
        bearer_token,
        body_bytes.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(&body_bytes))
        .map_err(|source| CliError::WriteFile {
            path: format!("write agent rpc {path}"),
            source,
        })?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|source| CliError::ReadFile {
            path: format!("read agent rpc {path}"),
            source,
        })?;
    parse_rpc_response(path, &response)
}

fn parse_rpc_response(path: &str, response: &[u8]) -> Result<Value, CliError> {
    let text = String::from_utf8_lossy(response);
    let (head, body) = text.split_once("\r\n\r\n").ok_or(CliError::Usage)?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let value: Value = serde_json::from_str(body)?;
    if status >= 400 {
        return Err(CliError::Runtime(fin_runtime::RuntimeError::State(
            format!("agent rpc {path} failed: {value}"),
        )));
    }
    Ok(value)
}

fn lease_id_from_body(body: Value) -> Result<String, CliError> {
    body.get("lease_id")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            CliError::Runtime(fin_runtime::RuntimeError::State(format!(
                "agent rpc response missing lease_id: {body}"
            )))
        })
}
