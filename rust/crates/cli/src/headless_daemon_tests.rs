use crate::{
    config::map_system_config,
    fs_utils::write_file,
    headless_daemon::{run_headless_daemon_with_provider, stop_headless_daemon},
    runtime_home::ensure_runtime_home_layout,
    test_env::env_lock,
};
use fin_config::SystemConfig;
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
struct HeadlessResumeProvider {
    descriptor: ProviderDescriptor,
}

impl HeadlessResumeProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl InferenceProvider for HeadlessResumeProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor.prepare_request(request)
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, fin_provider::ProviderError> {
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: "<fin_user_response>后台守护已恢复并完成本轮检查。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-auto-resume\",\"candidate_topic_thread_id\":\"topic-auto-resume\",\"continuity_confidence\":94,\"topic_shift_confidence\":4,\"simple_query_confidence\":4,\"previous_topic_summary\":\"waiting\",\"current_topic_summary\":\"waiting\",\"note_candidate\":\"daemon resume finished\",\"digest_candidate\":\"daemon resume finished\",\"reason\":\"headless daemon checkpoint resume completed\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"headless checkpoint resume complete\"}}]</fin_tool_calls>".into(),
            response_id: Some("headless-resume-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

#[derive(Debug, Clone)]
struct HeadlessProjectProvider {
    descriptor: ProviderDescriptor,
}

impl HeadlessProjectProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl InferenceProvider for HeadlessProjectProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor.prepare_request(request)
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, fin_provider::ProviderError> {
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: "<fin_user_response>project worker finished detached headless task.</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-fin-1\",\"candidate_topic_thread_id\":\"topic-fin-1\",\"continuity_confidence\":96,\"topic_shift_confidence\":3,\"simple_query_confidence\":4,\"previous_topic_summary\":\"project work\",\"current_topic_summary\":\"project work\",\"note_candidate\":\"headless project resume completed\",\"digest_candidate\":\"headless project resume completed\",\"reason\":\"detached daemon resumed local project agent\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.submit\",\"arguments\":{\"task_id\":\"task-fin-1\",\"result_summary\":\"detached daemon completed project task\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"headless project task complete\"}}]</fin_tool_calls>".into(),
            response_id: Some("headless-project-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

fn sample_user_toml() -> String {
    r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
    .into()
}

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-headless-daemon-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

#[test]
fn headless_daemon_cycle_resumes_checkpoint_without_frontstage() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES").ok();
    let previous_bind = std::env::var("FIN_DAEMON_CONTROL_PLANE_BIND").ok();
    unsafe {
        std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", "1");
        std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", "127.0.0.1:0");
    }

    let home = temp_runtime_home("resume");
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let user_toml = sample_user_toml();
    let system = map_system_config(&user_toml).expect("system config");
    let session_dir = home.join("sessions/2026/05/session-checkpoint-resume");
    for relative in [
        "conversation",
        "control",
        "queue",
        "tasks/routing",
        "digests",
        "context",
        "reasoning",
        "tools",
        "events",
    ] {
        fs::create_dir_all(session_dir.join(relative)).expect("session dir");
    }
    fs::create_dir_all(home.join("runtime/reminders")).expect("reminders dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(&session_dir.join("events/stream.jsonl"), b"").expect("events");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-waiting",
  "session_id":"session-checkpoint-resume",
  "task_id":"task-auto-resume",
  "status":"waiting_external",
  "active_turn_id":"turn-op-waiting",
  "active_step_id":"step-op-waiting-05-finalize",
  "resume_from_step_id":"step-op-waiting-04-tool_dispatch",
  "resume_checkpoint_ready":true,
  "resume_checkpoint_id":"checkpoint-op-waiting-r02",
  "pending_input_count":0,
  "accepts_user_input":true,
  "reason":"waiting for reminder or external result; resumable checkpoint ready",
  "updated_at":"2026-04-20T10:00:00+08:00"
}"#,
    )
    .expect("state");
    write_file(
        &session_dir.join("control/execution_checkpoint.json"),
        "{
  \"checkpoint_id\":\"checkpoint-op-waiting-r02\",
  \"session_id\":\"session-checkpoint-resume\",
  \"task_id\":\"task-auto-resume\",
  \"trace_id\":\"trace-op-waiting\",
  \"source_operation_id\":\"op-waiting\",
  \"source_turn_id\":\"turn-op-waiting\",
  \"source_step_id\":\"step-op-waiting-04-tool_dispatch\",
  \"checkpoint_kind\":\"wait_reminder_resume\",
  \"status\":\"open\",
  \"source_round_index\":1,
  \"next_round_index\":2,
  \"resume_input\":\"Continue the same turn with the latest tool results.\\nOriginal request: 等两分钟再看日志\\nLast assistant response: 先等待日志完成。\\nExecuted tool results (authoritative client facts):\\n- tool=wait.remind status=completed target=role=system result=scheduled system self reminder in 2 minute(s)\\nInspect these tool results before deciding whether another tool is needed. If the task is complete, answer directly and emit reasoning.stop.\",
  \"summary\":\"resume round 2 after wait.remind from round 1\",
  \"created_at\":\"2026-04-20T10:00:00+08:00\",
  \"consumed_at\":null,
  \"consumed_by_operation_id\":null
}"
        .as_bytes(),
    )
    .expect("checkpoint");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-op-waiting","decision_id":"routing-op-waiting","operation_id":"op-waiting","trace_id":"trace-op-waiting","session_id":"session-checkpoint-resume","task_id":"task-auto-resume","created_at":"2026-04-20T10:00:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-auto-resume","suggested_topic_thread_id":null,"confidence":95,"reason":"same task"}"#,
    )
    .expect("routing action");
    write_file(
        &home.join("runtime/reminders/pending.json"),
        format!(
            r#"[{{"reminder_id":"reminder-1","session_id":"session-checkpoint-resume","task_id":"task-auto-resume","operation_id":"op-waiting","trace_id":"trace-op-waiting","wait_minutes":1,"reminder":"check wait result","wake_role":"system","scheduled_at":"{}","status":"pending","fired_at":null}}]"#,
            (chrono::Local::now() - chrono::Duration::minutes(2))
                .format("%Y-%m-%dT%H:%M:%S%:z")
        )
        .as_bytes(),
    )
    .expect("reminders");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-checkpoint-resume",
  "task_id":"task-auto-resume",
  "session_messages_path":"sessions/2026/05/session-checkpoint-resume/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/05/session-checkpoint-resume/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/05/session-checkpoint-resume/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/05/session-checkpoint-resume/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/05/session-checkpoint-resume/tools/recent_tool_records.json"
}"#,
    )
    .expect("last_run");

    let report = run_headless_daemon_with_provider(
        &user_toml,
        &system,
        &HeadlessResumeProvider::new(&system),
        home.clone(),
    )
    .expect("headless daemon run");

    assert_eq!(report.cycles_completed, 1);
    let checkpoint = fs::read_to_string(session_dir.join("control/execution_checkpoint.json"))
        .expect("checkpoint");
    assert!(checkpoint.contains("\"status\": \"consumed\""));
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("Reminder"));
    assert!(messages.contains("后台守护已恢复并完成本轮检查"));
    assert!(!messages.contains("Continue the same turn with the latest tool results."));
    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("execution.checkpoint_consumed"));
    assert!(events.contains("reasoning.stopped"));
    let daemon_state =
        fs::read_to_string(home.join("runtime/current/current_daemon_state.json")).expect("state");
    assert!(daemon_state.contains("\"service_kind\": \"headless_daemon\""));
    assert!(daemon_state.contains("\"mode\": \"detached\""));
    let lease =
        fs::read_to_string(home.join("runtime/leases/headless-daemon.json")).expect("lease");
    assert!(lease.contains("\"processed_sessions\": 1"));
    assert!(lease.contains("session-checkpoint-resume"));

    if let Some(value) = previous {
        unsafe {
            std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
        }
    }
    if let Some(value) = previous_bind {
        unsafe {
            std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_DAEMON_CONTROL_PLANE_BIND");
        }
    }
}

