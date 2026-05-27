use crate::local_multi_agent_lifecycle_harness::run_local_multi_agent_lifecycle_harness_with_config;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fin-local-multi-agent-{prefix}-{unique}"));
    fs::create_dir_all(&path).expect("temp dir");
    path
}

#[test]
fn local_multi_agent_harness_static_mode_only_validates_harness_lifecycle_logic() {
    unsafe {
        std::env::set_var("FIN_LOCAL_MULTI_AGENT_STATIC_LLM", "1");
    }
    let runtime_home = temp_dir("runtime");
    let project_cwd = temp_dir("project");
    fs::write(project_cwd.join("AGENTS.md"), "# harness project rules\n")
        .expect("project sentinel");
    let receipt = run_local_multi_agent_lifecycle_harness_with_config(
        &runtime_home,
        None,
        &project_cwd,
        None,
    )
    .expect("local multi-agent lifecycle harness must complete");

    assert!(receipt.auth_success, "auth success path must be covered");
    assert!(
        receipt.system_pid.is_some(),
        "system instance PID must be recorded"
    );
    assert!(
        receipt.project_pid.is_some(),
        "project instance PID must be recorded"
    );
    assert_ne!(
        receipt.system_pid, receipt.project_pid,
        "system/project must be distinct local instances"
    );
    assert!(
        receipt.headless_parity_ok,
        "headless project agent must expose headed-agent parity command set"
    );
    assert!(
        receipt.project_endpoint.starts_with("127.0.0.1:"),
        "project endpoint must be a local listener endpoint"
    );
    assert_ne!(
        receipt.project_endpoint, "127.0.0.1:0",
        "project agent port must be auto-selected before persistence"
    );
    assert!(
        receipt.project_port_persisted,
        "auto-selected project port must be persisted"
    );
    assert!(
        receipt
            .headless_command_set
            .iter()
            .any(|command| command.contains("--cwd"))
    );
    assert!(
        receipt
            .headless_command_set
            .iter()
            .any(|command| command.contains("--session"))
    );
    assert!(
        receipt
            .headless_command_set
            .iter()
            .any(|command| command.contains("slash"))
    );
    assert!(
        receipt
            .headless_command_set
            .iter()
            .any(|command| command.contains("control status"))
    );
    assert!(
        receipt.auth_failure_recorded,
        "auth failure must be recorded"
    );
    assert_eq!(receipt.agent_rpc_transport, "agent_rpc");
    assert!(
        receipt.agent_rpc_endpoint.starts_with("127.0.0.1:"),
        "agent RPC endpoint must be a real local listener"
    );
    assert!(
        receipt.agent_rpc_project_discovered,
        "system must enumerate project agent through Agent RPC"
    );
    assert!(
        receipt.dispatch_recorded,
        "system dispatch must be recorded"
    );
    assert!(
        receipt.progress_event_count >= 3,
        "turn/tool/result progress events expected"
    );
    assert!(
        !receipt.result_refs.is_empty(),
        "project result refs must return to system"
    );
    assert!(
        receipt.mailbox_seq_monotonic,
        "mailbox seq must be monotonic"
    );
    assert!(receipt.ledger_seq_monotonic, "ledger seq must be monotonic");
    assert!(
        receipt.disconnect_recorded,
        "lost connection must be recorded"
    );
    assert!(receipt.reconnect_recorded, "reconnect must be recorded");
    assert!(
        receipt.execution_error_recorded,
        "forced execution error must be recorded"
    );
    assert!(
        receipt.subagent_hidden_from_system,
        "system must not directly discover project subagent"
    );
    assert!(
        receipt.system_ledger_strict_ok,
        "system ledger strict check must pass"
    );
    assert!(
        receipt.project_ledger_strict_ok,
        "project ledger strict check must pass"
    );
    assert!(
        receipt.compatibility_projection_ok,
        "old projection readers must remain compatible"
    );
    assert!(
        receipt.project_management_decision_recorded,
        "system must record project-management decision before delegating cwd work"
    );
    assert!(
        receipt.project_cwd_verified,
        "project agent must verify it executed against the configured cwd"
    );
    assert!(
        receipt
            .project_observed_files
            .iter()
            .any(|item| item == "AGENTS.md" || item == "README.md"),
        "project agent must observe real files from configured cwd"
    );
    assert!(
        receipt.llm_execution_recorded,
        "static harness mode must still record synthetic LLM-shaped result"
    );
    assert_eq!(receipt.llm_provider_name, "static-test-provider");
    assert_eq!(receipt.llm_model, "static-test-model");
    assert_eq!(receipt.llm_status, 200);
    assert!(
        receipt.llm_output_chars > 0,
        "LLM result must contain model output text"
    );
    assert!(
        receipt.llm_result_summary.contains("real LLM execution"),
        "summary must state the project agent completed LLM execution"
    );
    assert!(
        receipt.system_summary_recorded,
        "system agent must record summary after project result returns"
    );
    assert!(
        receipt.system_node_consumed_result,
        "system node itself must consume project result and publish summary; harness parent must not fake final orchestration"
    );
    assert!(
        receipt.user_request_in_system_ledger,
        "user request must first land in system ledger truth"
    );
    assert!(
        receipt.system_reasoning_before_dispatch,
        "system must record reasoning/decision before dispatch"
    );
    assert!(
        receipt.dispatch_tool_call_truth,
        "dispatch must be recorded as tool-call truth, not harness-prebaked event"
    );
    assert!(
        receipt.project_progress_reported,
        "project agent must report progress before final result"
    );
    assert!(
        receipt.system_inference_after_result,
        "system must continue inference after project result instead of harness direct closeout"
    );
    assert_eq!(
        receipt.compatibility_readers,
        vec![
            "status_probe".to_string(),
            "web_debug_session_messages".to_string(),
            "qqbot_conversations".to_string()
        ],
        "compatibility readers must cover status, web debug, and qqbot paths"
    );
    assert!(
        receipt.cleanup_scoped,
        "cleanup must be scoped to harness-owned explicit resources"
    );

    assert!(
        runtime_home
            .join("receipts/local-multi-agent-lifecycle.json")
            .exists()
    );

    let system_summary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/agents/state/system-node-summary.json"))
            .expect("system summary"),
    )
    .expect("system summary json");
    assert_eq!(
        system_summary["source"].as_str(),
        Some("system_node"),
        "system node summary must be emitted by system node itself"
    );

    let project_last_error_path =
        runtime_home.join("runtime/agents/state/local.project-fin.last_error.json");
    if project_last_error_path.exists() {
        let project_last_error =
            fs::read_to_string(&project_last_error_path).expect("project last error");
        assert!(
            project_last_error.contains("project_inbox_error")
                || project_last_error.contains("agent rpc"),
            "project last_error must describe inbox or RPC failure"
        );
    }

    let system_last_error_path =
        runtime_home.join("runtime/agents/state/local.system.last_error.json");
    if system_last_error_path.exists() {
        let system_last_error =
            fs::read_to_string(&system_last_error_path).expect("system last error");
        assert!(
            system_last_error.contains("system_inbox_error")
                || system_last_error.contains("agent rpc"),
            "system last_error must describe inbox or RPC failure"
        );
    }

    let runs = fs::read_to_string(runtime_home.join("runtime/agents/control/runs.json"))
        .expect("runs registry");
    assert!(
        runs.contains("\"status\": \"completed\""),
        "delegated run must reach completed terminal state"
    );
}
