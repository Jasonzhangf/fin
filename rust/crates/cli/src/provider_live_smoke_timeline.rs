use super::{
    ProviderLiveSmokeConformance, ProviderLiveSmokeTurnTimeline, live_smoke_scenario_violations,
};
use crate::transcript::{TranscriptScenario, TranscriptTurn};
use fin_runtime::ClosureRun;
use std::collections::BTreeSet;

pub(super) fn build_turn_timelines(
    transcript: &super::provider_live_smoke_report::LiveTranscriptRun,
    session_step_records: &[serde_json::Value],
    session_round_records: &[serde_json::Value],
    session_tool_records: &[serde_json::Value],
    session_provider_request_records: &[serde_json::Value],
) -> Vec<ProviderLiveSmokeTurnTimeline> {
    transcript
        .runs
        .iter()
        .enumerate()
        .map(|(index, run)| {
            build_turn_timeline(
                index,
                run,
                session_step_records,
                session_round_records,
                session_tool_records,
                session_provider_request_records,
            )
        })
        .collect()
}

pub(super) fn build_conformance(
    transcript: &super::provider_live_smoke_report::LiveTranscriptRun,
    session_messages: &[serde_json::Value],
    turn_timelines: &[ProviderLiveSmokeTurnTimeline],
    session_provider_response_records: &[serde_json::Value],
) -> ProviderLiveSmokeConformance {
    let user_turn_issues =
        live_smoke_scenario_violations(&transcript_scenario_from_run(transcript));
    let visible_message_roles = session_messages
        .iter()
        .filter_map(|message| {
            message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    let visible_message_roles_ok = visible_message_roles
        .iter()
        .all(|role| role == "user" || role == "assistant");
    let timeline_issues = turn_timelines
        .iter()
        .filter(|timeline| !timeline.required_steps_present)
        .map(|timeline| {
            format!(
                "turn {} missing required timeline steps: {}",
                timeline.turn_index,
                timeline.step_timeline.join(" -> ")
            )
        })
        .collect::<Vec<_>>();
    let provider_response_gaps = turn_timelines
        .iter()
        .filter(|timeline| {
            !session_provider_response_records.iter().any(|record| {
                record
                    .get("operation_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(timeline.operation_id.as_str())
            })
        })
        .map(|timeline| {
            format!(
                "turn {} missing provider response truth",
                timeline.turn_index
            )
        })
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    issues.extend(user_turn_issues.clone());
    if !visible_message_roles_ok {
        issues.push(format!(
            "visible conversation contains non user/assistant roles [{}]",
            visible_message_roles
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    issues.extend(timeline_issues);
    issues.extend(provider_response_gaps);
    let timeline_steps_ok = turn_timelines
        .iter()
        .all(|timeline| timeline.required_steps_present);
    let user_turn_purity_ok = user_turn_issues.is_empty();
    let model_gap_signals = turn_timelines
        .iter()
        .filter_map(|timeline| {
            if !timeline.assistant_response_present {
                Some(format!(
                    "turn {} closed without visible assistant answer",
                    timeline.turn_index
                ))
            } else if timeline.round_count > 1
                && !timeline.reasoning_stop_completed
                && timeline.closure_stop_source.as_deref() == Some("not_emitted")
            {
                Some(format!(
                    "turn {} required {} rounds and still closed without reasoning.stop",
                    timeline.turn_index, timeline.round_count
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    issues.extend(model_gap_signals.iter().cloned());
    let attribution_hint =
        if !user_turn_purity_ok || !visible_message_roles_ok || !timeline_steps_ok {
            "likely_framework_gap"
        } else if !model_gap_signals.is_empty() {
            "likely_model_or_prompt_gap"
        } else if issues.is_empty() {
            "framework_conformance_ok"
        } else {
            "insufficient_truth"
        };
    ProviderLiveSmokeConformance {
        policy: "human_like_goal_only".into(),
        user_turn_purity_ok,
        visible_message_roles_ok,
        timeline_steps_ok,
        issues,
        attribution_hint: attribution_hint.into(),
    }
}

fn build_turn_timeline(
    index: usize,
    run: &ClosureRun,
    session_step_records: &[serde_json::Value],
    session_round_records: &[serde_json::Value],
    session_tool_records: &[serde_json::Value],
    session_provider_request_records: &[serde_json::Value],
) -> ProviderLiveSmokeTurnTimeline {
    let operation_id = run.operation.operation_id.as_str();
    let step_records = session_step_records
        .iter()
        .filter(|record| {
            record
                .get("operation_id")
                .and_then(serde_json::Value::as_str)
                == Some(operation_id)
        })
        .collect::<Vec<_>>();
    let step_timeline = step_records
        .iter()
        .map(|record| {
            format!(
                "{}:{}",
                classify_step_kind(record),
                record
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
            )
        })
        .collect::<Vec<_>>();
    let model_tool_names = session_tool_records
        .iter()
        .filter(|record| {
            record
                .get("operation_id")
                .and_then(serde_json::Value::as_str)
                == Some(operation_id)
                && record.get("tool_name").and_then(serde_json::Value::as_str)
                    != Some("provider.call")
        })
        .filter_map(|record| {
            record
                .get("tool_name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let reasoning_stop_completed = session_tool_records.iter().any(|record| {
        record
            .get("operation_id")
            .and_then(serde_json::Value::as_str)
            == Some(operation_id)
            && record.get("tool_name").and_then(serde_json::Value::as_str) == Some("reasoning.stop")
            && record.get("status").and_then(serde_json::Value::as_str) == Some("completed")
    });
    let required_steps_present =
        required_step_kinds_present(&step_records, !model_tool_names.is_empty());
    ProviderLiveSmokeTurnTimeline {
        turn_index: index + 1,
        operation_id: run.operation.operation_id.clone(),
        provider_attempts: session_provider_request_records
            .iter()
            .filter(|record| {
                record
                    .get("operation_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(operation_id)
            })
            .count(),
        round_count: session_round_records
            .iter()
            .filter(|record| {
                record
                    .get("operation_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(operation_id)
            })
            .count(),
        assistant_response_present: !run.assistant_response_text.trim().is_empty(),
        model_tool_names,
        reasoning_stop_completed,
        closure_stop_source: step_records.iter().find_map(|record| {
            if classify_step_kind(record) == "finalize" {
                parse_stop_source(
                    record
                        .get("summary")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                )
            } else {
                None
            }
        }),
        required_steps_present,
        step_timeline,
    }
}

fn transcript_scenario_from_run(
    transcript: &super::provider_live_smoke_report::LiveTranscriptRun,
) -> TranscriptScenario {
    TranscriptScenario {
        session_id: Some(transcript.session_id.clone()),
        task_id: Some(transcript.task_id.clone()),
        turns: transcript
            .runs
            .iter()
            .map(|run| TranscriptTurn {
                input: run.context_snapshot.input.clone(),
            })
            .collect(),
    }
}

fn classify_step_kind(record: &serde_json::Value) -> &'static str {
    if let Some(step_kind) = record.get("step_kind").and_then(serde_json::Value::as_str) {
        return match step_kind {
            "context_build" => "context_build",
            "provider_request" => "provider_request",
            "model_parse" => "model_parse",
            "control_feedback" => "control_feedback",
            "tool_dispatch" => "tool_dispatch",
            "finalize" => "finalize",
            _ => "other",
        };
    }
    match record
        .get("step_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|step_id| step_id.rsplit_once('_').map(|(_, suffix)| suffix))
    {
        Some("context_build") => "context_build",
        Some("provider_request") => "provider_request",
        Some("model_parse") => "model_parse",
        Some("control_feedback") => "control_feedback",
        Some("tool_dispatch") => "tool_dispatch",
        Some("finalize") => "finalize",
        _ => "other",
    }
}

fn required_step_kinds_present(
    step_records: &[&serde_json::Value],
    require_tool_dispatch: bool,
) -> bool {
    let step_kinds = step_records
        .iter()
        .map(|record| classify_step_kind(record))
        .collect::<BTreeSet<_>>();
    let required = if require_tool_dispatch {
        [
            "context_build",
            "provider_request",
            "model_parse",
            "control_feedback",
            "tool_dispatch",
            "finalize",
        ]
        .as_slice()
    } else {
        [
            "context_build",
            "provider_request",
            "model_parse",
            "control_feedback",
            "finalize",
        ]
        .as_slice()
    };
    required.iter().all(|kind| step_kinds.contains(kind))
}

fn parse_stop_source(summary: &str) -> Option<String> {
    let marker = "stop_source=";
    let start = summary.find(marker)?;
    let rest = &summary[start + marker.len()..];
    let end = rest.find(' ').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}
