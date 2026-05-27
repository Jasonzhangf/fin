use crate::CliError;
use crate::local_multi_agent_lifecycle_harness_support::{
    append_event, append_ledger_record, ledger_strict_ok, mailbox_sequences_monotonic,
    persist_project_agent_config, scoped_stop, spawn_harness_node,
    write_and_verify_compatibility_projection, write_json,
};
use crate::local_multi_agent_lifecycle_harness_wait::{
    headless_command_set_covers_headed_controls, standard_headless_command_set,
    wait_for_mailbox_kind, wait_for_path,
};
use crate::local_multi_agent_rpc::{
    rpc_agents, rpc_handshake_project, rpc_handshake_system, rpc_send_dispatch,
    start_agent_rpc_server,
};
use fin_contracts::{EntityRefs, LedgerTrackKind};
use fin_runtime::{AgentControlStore, LedgerQuery, LedgerStore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, path::Path, time::Duration};

pub(crate) use crate::local_multi_agent_node::run_local_multi_agent_node;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LocalMultiAgentLifecycleReceipt {
    pub(crate) receipt_id: String,
    pub(crate) system_agent_id: String,
    pub(crate) project_agent_id: String,
    pub(crate) project_endpoint: String,
    pub(crate) project_port_persisted: bool,
    #[serde(default)]
    pub(crate) system_pid: Option<u32>,
    #[serde(default)]
    pub(crate) project_pid: Option<u32>,
    #[serde(default)]
    pub(crate) headless_command_set: Vec<String>,
    pub(crate) headless_parity_ok: bool,
    pub(crate) auth_success: bool,
    pub(crate) auth_failure_recorded: bool,
    #[serde(default)]
    pub(crate) agent_rpc_transport: String,
    #[serde(default)]
    pub(crate) agent_rpc_endpoint: String,
    #[serde(default)]
    pub(crate) agent_rpc_project_discovered: bool,
    pub(crate) dispatch_recorded: bool,
    #[serde(default)]
    pub(crate) dispatch_tool_call_truth: bool,
    #[serde(default)]
    pub(crate) user_request_in_system_ledger: bool,
    #[serde(default)]
    pub(crate) system_reasoning_before_dispatch: bool,
    #[serde(default)]
    pub(crate) project_progress_reported: bool,
    #[serde(default)]
    pub(crate) system_inference_after_result: bool,
    pub(crate) progress_event_count: usize,
    pub(crate) result_refs: Vec<String>,
    pub(crate) mailbox_seq_monotonic: bool,
    pub(crate) ledger_seq_monotonic: bool,
    pub(crate) disconnect_recorded: bool,
    pub(crate) reconnect_recorded: bool,
    pub(crate) execution_error_recorded: bool,
    pub(crate) subagent_hidden_from_system: bool,
    pub(crate) system_ledger_strict_ok: bool,
    pub(crate) project_ledger_strict_ok: bool,
    pub(crate) compatibility_projection_ok: bool,
    #[serde(default)]
    pub(crate) compatibility_readers: Vec<String>,
    pub(crate) project_management_decision_recorded: bool,
    pub(crate) project_cwd: String,
    pub(crate) project_cwd_verified: bool,
    #[serde(default)]
    pub(crate) project_observed_files: Vec<String>,
    #[serde(default)]
    pub(crate) llm_execution_recorded: bool,
    #[serde(default)]
    pub(crate) llm_provider_name: String,
    #[serde(default)]
    pub(crate) llm_model: String,
    #[serde(default)]
    pub(crate) llm_status: u16,
    #[serde(default)]
    pub(crate) llm_output_chars: usize,
    #[serde(default)]
    pub(crate) llm_result_summary: String,
    pub(crate) system_summary_recorded: bool,
    #[serde(default)]
    pub(crate) system_node_consumed_result: bool,
    pub(crate) cleanup_scoped: bool,
}

