use super::*;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-agent-control-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability_ids: vec!["exec".into(), "mailbox".into()],
        tool_allowlist: vec!["exec_command".into(), "mailbox.send".into()],
    }
}

fn register_system(store: &AgentControlStore) -> AgentIdentity {
    store
        .register_primary_agent(RegisterPrimaryAgentInput {
            agent_id: "system-main".into(),
            kind: AgentKind::SystemAgent,
            project_id: None,
            device_binding: "macbook".into(),
            auth_subject: "user:jason".into(),
            auth_lease_id: "lease-system".into(),
            capability_descriptor: descriptor(),
            now: "2026-05-23T10:00:00+08:00".into(),
        })
        .expect("register system")
}

fn register_project(store: &AgentControlStore) -> AgentIdentity {
    store
        .register_primary_agent(RegisterPrimaryAgentInput {
            agent_id: "project-fin".into(),
            kind: AgentKind::ProjectAgent,
            project_id: Some("fin".into()),
            device_binding: "macbook".into(),
            auth_subject: "user:jason/project:fin".into(),
            auth_lease_id: "lease-project".into(),
            capability_descriptor: descriptor(),
            now: "2026-05-23T10:00:00+08:00".into(),
        })
        .expect("register project")
}

#[test]
fn registers_system_and_project_as_primary_identities() {
    let runtime_home = temp_runtime_home("primary-registration");
    let store = AgentControlStore::new(&runtime_home);

    let system = register_system(&store);
    let project = register_project(&store);

    assert_eq!(system.kind, AgentKind::SystemAgent);
    assert_eq!(project.kind, AgentKind::ProjectAgent);
    assert_eq!(system.parent_agent_id, None);
    assert_eq!(project.parent_agent_id, None);
    assert_eq!(system.path, "system:system-main");
    assert_eq!(project.path, "project:fin:project-fin");

    let registry = fs::read_to_string(runtime_home.join("runtime/agents/control/identities.json"))
        .expect("identity registry");
    assert!(registry.contains("system_agent"));
    assert!(registry.contains("project_agent"));
    assert!(!registry.contains("parent_agent_id\": \"system-main"));
}

#[test]
fn system_and_project_spawn_subagents_with_parent_paths() {
    let runtime_home = temp_runtime_home("spawn-paths");
    let store = AgentControlStore::new(&runtime_home);
    register_system(&store);
    register_project(&store);

    let (_, system_child) = store
        .spawn_subagent(SpawnSubagentInput {
            parent_agent_id: "system-main".into(),
            parent_run_id: Some("system-run-1".into()),
            agent_run_id: "sys-child-1".into(),
            task_id: Some("task-global".into()),
            assignment_id: None,
            context_policy: ContextPolicy::default(),
            now: "2026-05-23T10:01:00+08:00".into(),
        })
        .expect("system spawn");
    let (_, project_child) = store
        .spawn_subagent(SpawnSubagentInput {
            parent_agent_id: "project-fin".into(),
            parent_run_id: Some("project-run-1".into()),
            agent_run_id: "proj-child-1".into(),
            task_id: Some("task-fin".into()),
            assignment_id: Some("assign-1".into()),
            context_policy: ContextPolicy {
                mode: ContextMode::LastNTurns,
                last_n_turns: Some(3),
                reason: None,
            },
            now: "2026-05-23T10:01:00+08:00".into(),
        })
        .expect("project spawn");

    assert_eq!(system_child.path, "system:system-main/subagent:sys-child-1");
    assert_eq!(
        project_child.path,
        "project:fin:project-fin/subagent:proj-child-1"
    );
    assert_eq!(project_child.status, "running");
    assert_eq!(project_child.assignment_id.as_deref(), Some("assign-1"));
}

#[test]
fn context_policy_requires_reason_for_full_session_context() {
    let runtime_home = temp_runtime_home("context-policy");
    let store = AgentControlStore::new(&runtime_home);
    register_project(&store);

    let error = store
        .spawn_subagent(SpawnSubagentInput {
            parent_agent_id: "project-fin".into(),
            parent_run_id: None,
            agent_run_id: "bad-full-context".into(),
            task_id: None,
            assignment_id: None,
            context_policy: ContextPolicy {
                mode: ContextMode::FullSessionContext,
                last_n_turns: None,
                reason: None,
            },
            now: "2026-05-23T10:02:00+08:00".into(),
        })
        .expect_err("full session context without reason must fail");
    assert!(error.contains("full_session_context requires reason"));
}

