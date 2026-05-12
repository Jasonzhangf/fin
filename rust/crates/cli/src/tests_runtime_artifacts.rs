use super::*;
use chrono::Datelike;

#[test]
fn transcript_session_persists_recent_context_history() {
    let user_toml = sample_user_toml();
    let system = sample_system_config();
    let home = temp_runtime_home();
    let scenario = TranscriptScenario {
        session_id: Some("session-test-transcript".into()),
        task_id: Some("task-test-transcript".into()),
        turns: vec![
            TranscriptTurn {
                input: "first turn".into(),
            },
            TranscriptTurn {
                input: "second turn".into(),
            },
            TranscriptTurn {
                input: "third turn".into(),
            },
        ],
    };

    let transcript = run_transcript_session(&system, &static_provider(&system), &scenario)
        .expect("transcript session should run");
    let mut last_session_dir = None;
    for run in &transcript.runs {
        let artifacts = persist_runtime_session(&user_toml, &system, run, Some(home.as_path()))
            .expect("turn artifacts should persist");
        last_session_dir = Some(artifacts.session_dir);
    }

    let session_dir = last_session_dir.expect("session dir should exist");
    let recent_contexts: Vec<ContextSnapshotRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("context/recent_contexts.json"))
            .expect("recent contexts should exist"),
    )
    .expect("recent contexts should decode");
    assert_eq!(recent_contexts.len(), 3);
    assert!(recent_contexts[0].context.continuity_tail.is_empty());
    assert_eq!(
        recent_contexts[1].context.continuity_tail,
        vec![
            "first turn".to_string(),
            "simulated response for first turn".to_string(),
        ]
    );
    assert!(
        recent_contexts[2]
            .context
            .continuity_tail
            .contains(&"simulated response for second turn".to_string())
    );
    assert!(
        recent_contexts[2]
            .context
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("second turn")
    );
    assert_eq!(
        recent_contexts[2]
            .context
            .control
            .as_ref()
            .and_then(|value| value.session_id.as_deref()),
        Some("session-test-transcript")
    );
    let turn3_context = &recent_contexts[2].context;
    let role_prompt = turn3_context
        .role_prompt
        .as_ref()
        .expect("role prompt should exist");
    let tools = turn3_context.tools.as_ref().expect("tools should exist");

    assert_eq!(role_prompt.role_id.as_str(), "project");
    assert_eq!(role_prompt.prompt_history.len(), 2);
    assert!(
        role_prompt
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "stable_core.framework_truth_rules")
    );
    assert!(
        role_prompt
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "role.project.identity")
    );
    assert!(
        role_prompt
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "role.project.task_board_first")
    );
    assert!(
        role_prompt
            .prompt_lineage
            .iter()
            .any(|item| item.contains("stable core"))
    );
    assert!(role_prompt.output_contract.len() >= 6);
    assert!(
        role_prompt
            .output_contract
            .iter()
            .any(|item| item.contains("project scope") || item.contains("verify step"))
    );
    assert!(
        role_prompt
            .output_contract
            .iter()
            .any(|item| item.contains("model_output_contract_v1"))
    );
    assert!(
        tools
            .framework_tools
            .iter()
            .any(|tool| tool.tool_name == "provider.call")
    );
    assert!(tools.tool_selection_policy.len() >= 8);
    assert!(
        tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("project-scoped closure"))
    );
    assert!(
        tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("write_stdin"))
    );
    assert_eq!(tools.disabled_tools.len(), 3);
    assert_eq!(
        recent_contexts[2]
            .context
            .history
            .as_ref()
            .map(|value| value.recent_messages.len()),
        Some(4)
    );
    assert!(
        recent_contexts[2]
            .context
            .project
            .as_ref()
            .and_then(|value| value.focus_summary.as_deref())
            .unwrap_or_default()
            .contains("reasoning")
    );
    assert_eq!(
        recent_contexts[2]
            .context
            .project
            .as_ref()
            .and_then(|value| value.primary_project.as_ref())
            .map(|value| value.label.as_str()),
        Some("transcript-session")
    );
    assert_eq!(
        recent_contexts[2]
            .context
            .project
            .as_ref()
            .map(|value| value.active_projects.len()),
        Some(1)
    );
    assert_eq!(
        recent_contexts[2]
            .context
            .project
            .as_ref()
            .map(|value| value.projects.len()),
        Some(1)
    );
    assert_eq!(
        recent_contexts[2]
            .context
            .current_input
            .as_ref()
            .map(|value| value.input.as_str()),
        Some("third turn")
    );

    let current_context: ContextSnapshotRecord = serde_json::from_str(
        &fs::read_to_string(home.join("runtime/current/current_context.json"))
            .expect("current context should exist"),
    )
    .expect("current context should decode");
    assert_eq!(current_context.input, "third turn");
    assert_eq!(
        current_context.refs.session_id.as_deref(),
        Some("session-test-transcript")
    );
    assert_eq!(
        current_context.refs.task_id.as_deref(),
        Some("task-test-transcript")
    );
    assert_eq!(
        current_context.refs.worker_id.as_deref(),
        Some("worker-cli-session")
    );

    let recent_digests: Vec<DigestRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("digests/recent_digests.json"))
            .expect("recent digests should exist"),
    )
    .expect("recent digests should decode");
    assert_eq!(recent_digests.len(), 3);
    assert!(recent_digests[2].summary.contains("third turn"));

    let messages: Vec<SessionMessageRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("conversation/messages.json"))
            .expect("messages should exist"),
    )
    .expect("messages should decode");
    assert_eq!(messages.len(), 6);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[4].content, "third turn");
    assert!(
        messages[5]
            .content
            .contains("simulated response for third turn")
    );

    let recent_reasoning: Vec<fin_contracts::ReasoningViewRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("reasoning/recent_reasoning_views.json"))
            .expect("recent reasoning should exist"),
    )
    .expect("recent reasoning should decode");
    let recent_tools: Vec<fin_contracts::ToolExecutionRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("tools/recent_tool_records.json"))
            .expect("recent tools should exist"),
    )
    .expect("recent tools should decode");
    let recent_closures: Vec<fin_contracts::ClosureTraceRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("closures/recent_closures.json"))
            .expect("recent closures should exist"),
    )
    .expect("recent closures should decode");
    let recent_turns: Vec<fin_contracts::TurnRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("turns/recent_turns.json"))
            .expect("recent turns should exist"),
    )
    .expect("recent turns should decode");
    let recent_rounds: Vec<fin_contracts::RoundRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("rounds/recent_rounds.json"))
            .expect("recent rounds should exist"),
    )
    .expect("recent rounds should decode");
    let recent_steps: Vec<fin_contracts::StepRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("steps/recent_steps.json"))
            .expect("recent steps should exist"),
    )
    .expect("recent steps should decode");
    assert_eq!(recent_reasoning[2].operation_id, "op-test-transcript-0003");
    assert_eq!(recent_tools[2].tool_name, "provider.call");
    assert_eq!(recent_turns[2].operation_id, "op-test-transcript-0003");
    assert_eq!(recent_rounds[2].operation_id, "op-test-transcript-0003");
    assert!(
        recent_steps
            .iter()
            .any(|step| step.operation_id == "op-test-transcript-0003")
    );
    assert!(
        recent_closures[2]
            .rendered_input
            .contains("Current request:\nthird turn")
    );
}