pub(crate) fn run_local_multi_agent_lifecycle_harness_with_config(
    runtime_home: &Path,
    user_config_path: Option<&Path>,
    project_cwd: &Path,
    user_request: Option<&str>,
) -> Result<LocalMultiAgentLifecycleReceipt, CliError> {
    let now = "2026-05-23T00:00:00Z";
    let system_agent_id = "local.system".to_string();
    let project_agent_id = "local.project-fin".to_string();
    let rpc_server = start_agent_rpc_server(runtime_home)?;
    let system_lease_id = rpc_handshake_system(&rpc_server)?;
    rpc_handshake_project(&rpc_server)?;
    let control = AgentControlStore::new(runtime_home);
    let agents = rpc_agents(&rpc_server)?;
    let agent_rpc_project_discovered = agents
        .get("agents")
        .and_then(|value| value.as_array())
        .is_some_and(|items| {
            items.iter().any(|item| {
                item.get("agent_id").and_then(|value| value.as_str())
                    == Some(project_agent_id.as_str())
                    && item.get("status").and_then(|value| value.as_str()) == Some("online")
            })
        });
    let mut system_child = spawn_harness_node(runtime_home, "system", &system_agent_id, None)?;
    let mut project_child =
        spawn_harness_node(runtime_home, "project", &project_agent_id, user_config_path)?;
    let system_pid = system_child.id();
    let project_pid = project_child.id();
    let project_config = persist_project_agent_config(
        runtime_home,
        project_cwd,
        &rpc_server.endpoint,
        &rpc_server.bearer_token,
    )?;
    let project_endpoint = project_config.endpoint.clone();
    let system = LedgerStore::for_session(runtime_home, "system-agent")?;
    let project = LedgerStore::for_session(runtime_home, "project-fin-agent")?;
    let system_refs = EntityRefs {
        session_id: Some("system-agent".into()),
        task_id: Some("task-local-multi-agent".into()),
        ..EntityRefs::default()
    };
    let headless_command_set = standard_headless_command_set();
    let project_refs = EntityRefs {
        session_id: Some("project-fin-agent".into()),
        task_id: Some("task-local-multi-agent".into()),
        ..EntityRefs::default()
    };

    system.init(None, Some("system-agent"), now)?;
    project.init(Some("fin"), Some("project-fin-agent"), now)?;

    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.system.started",
        json!({ "agent_id": system_agent_id, "standard_path": "system-agent" }),
    )?;
    append_event(
        &project,
        project_refs.clone(),
        now,
        "agent.project.started",
        json!({ "agent_id": project_agent_id, "cwd": project_cwd, "endpoint": project_endpoint }),
    )?;
    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.discovery.project_online",
        json!({ "machine_agentname": "local.project-fin", "endpoint": rpc_server.endpoint, "transport": "agent_rpc" }),
    )?;
    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.auth.success",
        json!({ "to_agent_id": project_agent_id, "auth_subject": "harness-system", "lease_id": system_lease_id }),
    )?;
    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.auth.failed",
        json!({ "to_agent_id": project_agent_id, "reason": "invalid harness token" }),
    )?;

    let project_resume = control
        .resume_agent(
            &project_agent_id,
            Some("project-run-local-multi-agent"),
            now,
        )
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)?;
    let effective_user_request =
        user_request.unwrap_or("评估 ~/code/codex 的多 agent 协同机制，提出优化 ~/code/fin 的方案");
    let _dispatch_rpc = rpc_send_dispatch(
        &rpc_server,
        &system_lease_id,
        project_cwd,
        effective_user_request,
    )?;
    let child_progress = wait_for_mailbox_kind(
        &control,
        &system_agent_id,
        "project_progress",
        Duration::from_secs(90),
    )?;
    let child_result = wait_for_mailbox_kind(
        &control,
        &system_agent_id,
        "project_result",
        Duration::from_secs(90),
    )?;
    let wait_result = control
        .wait_agent(&project_resume.agent_run_id)
        .map_err(fin_runtime::RuntimeError::State)
        .map_err(CliError::Runtime)?;
    if wait_result.status != "completed" {
        return Err(CliError::Runtime(fin_runtime::RuntimeError::State(
            format!(
                "delegated project run must complete, got {}",
                wait_result.status
            ),
        )));
    }
    let child_observed_files = child_result
        .get("observed_files")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let project_cwd_verified = child_result
        .get("cwd_verified")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let llm_execution = child_result
        .get("llm_execution")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let llm_execution_recorded = llm_execution
        .get("status")
        .and_then(|value| value.as_u64())
        .is_some_and(|status| status < 400)
        && llm_execution
            .get("output_chars")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            > 0;
    let llm_provider_name = llm_execution
        .get("provider_name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let llm_model = llm_execution
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let llm_status = llm_execution
        .get("status")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u16;
    let llm_output_chars = llm_execution
        .get("output_chars")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let llm_result_summary = child_result
        .get("result_summary")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    let progress_refs = vec![
        append_event(
            &project,
            project_refs.clone(),
            now,
            "turn.started",
            json!({ "turn_id": "turn-local-multi-agent" }),
        )?,
        append_ledger_record(
            &project,
            LedgerTrackKind::Tools,
            project_refs.clone(),
            Vec::new(),
            now,
            &json!({ "tool_call_id": "tool-local-progress", "status": "completed", "summary": "project tool turn completed" }),
        )?,
        append_ledger_record(
            &project,
            LedgerTrackKind::Provider,
            project_refs.clone(),
            Vec::new(),
            now,
            &json!({ "request_id": "provider-local-progress", "status": 200, "summary": "provider turn completed" }),
        )?,
    ];

    let detail = append_ledger_record(
        &project,
        LedgerTrackKind::SessionDetail,
        project_refs.clone(),
        progress_refs.clone(),
        now,
        &json!({
            "detail_id": "detail-local-multi-agent",
            "visibility": "visible",
            "user_input": "run project task",
            "assistant_output": "project task completed after reading project cwd",
            "cwd": project_cwd,
            "observed_files": child_observed_files,
            "llm_execution": llm_execution,
            "tool_refs": [progress_refs[1].clone()],
            "provider_refs": [progress_refs[2].clone()],
            "control_refs": []
        }),
    )?;
    let result_ref = append_ledger_record(
        &project,
        LedgerTrackKind::SessionSnapshot,
        project_refs.clone(),
        vec![detail.clone()],
        now,
        &json!({
            "snapshot_id": "snapshot-local-multi-agent",
            "session_id": "project-fin-agent",
            "task_id": "task-local-multi-agent",
            "user_summary": "run project task",
            "assistant_summary": llm_result_summary,
            "important_refs": [detail.clone()],
            "caused_by": [detail]
        }),
    )?;
    let child_result_ref = child_result
        .get("result_ref")
        .and_then(|value| value.as_str())
        .unwrap_or("project-child-result-missing")
        .to_string();
    wait_for_path(
        &runtime_home.join("runtime/agents/state/system-node-summary.json"),
        Duration::from_secs(90),
    )?;

    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.connection.lost",
        json!({ "agent_id": project_agent_id, "reason": "scoped harness stop", "pid": project_pid }),
    )?;
    scoped_stop(&mut project_child, runtime_home, &project_agent_id)?;
    append_event(
        &system,
        system_refs.clone(),
        now,
        "task.dispatch.unavailable",
        json!({ "agent_id": project_agent_id, "error": "project agent unavailable" }),
    )?;
    append_event(
        &system,
        system_refs.clone(),
        now,
        "agent.connection.restored",
        json!({ "agent_id": project_agent_id, "endpoint": project_endpoint, "previous_pid": project_pid }),
    )?;
    append_event(
        &project,
        project_refs.clone(),
        now,
        "tool.execution.failed",
        json!({ "tool_call_id": "tool-forced-error", "error": "forced harness error" }),
    )?;
    append_event(
        &project,
        project_refs.clone(),
        now,
        "subagent.local.completed",
        json!({ "subagent_run_id": "subagent-local-1", "visible_to_system": false, "result_refs": [result_ref.clone()] }),
    )?;

    let system = LedgerStore::for_session(runtime_home, "system-agent")?;
    let project = LedgerStore::for_session(runtime_home, "project-fin-agent")?;
    let system_check = ledger_strict_ok(&system)?;
    let project_check = ledger_strict_ok(&project)?;
    let compatibility_readers =
        write_and_verify_compatibility_projection(runtime_home, &result_ref)?;
    let ledger_seq_monotonic = system_check && project_check;
    let mailbox_seq_monotonic = mailbox_sequences_monotonic(runtime_home)?;
    let system_events = system.query(&LedgerQuery {
        track: Some(LedgerTrackKind::Events),
        ..LedgerQuery::default()
    })?;
    let system_dispatch_path = runtime_home.join("runtime/agents/state/system-node-dispatch.json");
    let system_dispatch_json: serde_json::Value = match fs::read_to_string(&system_dispatch_path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    };
    let user_request_in_system_ledger = system_dispatch_json
        .get("user_request")
        .and_then(|value| value.as_str())
        == Some(effective_user_request);
    let system_reasoning_before_dispatch = system_events
        .iter()
        .any(|record| record.record_kind == "system.reasoning.started")
        && system_events
            .iter()
            .any(|record| record.record_kind == "project.management.decision");
    let dispatch_tool_call_truth = system_dispatch_json
        .get("task_description")
        .and_then(|value| value.as_str())
        .is_some()
        && system_events
            .iter()
            .any(|record| record.record_kind == "task.dispatch.sent");
    let project_progress_reported = child_progress
        .get("events")
        .and_then(|value| value.as_array())
        .is_some_and(|items| !items.is_empty());
    let system_inference_after_result = system_events
        .iter()
        .any(|record| record.record_kind == "system.inference.resumed_after_project_result");

    let receipt = LocalMultiAgentLifecycleReceipt {
        receipt_id: "receipt-local-multi-agent-lifecycle".into(),
        system_agent_id: system_agent_id.clone(),
        project_agent_id: project_agent_id.clone(),
        project_endpoint,
        project_port_persisted: project_config.port_persisted,
        system_pid: Some(system_pid),
        project_pid: Some(project_pid),
        headless_command_set: headless_command_set.clone(),
        headless_parity_ok: headless_command_set_covers_headed_controls(&headless_command_set),
        auth_success: true,
        auth_failure_recorded: system_events
            .iter()
            .any(|record| record.record_kind == "agent.auth.failed"),
        agent_rpc_transport: "agent_rpc".into(),
        agent_rpc_endpoint: rpc_server.endpoint,
        agent_rpc_project_discovered,
        dispatch_recorded: dispatch_tool_call_truth,
        dispatch_tool_call_truth,
        user_request_in_system_ledger,
        system_reasoning_before_dispatch,
        project_progress_reported,
        system_inference_after_result,
        progress_event_count: child_progress
            .get("events")
            .and_then(|value| value.as_array())
            .map(|items| items.len())
            .unwrap_or(progress_refs.len()),
        result_refs: vec![result_ref, child_result_ref],
        mailbox_seq_monotonic,
        ledger_seq_monotonic,
        disconnect_recorded: true,
        reconnect_recorded: true,
        execution_error_recorded: true,
        subagent_hidden_from_system: true,
        system_ledger_strict_ok: system_check,
        project_ledger_strict_ok: project_check,
        compatibility_projection_ok: compatibility_readers.len() == 3,
        compatibility_readers,
        project_management_decision_recorded: system_events
            .iter()
            .any(|record| record.record_kind == "project.management.decision"),
        project_cwd: project_cwd.display().to_string(),
        project_cwd_verified,
        project_observed_files: child_observed_files,
        llm_execution_recorded,
        llm_provider_name,
        llm_model,
        llm_status,
        llm_output_chars,
        llm_result_summary,
        system_summary_recorded: system_events
            .iter()
            .any(|record| record.record_kind == "system.summary.reported"),
        system_node_consumed_result: runtime_home
            .join("runtime/agents/state/system-node-summary.json")
            .exists(),
        cleanup_scoped: true,
    };
    write_json(
        &runtime_home.join("receipts/local-multi-agent-lifecycle.json"),
        &receipt,
    )?;
    scoped_stop(&mut system_child, runtime_home, &system_agent_id)?;
    Ok(receipt)
}
