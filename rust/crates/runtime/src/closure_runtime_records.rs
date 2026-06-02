use super::*;

pub(super) struct ClosureRecordsBundle {
    pub(super) progress: ProgressBlock,
    pub(super) note: ExecutionNote,
    pub(super) reasoning_view: ReasoningViewRecord,
    pub(super) digest: DigestRecord,
    pub(super) routing_decision: RoutingDecisionRecord,
    pub(super) routing_action: RoutingActionRecord,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_closure_records(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    prepared_request: &PreparedRequest,
    assistant_response_text: &str,
    dispatched_tools: &tool_dispatch::ToolDispatchOutcome,
    closure_waiting_external: bool,
    closure_finished: bool,
    control_exit_channel: Option<&'static str>,
    control_feedback: &ControlFeedback,
    tool_records: &[ToolExecutionRecord],
    digest_id: &str,
    closure_id: &str,
) -> ClosureRecordsBundle {
    let progress = ProgressBlock {
        progress_id: format!("progress-{}", operation.operation_id),
        refs: refs.clone(),
        phase: "inference_completed".into(),
        blocker: None,
        next_step: Some(next_step(dispatched_tools.reminder_scheduled, closure_finished).into()),
        health_hint: Some("healthy".into()),
        tool_snapshots: tool_records
            .iter()
            .map(|record| ToolSnapshot {
                tool_name: record.tool_name.clone(),
                status: record.status.clone(),
                summary: record
                    .output_summary
                    .clone()
                    .or_else(|| record.input_summary.clone())
                    .unwrap_or_else(|| record.purpose.clone()),
            })
            .collect(),
    };
    let note = ExecutionNote {
        note_id: format!("note-{}", operation.operation_id),
        refs: refs.clone(),
        summary: if dispatched_tools.note_hints.is_empty() {
            format!(
                "provider {} returned: {}",
                prepared_request.provider_name, assistant_response_text
            )
        } else {
            format!(
                "provider {} returned: {}; {}",
                prepared_request.provider_name,
                assistant_response_text,
                dispatched_tools.note_hints.join(" | ")
            )
        },
        decision: Some(if dispatched_tools.reminder_scheduled {
            "wait_for_system_self_reminder".into()
        } else if !closure_finished {
            "continue_reasoning".into()
        } else if control_feedback.is_continuation {
            "continue_current_task".into()
        } else {
            "observe_topic_continuity".into()
        }),
        lesson: None,
        blocker: None,
        next_step: Some(next_step(dispatched_tools.reminder_scheduled, closure_finished).into()),
        control_feedback: Some(control_feedback.clone()),
        created_at: operation.submitted_at.clone(),
    };
    let reasoning_view = trace_records::reasoning_view_record(
        &operation.operation_id,
        &operation.trace_id,
        &refs,
        &operation.submitted_at,
        &note,
        &control_feedback,
        &tool_records,
    );
    let digest = DigestRecord {
        digest_id: digest_id.to_string(),
        closure_id: closure_id.to_string(),
        refs: refs.clone(),
        summary: format!(
            "closure {} with model {} and answer {}",
            if closure_waiting_external {
                "waiting_external"
            } else if let Some(channel) = control_exit_channel {
                channel
            } else {
                "continued_without_valid_exit"
            },
            prepared_request.model,
            assistant_response_text
        ),
        continuity_tail: vec![
            operation.payload.input.clone(),
            assistant_response_text.to_string(),
        ],
        note_refs: vec![note.note_id.clone()],
        artifact_candidates: vec![format!(
            "provider:{}:{}:{}",
            prepared_request.provider_name, prepared_request.model, assistant_response_text
        )],
        control_feedback: Some(control_feedback.clone()),
        created_at: operation.submitted_at.clone(),
    };
    let routing_decision = turn_records::routing_decision_record(
        &operation.operation_id,
        &operation.trace_id,
        &refs,
        &operation.submitted_at,
        &control_feedback,
    );
    let routing_action = routing_actions::derive_routing_action(&routing_decision);

    ClosureRecordsBundle {
        progress,
        note,
        reasoning_view,
        digest,
        routing_decision,
        routing_action,
    }
}
