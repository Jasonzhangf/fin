use fin_contracts::{
    ControlFeedback, EntityRefs, ExecutionNote, ProgressBlock, ProviderRequestRecord,
    ProviderResponseRecord, RoutingDecisionRecord, StepRecord, ToolExecutionRecord, TurnRecord,
};
use fin_provider::{PreparedRequest, ProviderResponse};

pub(super) fn provider_request_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    turn_id: &str,
    step_id: &str,
    round_index: u32,
    attempt_index: u32,
    request: &PreparedRequest,
    created_at: &str,
) -> ProviderRequestRecord {
    ProviderRequestRecord {
        request_id: format!(
            "provider-request-{operation_id}-r{round_index:02}-a{attempt_index:02}"
        ),
        turn_id: turn_id.into(),
        step_id: step_id.into(),
        round_index,
        attempt_index,
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        refs: refs.clone(),
        provider_name: request.provider_name.clone(),
        model: request.model.clone(),
        endpoint: request.endpoint.clone(),
        input: request.input.clone(),
        rendered_input: request.rendered_input.clone(),
        user_agent: request.user_agent.clone(),
        sanitized_headers: request.sanitized_headers.clone(),
        created_at: created_at.into(),
    }
}

pub(super) fn provider_response_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    turn_id: &str,
    step_id: &str,
    round_index: u32,
    attempt_index: u32,
    request_id: &str,
    response: &ProviderResponse,
    created_at: &str,
) -> ProviderResponseRecord {
    ProviderResponseRecord {
        response_record_id: format!(
            "provider-response-{operation_id}-r{round_index:02}-a{attempt_index:02}"
        ),
        request_id: request_id.into(),
        turn_id: turn_id.into(),
        step_id: step_id.into(),
        round_index,
        attempt_index,
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        refs: refs.clone(),
        status: response.status,
        response_id: response.response_id.clone(),
        stop_reason: response.stop_reason.clone(),
        output_text: response.output_text.clone(),
        created_at: created_at.into(),
    }
}

pub(super) fn step_record(
    step_id: String,
    turn_id: &str,
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    step_index: u32,
    step_kind: &str,
    status: &str,
    started_at: &str,
    summary: String,
    input_ref: Option<String>,
    output_ref: Option<String>,
    next_step_hint: Option<String>,
) -> StepRecord {
    StepRecord {
        step_id,
        turn_id: turn_id.into(),
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        step_index,
        step_kind: step_kind.into(),
        status: status.into(),
        started_at: started_at.into(),
        ended_at: Some(started_at.into()),
        refs: refs.clone(),
        summary,
        input_ref,
        output_ref,
        event_ids: Vec::new(),
        progress_ref: None,
        note_refs: Vec::new(),
        blocked_by_step_id: None,
        next_step_hint,
    }
}

pub(super) fn finalize_step_record(
    mut step: StepRecord,
    progress: &ProgressBlock,
    note: &ExecutionNote,
) -> StepRecord {
    step.progress_ref = Some("progress/latest.json".into());
    step.note_refs = vec![
        note.note_id.clone(),
        format!("notes/latest.json#note_id={}", note.note_id),
    ];
    step.next_step_hint = progress.next_step.clone();
    step
}

pub(super) fn turn_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    turn_id: &str,
    closure_id: &str,
    created_at: &str,
    user_input: &str,
    assistant_visible_output: &str,
    progress: &ProgressBlock,
    note: &ExecutionNote,
    tool_records: &[ToolExecutionRecord],
    provider_request_records: &[ProviderRequestRecord],
    provider_response_records: &[ProviderResponseRecord],
    step_records: &[StepRecord],
    status: &str,
) -> TurnRecord {
    TurnRecord {
        turn_id: turn_id.into(),
        closure_id: closure_id.into(),
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        turn_index: parse_turn_index(operation_id),
        status: status.into(),
        created_at: created_at.into(),
        completed_at: Some(created_at.into()),
        refs: refs.clone(),
        user_input: user_input.into(),
        assistant_visible_output: Some(assistant_visible_output.into()),
        progress_summary: progress
            .next_step
            .as_ref()
            .map(|next| format!("phase={} next_step={next}", progress.phase)),
        control_feedback_ref: Some(format!("control/latest.json#operation_id={operation_id}")),
        execution_note_refs: vec![
            note.note_id.clone(),
            format!("notes/latest.json#note_id={}", note.note_id),
        ],
        context_snapshot_ref: Some(format!(
            "context/recent_contexts.json#operation_id={operation_id}"
        )),
        reasoning_view_refs: vec![format!(
            "reasoning/recent_reasoning_views.json#operation_id={operation_id}"
        )],
        tool_record_refs: tool_records
            .iter()
            .map(|record| {
                format!(
                    "tools/recent_tool_records.json#tool_call_id={}",
                    record.tool_call_id
                )
            })
            .collect(),
        provider_request_ref: provider_request_records.last().map(|record| {
            format!(
                "provider/recent_provider_requests.json#request_id={}",
                record.request_id
            )
        }),
        provider_response_ref: provider_response_records.last().map(|record| {
            format!(
                "provider/recent_provider_responses.json#response_record_id={}",
                record.response_record_id
            )
        }),
        closure_trace_ref: Some(format!("closures/latest.json#closure_id={closure_id}")),
        digest_ref: Some(format!("digests/latest.json#closure_id={closure_id}")),
        step_ids: step_records
            .iter()
            .map(|step| step.step_id.clone())
            .collect(),
    }
}

pub(super) fn routing_decision_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    created_at: &str,
    feedback: &ControlFeedback,
) -> RoutingDecisionRecord {
    let has_bound_task = refs.task_id.is_some();
    let (disposition, requires_user_confirmation) = if has_bound_task {
        if feedback.topic_shift_confidence >= 60
            && feedback.continuity_confidence < feedback.topic_shift_confidence
        {
            ("candidate_topic_switch".to_string(), true)
        } else {
            ("continue_current_task".to_string(), false)
        }
    } else if feedback.is_simple_query && feedback.simple_query_confidence >= 70 {
        ("tentative_simple_chat".to_string(), false)
    } else if feedback.candidate_task_id.is_some() {
        ("candidate_existing_task".to_string(), true)
    } else if feedback.continuity_confidence < 70 {
        ("candidate_new_task".to_string(), true)
    } else {
        ("pending_observation".to_string(), false)
    };

    RoutingDecisionRecord {
        decision_id: format!("routing-{operation_id}"),
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        refs: refs.clone(),
        created_at: created_at.into(),
        disposition,
        requires_user_confirmation,
        candidate_task_id: feedback.candidate_task_id.clone(),
        candidate_topic_thread_id: feedback.candidate_topic_thread_id.clone(),
        continuity_confidence: feedback.continuity_confidence,
        topic_shift_confidence: feedback.topic_shift_confidence,
        simple_query_confidence: feedback.simple_query_confidence,
        previous_topic_summary: feedback.previous_topic_summary.clone(),
        current_topic_summary: feedback.current_topic_summary.clone(),
        reason: feedback.reason.clone(),
    }
}

fn parse_turn_index(operation_id: &str) -> Option<u64> {
    operation_id
        .rsplit_once('-')
        .and_then(|(_, tail)| tail.parse::<u64>().ok())
}