#[test]
fn runtime_retention_limits_trim_recent_records_and_messages() {
    let user_toml = sample_user_toml();
    let mut system = sample_system_config();
    system.runtime.retention.recent_round_limit = 2;
    system.runtime.retention.session_message_limit = 4;
    let home = temp_runtime_home();
    let scenario = TranscriptScenario {
        session_id: Some("session-retention".into()),
        task_id: Some("task-retention".into()),
        turns: vec![
            TranscriptTurn {
                input: "first".into(),
            },
            TranscriptTurn {
                input: "second".into(),
            },
            TranscriptTurn {
                input: "third".into(),
            },
        ],
    };

    let transcript = run_transcript_session(&system, &static_provider(&system), &scenario)
        .expect("transcript session should run");
    let mut last_session_dir = None;
    for run in &transcript.runs {
        let artifacts = persist_runtime_session(&user_toml, &system, run, Some(home.as_path()))
            .expect("turn artifacts should persist");
        last_session_dir = Some(artifacts.session_dir);
    }

    let session_dir = last_session_dir.expect("session dir should exist");
    let recent_rounds: Vec<fin_contracts::RoundRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("rounds/recent_rounds.json"))
            .expect("recent rounds should exist"),
    )
    .expect("recent rounds should decode");
    let messages: Vec<SessionMessageRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("conversation/messages.json"))
            .expect("messages should exist"),
    )
    .expect("messages should decode");
    assert_eq!(recent_rounds.len(), 2);
    assert_eq!(recent_rounds[0].operation_id, "op-retention-0002");
    assert_eq!(recent_rounds[1].operation_id, "op-retention-0003");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].content, "second");
    assert_eq!(messages[3].content, "simulated response for third");
}

#[test]
fn event_stream_rotation_moves_old_segments_into_archive_without_losing_truth() {
    let user_toml = sample_user_toml();
    let mut system = sample_system_config();
    system.runtime.retention.session_event_hot_limit = 10;
    system
        .runtime
        .retention
        .session_event_local_archive_file_limit = 1;
    let home = temp_runtime_home();
    let scenario = TranscriptScenario {
        session_id: Some("session-event-archive".into()),
        task_id: Some("task-event-archive".into()),
        turns: vec![
            TranscriptTurn {
                input: "first".into(),
            },
            TranscriptTurn {
                input: "second".into(),
            },
            TranscriptTurn {
                input: "third".into(),
            },
        ],
    };

    let transcript = run_transcript_session(&system, &static_provider(&system), &scenario)
        .expect("transcript session should run");
    let expected_event_count = transcript
        .runs
        .iter()
        .map(|run| run.events.len())
        .sum::<usize>();
    let mut last_session_dir = None;
    for run in &transcript.runs {
        let artifacts = persist_runtime_session(&user_toml, &system, run, Some(home.as_path()))
            .expect("turn artifacts should persist");
        last_session_dir = Some(artifacts.session_dir);
    }

    let session_dir = last_session_dir.expect("session dir should exist");
    let live_stream_path = session_dir.join("events/stream.jsonl");
    let local_archive_dir = session_dir.join("events/archive");
    let now = chrono::Local::now();
    let cold_archive_dir = home.join(format!("archive/sessions/{}/{:02}/session-event-archive/events", now.year(), now.month()));
    let live_event_count = fs::read_to_string(&live_stream_path)
        .expect("live stream")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let local_archive_files = fs::read_dir(&local_archive_dir)
        .expect("local archive dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    let cold_archive_files = fs::read_dir(&cold_archive_dir)
        .expect("cold archive dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    let archived_event_count = local_archive_files
        .iter()
        .chain(cold_archive_files.iter())
        .map(|entry| {
            fs::read_to_string(entry.path())
                .expect("archive segment")
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        })
        .sum::<usize>();

    assert!(live_event_count <= 10);
    assert!(local_archive_files.len() <= 1);
    assert!(!cold_archive_files.is_empty());
    assert_eq!(
        live_event_count + archived_event_count,
        expected_event_count
    );
}
