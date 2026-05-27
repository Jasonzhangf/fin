use crate::CliError;
use fin_contracts::{EntityRefs, LedgerRefs, LedgerTrackKind};
use fin_debug_server::{ChatSendRequest, DebugBinding};
use fin_runtime::{
    AgentControlStore, AppendLedgerRecordInput, LedgerQuery, LedgerStore, RuntimeError,
};
use serde::Serialize;
use serde_json::json;
use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectAgentConfigReceipt {
    pub(crate) endpoint: String,
    pub(crate) port_persisted: bool,
}

pub(crate) fn spawn_harness_node(
    runtime_home: &Path,
    role: &str,
    agent_id: &str,
    user_config_path: Option<&Path>,
) -> Result<Child, CliError> {
    fs::create_dir_all(runtime_home.join("runtime/agents/state")).map_err(|source| {
        CliError::WriteFile {
            path: runtime_home
                .join("runtime/agents/state")
                .display()
                .to_string(),
            source,
        }
    })?;
    let current_exe = resolve_harness_executable()?;
    let mut command = if current_exe == PathBuf::from("cargo") {
        let mut command = ProcessCommand::new("cargo");
        command
            .arg("run")
            .arg("--manifest-path")
            .arg(cargo_workspace_manifest_path()?)
            .arg("-p")
            .arg("fin-cli")
            .arg("--")
            .arg("local-multi-agent-node");
        command
    } else {
        let mut command = ProcessCommand::new(current_exe);
        command.arg("local-multi-agent-node");
        command
    };
    let log_dir = runtime_home.join("runtime/agents/logs");
    fs::create_dir_all(&log_dir).map_err(|source| CliError::WriteFile {
        path: log_dir.display().to_string(),
        source,
    })?;
    let stdout_path = log_dir.join(format!("{}.stdout.log", agent_id));
    let stderr_path = log_dir.join(format!("{}.stderr.log", agent_id));
    let stdout = fs::File::create(&stdout_path).map_err(|source| CliError::WriteFile {
        path: stdout_path.display().to_string(),
        source,
    })?;
    let stderr = fs::File::create(&stderr_path).map_err(|source| CliError::WriteFile {
        path: stderr_path.display().to_string(),
        source,
    })?;
    command.arg(runtime_home).arg(role).arg(agent_id);
    if let Some(path) = user_config_path {
        command.arg(path);
    }
    if let Ok(value) = std::env::var("FIN_LOCAL_MULTI_AGENT_STATIC_LLM") {
        command.env("FIN_LOCAL_MULTI_AGENT_STATIC_LLM", value);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|source| CliError::ReadFile {
            path: format!("spawn local-multi-agent-node {agent_id}"),
            source,
        })
}

fn resolve_harness_executable() -> Result<std::path::PathBuf, CliError> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_fin-cli") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    let current_exe = std::env::current_exe().map_err(|source| CliError::ReadFile {
        path: "current executable".into(),
        source,
    })?;
    let current_name = current_exe
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let running_under_test_harness = current_name.starts_with("fin_cli-")
        || current_exe
            .components()
            .any(|component| component.as_os_str() == "deps");
    if running_under_test_harness {
        return Ok(PathBuf::from("cargo"));
    }
    let debug_binary = current_exe
        .parent()
        .and_then(|deps| deps.parent())
        .map(|debug_dir| debug_dir.join("fin-cli"));
    if current_name == "fin-cli" || current_name == "fin" {
        return Ok(current_exe);
    }
    if let Some(binary) = debug_binary.filter(|path| path.exists()) {
        return Ok(binary);
    }
    Err(CliError::Runtime(RuntimeError::State(format!(
        "fin-cli harness executable not found from {}",
        current_exe.display()
    ))))
}

fn cargo_workspace_manifest_path() -> Result<PathBuf, CliError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|crates_dir| crates_dir.parent())
        .map(|rust_dir| rust_dir.join("Cargo.toml"))
        .ok_or_else(|| {
            CliError::Runtime(RuntimeError::State(
                "failed to resolve rust workspace manifest path".into(),
            ))
        })
}

pub(crate) fn scoped_stop(
    child: &mut Child,
    runtime_home: &Path,
    agent_id: &str,
) -> Result<(), CliError> {
    fs::write(
        runtime_home.join(format!("runtime/agents/state/{agent_id}.stop")),
        child.id().to_string(),
    )
    .map_err(|source| CliError::WriteFile {
        path: format!("stop pid {}", child.id()),
        source,
    })?;
    let status = child.wait().map_err(|source| CliError::ReadFile {
        path: format!("wait pid {}", child.id()),
        source,
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(CliError::ProcessFailed {
            command: format!("local-multi-agent-node pid {}", child.id()),
            exit_code: status.code(),
        })
    }
}

pub(crate) fn persist_project_agent_config(
    runtime_home: &Path,
    project_cwd: &Path,
    agent_rpc_endpoint: &str,
    agent_rpc_token: &str,
) -> Result<ProjectAgentConfigReceipt, CliError> {
    let port = allocate_ephemeral_port()?;
    let endpoint = format!("127.0.0.1:{port}");
    write_json(
        &runtime_home.join("runtime/projects/project-fin-agent-config.json"),
        &json!({
            "project_id": "fin",
            "agent_id": "local.project-fin",
            "cwd": project_cwd,
            "endpoint": endpoint,
            "port": port,
            "port_persisted": true,
            "agent_rpc": {
                "endpoint": agent_rpc_endpoint,
                "token": agent_rpc_token
            },
            "auth": {
                "kind": "harness-token",
                "configured": true,
                "token": agent_rpc_token
            }
        }),
    )?;
    Ok(ProjectAgentConfigReceipt {
        endpoint,
        port_persisted: true,
    })
}

fn allocate_ephemeral_port() -> Result<u16, CliError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|source| CliError::ReadFile {
        path: "allocate project agent ephemeral port".into(),
        source,
    })?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|source| CliError::ReadFile {
            path: "read project agent ephemeral port".into(),
            source,
        })
}