#[test]
fn headless_daemon_cycle_autonomously_resumes_local_project_agent_without_frontstage() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES").ok();
    let previous_bind = std::env::var("FIN_DAEMON_CONTROL_PLANE_BIND").ok();
    unsafe {
        std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", "1");
        std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", "127.0.0.1:0");
    }

    let home = temp_runtime_home("project-detached");
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let user_toml = sample_user_toml();
    let mut system = map_system_config(&user_toml).expect("system config");
    system.runtime.device_name = Some("mbp".into());
    system
        .runtime
        .startup
        .project_agents
        .push(fin_config::ProjectAgentStartupConfig {
            project_id: "fin".into(),
            mode: fin_config::ProjectAgentMode::Local,
            project_root: Some("/tmp/fin".into()),
            endpoint: None,
            agent_name: Some("builder".into()),
            worker_budget: 2,
            always_on: true,
            auto_resume: true,
            auto_connect: true,
        });

    let session_dir = home.join("sessions/2026/05/session-fin-detached");
    for relative in [
        "conversation",
        "control",
        "queue",
        "tasks/registry",
        "tasks/board",
        "tasks/routing",
        "digests",
        "context",
        "reasoning",
        "tools",
        "events",
    ] {
        fs::create_dir_all(session_dir.join(relative)).expect("session dir");
    }
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(&session_dir.join("events/stream.jsonl"), b"").expect("events");
    write_file(
        &session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin","label":"fin"}}}"#,
    )
    .expect("current context");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("control/execution_state.json"),
        serde_json::to_vec_pretty(&json!({
            "state_id":"exec-state-fin-detached",
            "session_id":"session-fin-detached",
            "task_id":"task-fin-1",
            "status":"idle",
            "pending_input_count":0,
            "accepts_user_input":true,
            "updated_at":"2026-04-21T00:10:00+08:00"
        }))
        .expect("state json")
        .as_slice(),
    )
    .expect("state");
    write_file(
        &session_dir.join("tasks/registry/task-fin-1.json"),
        serde_json::to_vec_pretty(&json!({
            "task_id":"task-fin-1",
            "session_id":"session-fin-detached",
            "title":"Detached task",
            "summary":"autonomous local project work",
            "status":"ready",
            "review_owner_worker_id":"worker-system",
            "created_at":"2026-04-21T00:10:00+08:00",
            "updated_at":"2026-04-21T00:11:00+08:00"
        }))
        .expect("task json")
        .as_slice(),
    )
    .expect("task");
    write_file(
        &home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&json!({
            "session_id":"session-fin-detached",
            "task_id":"task-fin-1",
            "session_messages_path":"sessions/2026/05/session-fin-detached/conversation/messages.json",
            "session_recent_contexts_path":"sessions/2026/05/session-fin-detached/context/recent_contexts.json",
            "session_recent_digests_path":"sessions/2026/05/session-fin-detached/digests/recent_digests.json",
            "session_recent_reasoning_path":"sessions/2026/05/session-fin-detached/reasoning/recent_reasoning_views.json",
            "session_recent_tool_records_path":"sessions/2026/05/session-fin-detached/tools/recent_tool_records.json"
        }))
        .expect("last run")
        .as_slice(),
    )
    .expect("last run");

    let report = run_headless_daemon_with_provider(
        &user_toml,
        &system,
        &HeadlessProjectProvider::new(&system),
        home.clone(),
    )
    .expect("headless daemon run");

    assert_eq!(report.cycles_completed, 1);
    assert!(report.drove_count >= 1);

    let task_json =
        fs::read_to_string(session_dir.join("tasks/registry/task-fin-1.json")).expect("task json");
    assert!(task_json.contains("\"status\": \"submitted\""));
    assert!(task_json.contains("\"submitted_by_worker_id\": \"worker-builder\""));
    assert!(task_json.contains("detached daemon completed project task"));

    let queue =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending queue");
    assert_eq!(queue.trim(), "[]");

    let resume =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_resume.json"))
            .expect("resume");
    assert!(!resume.contains("\"drove_count\": 0"));
    assert!(resume.contains("\"project_ids\": ["));
    assert!(resume.contains("\"fin\""));

    let builder_presence = fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json"))
        .expect("builder presence");
    assert!(builder_presence.contains("\"role_id\": \"project\""));
    assert!(builder_presence.contains("\"status\": \"idle\""));
    assert!(builder_presence.contains("\"current_session_id\": \"session-fin-detached\""));

    let startup_wakeup =
        fs::read_to_string(home.join("runtime/current/current_startup_wakeup.json"))
            .expect("startup wakeup");
    assert!(startup_wakeup.contains("\"project_id\": \"fin\""));
    assert!(startup_wakeup.contains("\"status\": \"completed\""));

    if let Some(value) = previous {
        unsafe {
            std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
        }
    }
    if let Some(value) = previous_bind {
        unsafe {
            std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_DAEMON_CONTROL_PLANE_BIND");
        }
    }
}

