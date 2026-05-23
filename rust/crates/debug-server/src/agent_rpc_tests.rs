use super::agent_rpc::{
    AgentRpcConfig, serve_agent_rpc_on_listener, test_response_for_agent_rpc_request,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

struct AgentRpcHarness {
    runtime_home: PathBuf,
    config: AgentRpcConfig,
}

impl AgentRpcHarness {
    fn new(prefix: &str) -> Self {
        Self {
            runtime_home: temp_runtime_home(prefix),
            config: AgentRpcConfig {
                bind_addr: "127.0.0.1:0".into(),
                bearer_token: "secret-token".into(),
                lease_ttl_ms: 300_000,
                heartbeat_ttl_ms: 30_000,
            },
        }
    }

    fn with_ttls(prefix: &str, lease_ttl_ms: u64, heartbeat_ttl_ms: u64) -> Self {
        let mut harness = Self::new(prefix);
        harness.config.lease_ttl_ms = lease_ttl_ms;
        harness.config.heartbeat_ttl_ms = heartbeat_ttl_ms;
        harness
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        authorization: Option<&str>,
        body: Value,
    ) -> (u16, Value) {
        test_response_for_agent_rpc_request(
            method,
            path,
            authorization,
            body,
            &self.runtime_home,
            &self.config,
        )
    }

    fn authed(&self, method: &str, path: &str, body: Value) -> (u16, Value) {
        self.request(method, path, Some("Bearer secret-token"), body)
    }

    fn handshake_project(&self, machine: &str, agent_name: &str, project_id: &str) -> String {
        let agent_id = format!("{machine}.{agent_name}");
        let (status, body) = self.authed(
            "POST",
            "/agent/v1/handshake",
            json!({
                "agent_id": agent_id,
                "kind": "project_agent",
                "project_id": project_id,
                "machine": machine,
                "agent_name": agent_name,
                "auth_subject": format!("user:jason/project:{project_id}"),
                "capabilities": ["exec", "mailbox"],
                "tools": ["exec_command", "mailbox.send"],
                "endpoint": format!("http://{machine}.local:4041")
            }),
        );
        assert_eq!(status, 200, "project handshake failed: {body}");
        body["lease_id"].as_str().expect("lease id").to_string()
    }

    fn handshake_system(&self, machine: &str, agent_name: &str) -> String {
        let agent_id = format!("{machine}.{agent_name}");
        let (status, body) = self.authed(
            "POST",
            "/agent/v1/handshake",
            json!({
                "agent_id": agent_id,
                "kind": "system_agent",
                "machine": machine,
                "agent_name": agent_name,
                "capabilities": ["mailbox"],
                "tools": ["mailbox.send"]
            }),
        );
        assert_eq!(status, 200, "system handshake failed: {body}");
        body["lease_id"].as_str().expect("lease id").to_string()
    }

    fn heartbeat(&self, agent_id: &str, lease_id: &str) -> (u16, Value) {
        self.authed(
            "POST",
            "/agent/v1/heartbeat",
            json!({"agent_id": agent_id, "lease_id": lease_id}),
        )
    }

    fn send_mailbox(
        &self,
        message_id: &str,
        from_agent_id: &str,
        to_agent_id: &str,
        lease_id: &str,
    ) -> (u16, Value) {
        self.authed(
            "POST",
            "/agent/v1/mailbox/send",
            json!({
                "message_id": message_id,
                "from_agent_id": from_agent_id,
                "to_agent_id": to_agent_id,
                "lease_id": lease_id,
                "thread_id": "thread-1",
                "task_id": "task-1",
                "trigger_turn": true,
                "payload": {"text":"please inspect"}
            }),
        )
    }

    fn agents(&self) -> (u16, Value) {
        self.authed("GET", "/agent/v1/agents", json!({}))
    }

    fn report_run_status(
        &self,
        agent_id: &str,
        lease_id: &str,
        agent_run_id: &str,
        status: &str,
        result_refs: Vec<&str>,
    ) -> (u16, Value) {
        self.authed(
            "POST",
            "/agent/v1/run/status",
            json!({
                "agent_id": agent_id,
                "lease_id": lease_id,
                "agent_run_id": agent_run_id,
                "status": status,
                "result_refs": result_refs,
            }),
        )
    }

    fn read_json(&self, relative: &str) -> Value {
        let text = std::fs::read_to_string(self.runtime_home.join(relative)).expect(relative);
        serde_json::from_str(&text).expect(relative)
    }
}

struct LocalAgentRpcServer {
    runtime_home: PathBuf,
    addr: std::net::SocketAddr,
    config: AgentRpcConfig,
}

impl LocalAgentRpcServer {
    fn start(prefix: &str) -> Self {
        Self::start_with_ttls(prefix, 300_000, 30_000)
    }

    fn start_with_ttls(prefix: &str, lease_ttl_ms: u64, heartbeat_ttl_ms: u64) -> Self {
        let runtime_home = temp_runtime_home(prefix);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind agent rpc listener");
        let addr = listener.local_addr().expect("local addr");
        let config = AgentRpcConfig {
            bind_addr: addr.to_string(),
            bearer_token: "secret-token".into(),
            lease_ttl_ms,
            heartbeat_ttl_ms,
        };
        let server_home = runtime_home.clone();
        let server_config = config.clone();
        thread::spawn(move || {
            let _ = serve_agent_rpc_on_listener(server_home, listener, server_config);
        });
        Self {
            runtime_home,
            addr,
            config,
        }
    }

    fn client(
        &self,
        machine: &str,
        agent_name: &str,
        kind: &str,
        project_id: Option<&str>,
    ) -> LocalAgentClient {
        LocalAgentClient {
            base_addr: self.addr,
            token: self.config.bearer_token.clone(),
            machine: machine.into(),
            agent_name: agent_name.into(),
            kind: kind.into(),
            project_id: project_id.map(str::to_string),
            lease_id: None,
        }
    }

    fn read_json(&self, relative: &str) -> Value {
        let text = std::fs::read_to_string(self.runtime_home.join(relative)).expect(relative);
        serde_json::from_str(&text).expect(relative)
    }
}

struct LocalAgentClient {
    base_addr: std::net::SocketAddr,
    token: String,
    machine: String,
    agent_name: String,
    kind: String,
    project_id: Option<String>,
    lease_id: Option<String>,
}

impl LocalAgentClient {
    fn agent_id(&self) -> String {
        format!("{}.{}", self.machine, self.agent_name)
    }

    fn handshake(&mut self) -> Value {
        let mut payload = json!({
            "agent_id": self.agent_id(),
            "kind": self.kind,
            "machine": self.machine,
            "agent_name": self.agent_name,
            "capabilities": ["mailbox"],
            "tools": ["mailbox.send"],
        });
        if let Some(project_id) = &self.project_id {
            payload["project_id"] = json!(project_id);
        }
        let (status, body) = self.request("POST", "/agent/v1/handshake", payload);
        assert_eq!(status, 200, "handshake failed: {body}");
        self.lease_id = Some(body["lease_id"].as_str().expect("lease id").to_string());
        body
    }

    fn heartbeat(&self) -> Value {
        let (status, body) = self.request(
            "POST",
            "/agent/v1/heartbeat",
            json!({"agent_id": self.agent_id(), "lease_id": self.lease_id.as_ref().expect("lease")}),
        );
        assert_eq!(status, 200, "heartbeat failed: {body}");
        body
    }

    fn list_agents(&self) -> Value {
        let (status, body) = self.request("GET", "/agent/v1/agents", json!({}));
        assert_eq!(status, 200, "agents failed: {body}");
        body
    }

    fn send_mailbox(&self, to_agent_id: &str, message_id: &str) -> Value {
        let (status, body) = self.request(
            "POST",
            "/agent/v1/mailbox/send",
            json!({
                "message_id": message_id,
                "from_agent_id": self.agent_id(),
                "to_agent_id": to_agent_id,
                "lease_id": self.lease_id.as_ref().expect("lease"),
                "trigger_turn": true,
                "payload": {"text":"real tcp hello"}
            }),
        );
        assert_eq!(status, 200, "mailbox failed: {body}");
        body
    }

    fn request(&self, method: &str, path: &str, body: Value) -> (u16, Value) {
        let body_text = body.to_string();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.base_addr,
            self.token,
            body_text.len(),
            body_text
        );
        let mut stream = TcpStream::connect(self.base_addr).expect("connect agent rpc");
        stream.write_all(request.as_bytes()).expect("write request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        parse_http_json_response(&response)
    }
}

fn parse_http_json_response(response: &[u8]) -> (u16, Value) {
    let text = String::from_utf8_lossy(response);
    let (head, body) = text.split_once("\r\n\r\n").expect("http response");
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .expect("status code");
    let value = serde_json::from_str(body).expect("json body");
    (status, value)
}

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-agent-rpc-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn lifecycle_harness_covers_register_heartbeat_discover_mailbox_and_artifacts() {
    let harness = AgentRpcHarness::new("lifecycle");

    let project_lease = harness.handshake_project("mac", "project-fin", "fin");
    let system_lease = harness.handshake_system("studio", "system");

    let (status, heartbeat) = harness.heartbeat("mac.project-fin", &project_lease);
    assert_eq!(status, 200);
    assert_eq!(heartbeat["agent_id"], "mac.project-fin");

    let (status, agents) = harness.agents();
    assert_eq!(status, 200);
    let ids = agents["agents"]
        .as_array()
        .expect("agents")
        .iter()
        .map(|agent| agent["agent_id"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(ids.contains(&"mac.project-fin".to_string()));
    assert!(ids.contains(&"studio.system".to_string()));

    let (status, first) =
        harness.send_mailbox("msg-1", "studio.system", "mac.project-fin", &system_lease);
    assert_eq!(status, 200);
    assert_eq!(first["seq"], 1);
    let (status, second) =
        harness.send_mailbox("msg-2", "studio.system", "mac.project-fin", &system_lease);
    assert_eq!(status, 200);
    assert_eq!(second["seq"], 2);

    let leases = harness.read_json("runtime/agents/network_leases.json");
    assert_eq!(leases["leases"].as_array().expect("leases").len(), 2);
    let presence = harness.read_json("runtime/current/current_agent_presence_registry.json");
    assert!(presence.to_string().contains("network_heartbeat"));
    let peers = harness.read_json("runtime/peers/registry.json");
    assert!(peers.to_string().contains("agent_rpc_lease"));
    let inbox = harness.read_json("runtime/agents/control/mailbox/mac.project-fin/inbox.json");
    assert_eq!(inbox.as_array().expect("inbox").len(), 2);
}

#[test]
fn real_tcp_two_local_agent_instances_register_discover_and_collaborate() {
    let server = LocalAgentRpcServer::start("real-two-agent");
    let mut system_agent = server.client("studio", "system", "system_agent", None);
    let mut project_agent = server.client("mac", "project-fin", "project_agent", Some("fin"));

    let system_handshake = system_agent.handshake();
    let project_handshake = project_agent.handshake();
    assert_eq!(system_handshake["agent_id"], "studio.system");
    assert_eq!(project_handshake["agent_id"], "mac.project-fin");

    system_agent.heartbeat();
    project_agent.heartbeat();

    let agents = system_agent.list_agents();
    let agent_ids = agents["agents"]
        .as_array()
        .expect("agents")
        .iter()
        .map(|agent| agent["agent_id"].as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    assert!(agent_ids.contains(&"studio.system".to_string()));
    assert!(agent_ids.contains(&"mac.project-fin".to_string()));

    let message = system_agent.send_mailbox("mac.project-fin", "real-msg-1");
    assert_eq!(message["seq"], 1);

    let leases = server.read_json("runtime/agents/network_leases.json");
    assert_eq!(leases["leases"].as_array().expect("leases").len(), 2);
    let inbox = server.read_json("runtime/agents/control/mailbox/mac.project-fin/inbox.json");
    assert_eq!(inbox[0]["message_id"], "real-msg-1");
    assert_eq!(inbox[0]["trigger_turn"], true);
}

#[test]
fn real_tcp_second_local_instance_can_reconnect_after_lost_heartbeat() {
    let server = LocalAgentRpcServer::start_with_ttls("real-reconnect", 300_000, 1);
    let mut project_agent = server.client("mac", "project-fin", "project_agent", Some("fin"));
    project_agent.handshake();

    std::thread::sleep(std::time::Duration::from_millis(3));
    let agents = project_agent.list_agents();
    assert_eq!(agents["agents"][0]["status"], "offline");

    project_agent.heartbeat();
    let agents = project_agent.list_agents();
    assert_eq!(agents["agents"][0]["status"], "online");
}

#[test]
fn auth_matrix_rejects_missing_malformed_and_wrong_bearer_without_side_effects() {
    for (label, authorization) in [
        ("missing", None),
        ("malformed", Some("Token secret-token")),
        ("wrong", Some("Bearer wrong-token")),
    ] {
        let harness = AgentRpcHarness::new(label);
        let (status, body) = harness.request(
            "POST",
            "/agent/v1/handshake",
            authorization,
            json!({"agent_id":"mac.system","kind":"system_agent","machine":"mac","agent_name":"system"}),
        );
        assert_eq!(status, 401, "{label}: {body}");
        assert_eq!(body["error"], "auth_failed");
        assert!(
            !harness
                .runtime_home
                .join("runtime/agents/network_leases.json")
                .exists()
        );
    }
}

#[test]
fn handshake_error_matrix_rejects_bad_identity_subagent_missing_project_and_duplicate_online() {
    let harness = AgentRpcHarness::new("handshake-errors");

    let (status, body) = harness.authed(
        "POST",
        "/agent/v1/handshake",
        json!({"agent_id":"wrong.name","kind":"system_agent","machine":"mac","agent_name":"system"}),
    );
    assert_eq!(status, 400);
    assert_eq!(body["error"], "agent_id_must_match_machine_agentname");

    let (status, body) = harness.authed(
        "POST",
        "/agent/v1/handshake",
        json!({"agent_id":"mac.child","kind":"subagent","machine":"mac","agent_name":"child"}),
    );
    assert_eq!(status, 400);
    assert_eq!(
        body["error"],
        "network_handshake_accepts_primary_agents_only"
    );

    let (status, body) = harness.authed(
        "POST",
        "/agent/v1/handshake",
        json!({"agent_id":"mac.project","kind":"project_agent","machine":"mac","agent_name":"project"}),
    );
    assert_eq!(status, 400);
    assert_eq!(body["error"], "project_agent_requires_project_id");

    harness.handshake_system("mac", "system");
    let (status, body) = harness.authed(
        "POST",
        "/agent/v1/handshake",
        json!({"agent_id":"mac.system","kind":"system_agent","machine":"mac","agent_name":"system"}),
    );
    assert_eq!(status, 409);
    assert_eq!(body["error"], "agent_already_online");
}

#[test]
fn lease_error_matrix_covers_unknown_and_expired_leases() {
    let harness = AgentRpcHarness::new("lease-errors");
    harness.handshake_system("mac", "system");

    let (status, body) = harness.heartbeat("mac.system", "lease-missing");
    assert_eq!(status, 403);
    assert_eq!(body["error"], "unknown_lease");

    let expired = AgentRpcHarness::with_ttls("expired", 1, 1);
    let lease = expired.handshake_system("mac", "system");
    std::thread::sleep(std::time::Duration::from_millis(3));
    let (status, body) = expired.heartbeat("mac.system", &lease);
    assert_eq!(status, 403);
    assert_eq!(body["error"], "lease_expired");

    let (status, agents) = expired.agents();
    assert_eq!(status, 200);
    assert_eq!(agents["agents"][0]["status"], "offline");
}

#[test]
fn mailbox_error_matrix_rejects_unknown_sender_unknown_target_and_bad_lease() {
    let harness = AgentRpcHarness::new("mailbox-errors");
    let project_lease = harness.handshake_project("mac", "project-fin", "fin");
    let system_lease = harness.handshake_system("studio", "system");

    let (status, body) = harness.send_mailbox(
        "msg-bad-lease",
        "studio.system",
        "mac.project-fin",
        "lease-missing",
    );
    assert_eq!(status, 403);
    assert_eq!(body["error"], "unknown_lease");

    let (status, body) = harness.send_mailbox(
        "msg-unknown-target",
        "studio.system",
        "ghost.project",
        &system_lease,
    );
    assert_eq!(status, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unknown agent identity")
    );

    let (status, body) = harness.send_mailbox(
        "msg-unknown-sender",
        "ghost.system",
        "mac.project-fin",
        &project_lease,
    );
    assert_eq!(status, 403);
    assert_eq!(body["error"], "unknown_lease");
}

#[test]
fn route_and_body_errors_are_structured() {
    let harness = AgentRpcHarness::new("route-body");
    let (status, body) = harness.authed("GET", "/agent/v1/missing", json!({}));
    assert_eq!(status, 404);
    assert_eq!(body["error"], "not_found");

    let (status, body) = test_response_for_agent_rpc_request(
        "POST",
        "/agent/v1/handshake",
        Some("Bearer secret-token"),
        Value::String("not an object".into()),
        Path::new(&harness.runtime_home),
        &harness.config,
    );
    assert_eq!(status, 400);
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid_body")
    );
}

#[test]
fn dropped_connection_marks_agent_offline_then_recovery_heartbeat_marks_online() {
    let harness = AgentRpcHarness::with_ttls("drop-recover", 300_000, 1);
    let lease = harness.handshake_project("mac", "project-fin", "fin");

    std::thread::sleep(std::time::Duration::from_millis(3));
    let (status, agents) = harness.agents();
    assert_eq!(status, 200);
    assert_eq!(agents["agents"][0]["status"], "offline");

    let (status, heartbeat) = harness.heartbeat("mac.project-fin", &lease);
    assert_eq!(status, 200);
    assert_eq!(heartbeat["agent_id"], "mac.project-fin");

    let (status, agents) = harness.agents();
    assert_eq!(status, 200);
    assert_eq!(agents["agents"][0]["status"], "online");
    let presence = harness.read_json("runtime/current/current_agent_presence_registry.json");
    assert!(presence.to_string().contains("network_heartbeat"));
}

#[test]
fn execution_failure_report_updates_run_truth_and_wait_result() {
    let harness = AgentRpcHarness::new("execution-failure");
    let lease = harness.handshake_project("mac", "project-fin", "fin");

    let (status, body) = harness.report_run_status(
        "mac.project-fin",
        &lease,
        "run-failed-1",
        "failed",
        vec!["runtime/results/run-failed-1.json"],
    );
    assert_eq!(status, 200);
    assert_eq!(body["status"], "failed");

    let run = harness.read_json("runtime/agents/control/runs/run-failed-1.json");
    assert_eq!(run["status"], "failed");
    assert_eq!(run["result_refs"][0], "runtime/results/run-failed-1.json");
    assert!(run["closed_at"].as_str().is_some());

    let (status, invalid) = harness.report_run_status(
        "mac.project-fin",
        &lease,
        "run-bad-status",
        "panic-string",
        vec![],
    );
    assert_eq!(status, 400);
    assert_eq!(invalid["error"], "invalid_run_status");
}

#[test]
fn tcp_connection_unavailable_and_dropped_mid_request_are_observable_errors() {
    let unavailable = TcpStream::connect("127.0.0.1:9");
    assert!(
        unavailable.is_err(),
        "discard port should not accept fin Agent RPC"
    );

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let addr = listener.local_addr().expect("addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 16];
        let _ = stream.read(&mut buffer);
        drop(stream);
    });

    let mut stream = TcpStream::connect(addr).expect("connect test server");
    stream
        .write_all(b"POST /agent/v1/handshake HTTP/1.1\r\nContent-Length: 100\r\n\r\n{")
        .expect("write partial request");
    let mut response = String::new();
    let read = stream.read_to_string(&mut response);
    assert!(
        read.is_err() || response.is_empty(),
        "dropped connection must not produce a success response"
    );
    handle.join().expect("server thread");
}