pub(crate) fn append_event(
    store: &LedgerStore,
    refs: EntityRefs,
    created_at: &str,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<String, CliError> {
    append_ledger_record(
        store,
        LedgerTrackKind::Events,
        refs,
        Vec::new(),
        created_at,
        &json!({ "event_type": event_type, "payload": payload }),
    )
}

pub(crate) fn append_ledger_record(
    store: &LedgerStore,
    track: LedgerTrackKind,
    refs: EntityRefs,
    caused_by: Vec<String>,
    created_at: &str,
    payload: &serde_json::Value,
) -> Result<String, CliError> {
    let next_seq = store.query(&LedgerQuery::default())?.len() + 1;
    let record_id = format!("{}-{next_seq}", track.as_str().replace('.', "-"));
    let record_kind = payload
        .get("event_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| track.as_str())
        .to_string();
    let record_ref = format!("{}:{record_id}", track.as_str());
    store.append(AppendLedgerRecordInput {
        ts: created_at.into(),
        track,
        record_id,
        record_kind,
        refs: LedgerRefs {
            agent_id: refs.worker_id.clone(),
            entity: refs,
            ledger_id: Some(store.ledger_id().into()),
            record_refs: caused_by,
        },
        payload: payload.clone(),
        caused_by: None,
        supersedes: None,
    })?;
    Ok(record_ref)
}

pub(crate) fn ledger_strict_ok(store: &LedgerStore) -> Result<bool, CliError> {
    let records = store.query(&LedgerQuery::default())?;
    let mut previous_seq = 0;
    let mut seen = std::collections::BTreeSet::new();
    for record in records {
        if record.seq <= previous_seq || !seen.insert(record.seq) {
            return Ok(false);
        }
        previous_seq = record.seq;
    }
    Ok(true)
}

pub(crate) fn write_and_verify_compatibility_projection(
    runtime_home: &Path,
    result_ref: &str,
) -> Result<Vec<String>, CliError> {
    let messages_path = "sessions/2026/05/system-agent/conversation/messages.json";
    write_json(
        &runtime_home.join(messages_path),
        &json!([
            { "message_id": "msg-user-1", "role": "user", "content": "dispatch project task", "created_at": "2026-05-23T00:00:00Z", "session_id": "system-agent", "task_id": "task-local-multi-agent" },
            { "message_id": "msg-assistant-1", "role": "assistant", "content": format!("project result: {result_ref}"), "created_at": "2026-05-23T00:00:01Z", "session_id": "system-agent", "task_id": "task-local-multi-agent" }
        ]),
    )?;
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &json!({
            "digest_id": "digest-local-multi-agent",
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "session_messages_path": messages_path
        }),
    )?;
    write_json(
        &runtime_home.join("runtime/channels/qqbot/conversations.json"),
        &json!({
            "conversations": [{
                "conversation_id": "qqconv-local-agent-harness",
                "channel_id": "qqbot",
                "target": "local-agent-harness",
                "session_id": "system-agent",
                "status": "bound",
                "created_at": "2026-05-23T00:00:00Z",
                "updated_at": "2026-05-23T00:00:01Z"
            }]
        }),
    )?;

    let binding = DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: Some("system-agent".into()),
        task_id: Some("task-local-multi-agent".into()),
        session_messages_path: Some(messages_path.into()),
        recent_contexts_path: None,
        recent_digests_path: None,
    };
    let status_response = crate::status_probe::build_status_probe_response(
        runtime_home,
        binding,
        &ChatSendRequest {
            message: "/status".into(),
            input_kind: Some("status_probe".into()),
            attachments: Vec::new(),
        },
    )?;
    if status_response.response_kind != "status_probe" {
        return Err(CliError::Runtime(RuntimeError::State(format!(
            "status probe compatibility failed: {}",
            status_response.response_kind
        ))));
    }
    let messages: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join(messages_path)).map_err(|source| {
            CliError::ReadFile {
                path: runtime_home.join(messages_path).display().to_string(),
                source,
            }
        })?,
    )?;
    let message_count = messages.as_array().map_or(0, Vec::len);
    if message_count < 2 {
        return Err(CliError::Runtime(RuntimeError::State(
            "web debug session messages reader failed".into(),
        )));
    }
    let conversations: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/channels/qqbot/conversations.json"))
            .map_err(|source| CliError::ReadFile {
                path: runtime_home
                    .join("runtime/channels/qqbot/conversations.json")
                    .display()
                    .to_string(),
                source,
            })?,
    )?;
    let conversation_count = conversations
        .get("conversations")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if conversation_count != 1 {
        return Err(CliError::Runtime(RuntimeError::State(
            "qqbot conversation reader failed".into(),
        )));
    }
    Ok(vec![
        "status_probe".into(),
        "web_debug_session_messages".into(),
        "qqbot_conversations".into(),
    ])
}

pub(crate) fn mailbox_sequences_monotonic(runtime_home: &Path) -> Result<bool, CliError> {
    let store = AgentControlStore::new(runtime_home);
    for agent_id in ["local.system", "local.project-fin"] {
        let inbox = store
            .read_mailbox(agent_id)
            .map_err(|error| CliError::Runtime(RuntimeError::State(error)))?;
        let mut previous = 0;
        for message in inbox {
            if message.seq <= previous {
                return Ok(false);
            }
            previous = message.seq;
        }
        if previous == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}
