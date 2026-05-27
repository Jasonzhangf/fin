use crate::{
    CliError,
    local_multi_agent_lifecycle_harness_support::{append_event, append_ledger_record, write_json},
};
use fin_contracts::{EntityRefs, LedgerTrackKind};
use fin_runtime::{AgentControlStore, LedgerStore, RuntimeError, SendAgentInput};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

pub(crate) fn run_local_multi_agent_node(
    runtime_home: &Path,
    role: &str,
    agent_id: &str,
    user_config_path: Option<&Path>,
) -> Result<(), CliError> {
    write_json(
        &runtime_home.join(format!("runtime/agents/state/{agent_id}.json")),
        &json!({
            "agent_id": agent_id,
            "role": role,
            "pid": std::process::id(),
            "status": "online",
            "source": "local_multi_agent_harness_node"
        }),
    )?;
    loop {
        if role == "system" {
            if let Err(err) = handle_system_inbox(runtime_home, agent_id) {
                record_node_error(runtime_home, agent_id, "system_inbox_error", &err)?;
            }
        }
        if role == "project" {
            if let Err(err) = handle_project_inbox(runtime_home, agent_id, user_config_path) {
                record_node_error(runtime_home, agent_id, "project_inbox_error", &err)?;
            }
        }
        thread::sleep(Duration::from_millis(100));
        if runtime_home
            .join(format!("runtime/agents/state/{agent_id}.stop"))
            .exists()
        {
            break;
        }
    }
    write_json(
        &runtime_home.join(format!("runtime/agents/state/{agent_id}.stopped.json")),
        &json!({ "agent_id": agent_id, "pid": std::process::id(), "status": "stopped" }),
    )
}

fn record_node_error(
    runtime_home: &Path,
    agent_id: &str,
    kind: &str,
    err: &CliError,
) -> Result<(), CliError> {
    write_json(
        &runtime_home.join(format!("runtime/agents/state/{agent_id}.last_error.json")),
        &json!({
            "agent_id": agent_id,
            "kind": kind,
            "error": err.to_string(),
            "pid": std::process::id(),
            "source": "local_multi_agent_harness_node"
        }),
    )
}

fn handle_project_inbox(
    runtime_home: &Path,
    agent_id: &str,
    user_config_path: Option<&Path>,
) -> Result<(), CliError> {
    let Some(message) = receive_project_dispatch_over_rpc(runtime_home, agent_id)? else {
        return Ok(());
    };
    let cwd = message
        .payload
        .get("cwd")
        .and_then(|value| value.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| {
            CliError::Runtime(RuntimeError::State(
                "project harness dispatch missing cwd".into(),
            ))
        })?;
    let observed_files = observe_project_cwd(&cwd)?;
    let cwd_verified = cwd.is_dir() && !observed_files.is_empty();
    let task_summary = message
        .payload
        .get("task_description")
        .or_else(|| message.payload.get("task_summary"))
        .or_else(|| message.payload.get("content"))
        .and_then(|value| value.as_str())
        .unwrap_or("project task");
    let agent_run_id = message
        .payload
        .get("agent_run_id")
        .and_then(|value| value.as_str())
        .unwrap_or("project-run-missing");
    let config_path = resolve_project_config_path(user_config_path)?;
    let llm_execution = crate::local_multi_agent_llm_task::execute_project_llm_task(
        config_path,
        &cwd,
        task_summary,
        &observed_files,
    )?;
    write_project_result(
        runtime_home,
        agent_id,
        agent_run_id,
        &message.from_agent_id,
        message.task_id.as_deref(),
        &cwd,
        cwd_verified,
        &observed_files,
        &llm_execution,
    )
}

fn synthesize_project_task_description(user_request: &str, cwd: &Path) -> String {
    format!(
        "在工作目录 {} 中执行该任务，并持续回报 progress 与最终 result：{}",
        cwd.display(),
        user_request
    )
}