#[test]
fn stop_headless_daemon_writes_stop_request() {
    let home = temp_runtime_home("stop");
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml();
    let system = map_system_config(&user_toml).expect("system config");
    fs::create_dir_all(home.join("runtime/pids")).expect("pid dir");
    fs::write(home.join("runtime/pids/headless-daemon.pid"), b"12345").expect("pid");

    let report =
        stop_headless_daemon(&user_toml, &system, Some(home.as_path())).expect("stop report");
    assert_eq!(report.status, "stop_requested");
    assert_eq!(report.pid, Some(12345));
    assert!(home.join("runtime/locks/headless-daemon.stop").exists());
}

#[test]
fn start_daemon_rejects_when_port_already_bound() {
    use std::net::TcpListener;

    let _guard = env_lock().lock().expect("env lock");
    let previous_bind = std::env::var("FIN_DAEMON_CONTROL_PLANE_BIND").ok();

    // Bind a unique port to simulate an existing daemon
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    unsafe {
        std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", format!("127.0.0.1:{}", port));
    }

    let home = temp_runtime_home("port-mutex");
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml();
    let system = map_system_config(&user_toml).expect("system config");

    let report = crate::headless_daemon::start_headless_daemon(
        &user_toml,
        &system,
        Some(home.as_path()),
    ).expect("start report");

    assert_eq!(report.status, "already_running", "must refuse to start when port is held");
    // Restore env
    if let Some(value) = previous_bind {
        unsafe { std::env::set_var("FIN_DAEMON_CONTROL_PLANE_BIND", value); }
    } else {
        unsafe { std::env::remove_var("FIN_DAEMON_CONTROL_PLANE_BIND"); }
    }
    drop(listener);
    let _ = std::fs::remove_dir_all(&home);
}