#[test]
fn mailbox_seq_is_monotonic_and_trigger_turn_is_persisted() {
    let runtime_home = temp_runtime_home("mailbox-seq");
    let store = AgentControlStore::new(&runtime_home);
    register_system(&store);
    register_project(&store);

    let first = store
        .send_agent_input(SendAgentInput {
            message_id: "msg-1".into(),
            from_agent_id: "system-main".into(),
            to_agent_id: "project-fin".into(),
            thread_id: Some("thread-1".into()),
            task_id: Some("task-fin".into()),
            trigger_turn: true,
            payload: json!({ "text": "please execute task" }),
        })
        .expect("send first");
    let second = store
        .send_agent_input(SendAgentInput {
            message_id: "msg-2".into(),
            from_agent_id: "project-fin".into(),
            to_agent_id: "project-fin".into(),
            thread_id: Some("thread-1".into()),
            task_id: Some("task-fin".into()),
            trigger_turn: false,
            payload: json!({ "text": "local note" }),
        })
        .expect("send second");

    assert_eq!(first.seq, 1);
    assert_eq!(second.seq, 2);
    assert!(first.trigger_turn);

    let inbox = store.read_mailbox("project-fin").expect("read mailbox");
    assert_eq!(inbox.len(), 2);
    assert_eq!(inbox[0].seq, 1);
    assert_eq!(inbox[1].seq, 2);
}

#[test]
fn wait_close_and_resume_follow_primary_vs_subagent_lifecycle() {
    let runtime_home = temp_runtime_home("lifecycle");
    let store = AgentControlStore::new(&runtime_home);
    register_project(&store);
    let (child_identity, child_run) = store
        .spawn_subagent(SpawnSubagentInput {
            parent_agent_id: "project-fin".into(),
            parent_run_id: Some("project-run-1".into()),
            agent_run_id: "proj-child-1".into(),
            task_id: Some("task-fin".into()),
            assignment_id: None,
            context_policy: ContextPolicy::default(),
            now: "2026-05-23T10:03:00+08:00".into(),
        })
        .expect("spawn child");

    let initial_wait = store
        .wait_agent(&child_run.agent_run_id)
        .expect("wait running");
    assert_eq!(initial_wait.status, "timeout");

    store
        .update_run_status(
            &child_run.agent_run_id,
            "completed",
            vec!["runtime/results/proj-child-1.json".into()],
            "2026-05-23T10:04:00+08:00",
        )
        .expect("complete child");
    let completed_wait = store
        .wait_agent(&child_run.agent_run_id)
        .expect("wait completed");
    assert_eq!(completed_wait.status, "completed");
    assert_eq!(
        completed_wait.result_refs,
        vec!["runtime/results/proj-child-1.json"]
    );

    let close_primary = store
        .close_agent("project-fin", "2026-05-23T10:05:00+08:00")
        .expect("close primary");
    assert_eq!(close_primary.action, "release_primary_lease");
    assert_eq!(close_primary.status, "detached");
    assert!(close_primary.affected_run_ids.is_empty());

    let resumed_primary = store
        .resume_agent(
            "project-fin",
            Some("project-resume-1"),
            "2026-05-23T10:06:00+08:00",
        )
        .expect("resume primary");
    assert_eq!(resumed_primary.status, "running");
    assert_eq!(resumed_primary.path, "project:fin:project-fin");

    let close_child = store
        .close_agent(&child_identity.agent_id, "2026-05-23T10:07:00+08:00")
        .expect("close child");
    assert_eq!(close_child.action, "close_subagent_subtree");
    assert_eq!(close_child.affected_run_ids, vec!["proj-child-1"]);

    let resumed_child = store
        .resume_agent(
            &child_identity.agent_id,
            Some("proj-child-1"),
            "2026-05-23T10:08:00+08:00",
        )
        .expect("resume child");
    assert_eq!(resumed_child.status, "running");
    assert_eq!(
        resumed_child.path,
        "project:fin:project-fin/subagent:proj-child-1"
    );
}