fn handle_system_inbox(runtime_home: &Path, agent_id: &str) -> Result<(), CliError> {
    let Some(message) = receive_dispatch_over_rpc(runtime_home, agent_id)? else {
        return Ok(());
    };
    let kind = message
        .payload
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let system = LedgerStore::for_session(runtime_home, "system-agent")?;
    system.init(None, Some("system-agent"), "2026-05-23T00:00:00Z")?;
    let refs = EntityRefs {
        session_id: Some("system-agent".into()),
        task_id: message
            .task_id
            .clone()
            .or(Some("task-local-multi-agent".into())),
        ..EntityRefs::default()
    };

    if kind == "user_request" {
        let user_request = message
            .payload
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or("project task request");
        let cwd = message
            .payload
            .get("cwd")
            .and_then(|value| value.as_str())
            .map(PathBuf::from)
            .ok_or_else(|| {
                CliError::Runtime(RuntimeError::State(
                    "system inbox user_request missing cwd".into(),
                ))
            })?;
        let task_description = synthesize_project_task_description(user_request, &cwd);
        append_ledger_record(
            &system,
            LedgerTrackKind::SessionDetail,
            refs.clone(),
            Vec::new(),
            "2026-05-23T00:00:00Z",
            &json!({
                "detail_id":"system-user-request-local-multi-agent",
                "visibility":"visible",
                "user_input": user_request,
                "assistant_output":"system received user request and is evaluating whether project dispatch is required"
            }),
        )?;
        append_event(
            &system,
            refs.clone(),
            "2026-05-23T00:00:00Z",
            "system.reasoning.started",
            json!({
                "reason":"request targets different cwd and requires project agent execution",
                "user_request": user_request,
                "requested_cwd": cwd
            }),
        )?;
        append_event(
            &system,
            refs.clone(),
            "2026-05-23T00:00:00Z",
            "project.management.decision",
            json!({
                "reason": "system analyzed user request and decided project cwd access is required",
                "requested_cwd": cwd,
                "selected_agent_id": "local.project-fin",
                "user_request": user_request
            }),
        )?;
        append_ledger_record(
            &system,
            LedgerTrackKind::Tools,
            refs.clone(),
            Vec::new(),
            "2026-05-23T00:00:00Z",
            &json!({
                "tool_call_id":"tool-system-dispatch-local-multi-agent",
                "tool_name":"agent.assign",
                "status":"completed",
                "target_agent_id":"local.project-fin",
                "task_description": task_description,
                "report_on_progress": true,
                "cwd": cwd
            }),
        )?;
        append_event(
            &system,
            refs.clone(),
            "2026-05-23T00:00:00Z",
            "task.dispatch.sent",
            json!({
                "task_id": refs.task_id,
                "to_agent_id": "local.project-fin",
                "task_description": task_description,
                "source": "system_tool_call"
            }),
        )?;
        let control = AgentControlStore::new(runtime_home);
        control
            .send_agent_input(SendAgentInput {
                message_id: "msg-system-dispatch-project-1".into(),
                from_agent_id: agent_id.to_string(),
                to_agent_id: "local.project-fin".into(),
                thread_id: Some("thread-local-multi-agent".into()),
                task_id: refs.task_id.clone(),
                trigger_turn: true,
                payload: json!({
                    "kind":"project_dispatch",
                    "agent_run_id":"project-run-local-multi-agent",
                    "cwd": cwd,
                    "task_description": task_description,
                    "user_request": user_request,
                    "report_on_progress": true
                }),
            })
            .map_err(fin_runtime::RuntimeError::State)
            .map_err(CliError::Runtime)?;
        write_json(
            &runtime_home.join("runtime/agents/state/system-node-dispatch.json"),
            &json!({
                "agent_id": agent_id,
                "consumed_message_id": message.message_id,
                "user_request": user_request,
                "task_description": task_description,
                "cwd": cwd,
                "source":"system_node"
            }),
        )?;
        return Ok(());
    }

    if kind != "project_result" {
        return Ok(());
    }
    let result_ref = message
        .payload
        .get("result_ref")
        .and_then(|value| value.as_str())
        .unwrap_or("project-result-missing");
    let result_summary = message
        .payload
        .get("result_summary")
        .and_then(|value| value.as_str())
        .unwrap_or("project result received");
    append_event(
        &system,
        refs.clone(),
        "2026-05-23T00:00:00Z",
        "system.inference.resumed_after_project_result",
        json!({
            "from_agent_id": message.from_agent_id,
            "result_summary": result_summary,
            "result_ref": result_ref
        }),
    )?;
    append_event(
        &system,
        refs.clone(),
        "2026-05-23T00:00:00Z",
        "task.result.received",
        json!({
            "from_agent_id": message.from_agent_id,
            "result_refs": [result_ref]
        }),
    )?;
    let user_request =
        fs::read_to_string(runtime_home.join("runtime/agents/state/system-node-dispatch.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                value
                    .get("user_request")
                    .and_then(|item| item.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "dispatch project task".to_string());
    let summary_ref = append_ledger_record(
        &system,
        LedgerTrackKind::SessionSnapshot,
        refs.clone(),
        vec![result_ref.to_string()],
        "2026-05-23T00:00:00Z",
        &json!({
            "snapshot_id":"system-summary-local-multi-agent",
            "session_id":"system-agent",
            "task_id": refs.task_id,
            "user_summary": user_request,
            "assistant_summary": result_summary,
            "important_refs":[result_ref]
        }),
    )?;
    append_event(
        &system,
        refs,
        "2026-05-23T00:00:00Z",
        "system.summary.reported",
        json!({
            "source_agent_id": message.from_agent_id,
            "summary": result_summary,
            "result_refs": [summary_ref, result_ref]
        }),
    )?;
    write_json(
        &runtime_home.join("runtime/agents/state/system-node-summary.json"),
        &json!({
            "agent_id": agent_id,
            "consumed_message_id": message.message_id,
            "result_ref": result_ref,
            "summary": result_summary,
            "source":"system_node"
        }),
    )?;
    Ok(())
}

fn receive_project_dispatch_over_rpc(
    runtime_home: &Path,
    agent_id: &str,
) -> Result<Option<fin_runtime::AgentMailboxMessage>, CliError> {
    receive_dispatch_over_rpc(runtime_home, agent_id)
}

fn receive_dispatch_over_rpc(
    runtime_home: &Path,
    agent_id: &str,
) -> Result<Option<fin_runtime::AgentMailboxMessage>, CliError> {
    let control = AgentControlStore::new(runtime_home);
    control
        .consume_next_mailbox_message(agent_id, "local_node_consumed")
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)
}

fn resolve_project_config_path(user_config_path: Option<&Path>) -> Result<&Path, CliError> {
    let static_llm = std::env::var("FIN_LOCAL_MULTI_AGENT_STATIC_LLM")
        .ok()
        .as_deref()
        == Some("1");
    let static_config_path = Path::new("static-test-user.toml");
    user_config_path
        .or(if static_llm {
            Some(static_config_path)
        } else {
            None
        })
        .ok_or_else(|| {
            CliError::Runtime(RuntimeError::State(
                "project harness node missing user config path for real LLM execution".into(),
            ))
        })
}

fn write_project_result(
    runtime_home: &Path,
    agent_id: &str,
    agent_run_id: &str,
    to_agent_id: &str,
    task_id: Option<&str>,
    cwd: &Path,
    cwd_verified: bool,
    observed_files: &[String],
    llm_execution: &serde_json::Value,
) -> Result<(), CliError> {
    let control = AgentControlStore::new(runtime_home);
    control
        .send_agent_input(SendAgentInput {
            message_id: format!("msg-progress-{agent_run_id}"),
            from_agent_id: agent_id.to_string(),
            to_agent_id: to_agent_id.to_string(),
            thread_id: Some("thread-local-multi-agent".into()),
            task_id: task_id.map(str::to_string),
            trigger_turn: false,
            payload: json!({
                "kind": "project_progress",
                "agent_run_id": agent_run_id,
                "from_agent_id": agent_id,
                "task_id": task_id,
                "cwd": cwd,
                "cwd_verified": cwd_verified,
                "events": ["turn.started", "tool.completed", "provider.completed", "turn.completed"],
                "llm_execution": llm_execution
            }),
        })
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)?;
    let result_ref = "child-node://project-fin-agent/result/task-local-multi-agent";
    control
        .send_agent_input(SendAgentInput {
            message_id: format!("msg-result-{agent_run_id}"),
            from_agent_id: agent_id.to_string(),
            to_agent_id: to_agent_id.to_string(),
            thread_id: Some("thread-local-multi-agent".into()),
            task_id: task_id.map(str::to_string),
            trigger_turn: true,
            payload: json!({
                "kind": "project_result",
                "agent_run_id": agent_run_id,
                "from_agent_id": agent_id,
                "task_id": task_id,
                "cwd": cwd,
                "cwd_verified": cwd_verified,
                "observed_files": observed_files,
                "llm_execution": llm_execution,
                "result_summary": "project agent completed real LLM execution against configured cwd",
                "result_ref": result_ref
            }),
        })
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)?;
    control
        .update_run_status(
            agent_run_id,
            "completed",
            vec![result_ref.to_string()],
            "2026-05-23T00:00:00Z",
        )
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use fin_shared::exponential_backoff;
    use std::time::Duration;

    #[test]
    fn node_retry_uses_exponential_backoff_schedule() {
        assert_eq!(exponential_backoff(1), Duration::from_secs(1));
        assert_eq!(exponential_backoff(2), Duration::from_secs(2));
        assert_eq!(exponential_backoff(3), Duration::from_secs(4));
        assert_eq!(exponential_backoff(4), Duration::from_secs(8));
        assert_eq!(exponential_backoff(5), Duration::from_secs(16));
    }
}

fn observe_project_cwd(cwd: &Path) -> Result<Vec<String>, CliError> {
    let mut observed = Vec::new();
    let entries = fs::read_dir(cwd).map_err(|source| CliError::ReadFile {
        path: cwd.display().to_string(),
        source,
    })?;
    for entry in entries.take(12) {
        let entry = entry.map_err(|source| CliError::ReadFile {
            path: cwd.display().to_string(),
            source,
        })?;
        if let Some(name) = entry.file_name().to_str() {
            observed.push(name.to_string());
        }
    }
    observed.sort();
    Ok(observed)
}
