use crate::ClosureRun;
use fin_contracts::{
    ClosureTraceRecord, ControlFeedback, EntityRefs, ExecutionNote, ReasoningViewRecord,
    ToolExecutionRecord,
};
use fin_provider::{PreparedRequest, ProviderResponse};

pub(crate) fn provider_tool_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    prepared_request: &PreparedRequest,
    provider_response: &ProviderResponse,
    assistant_response_text: &str,
    occurred_at: &str,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        tool_call_id: format!("tool-provider-call-{operation_id}"),
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        refs: refs.clone(),
        tool_name: "provider.call".into(),
        tool_kind: "framework_tool".into(),
        title: "Provider Call".into(),
        purpose: "dispatch compiled prompt to provider and normalize the response".into(),
        target_kind: Some("provider".into()),
        target_ref: Some(format!(
            "{}.{} @ {}",
            prepared_request.provider_name, prepared_request.model, prepared_request.endpoint
        )),
        input_summary: Some(short_text(&prepared_request.input, 240)),
        output_summary: Some(short_text(assistant_response_text, 240)),
        status: if provider_response.status >= 400 {
            "failed".into()
        } else {
            "completed".into()
        },
        started_at: occurred_at.into(),
        ended_at: Some(occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["network_request".into(), "provider_response".into()],
        artifact_refs: vec![
            "events/stream.jsonl".into(),
            "progress/latest.json".into(),
            "notes/latest.json".into(),
        ],
        error_summary: (provider_response.status >= 400)
            .then(|| short_text(&provider_response.output_text, 240)),
    }
}

pub(crate) fn reasoning_view_record(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    created_at: &str,
    note: &ExecutionNote,
    control_feedback: &ControlFeedback,
    tool_records: &[ToolExecutionRecord],
) -> ReasoningViewRecord {
    let tool_summary = tool_records.first().map(|record| {
        format!(
            "{} {} -> {}",
            record.tool_name,
            record.status,
            record.target_ref.as_deref().unwrap_or("unknown target")
        )
    });
    ReasoningViewRecord {
        reasoning_id: format!("reasoning-{operation_id}"),
        operation_id: operation_id.into(),
        trace_id: trace_id.into(),
        refs: refs.clone(),
        created_at: created_at.into(),
        summary: if !control_feedback.note_candidate.trim().is_empty() {
            control_feedback.note_candidate.clone()
        } else {
            note.summary.clone()
        },
        decision_summary: note.decision.clone(),
        continuity_summary: Some(format!(
            "continuation={} continuity={} shift={} simple={} reason={}",
            control_feedback.is_continuation,
            control_feedback.continuity_confidence,
            control_feedback.topic_shift_confidence,
            control_feedback.simple_query_confidence,
            control_feedback.reason
        )),
        tool_intent_summary: tool_summary,
        risk_summary: note.blocker.clone(),
        next_step: note.next_step.clone(),
        source_refs: vec![
            "control/latest.json".into(),
            "notes/latest.json".into(),
            "digests/latest.json".into(),
            "tools/recent_tool_records.json".into(),
        ],
    }
}

pub(crate) fn closure_trace_record(run: &ClosureRun) -> ClosureTraceRecord {
    ClosureTraceRecord {
        closure_id: run.digest.closure_id.clone(),
        digest_id: run.digest.digest_id.clone(),
        operation_id: run.operation.operation_id.clone(),
        trace_id: run.operation.trace_id.clone(),
        refs: run.operation.refs.clone(),
        created_at: run.note.created_at.clone(),
        user_input: run.context_snapshot.input.clone(),
        assistant_response: run.assistant_response_text.clone(),
        rendered_input: run.prepared_request.rendered_input.clone(),
        provider_name: run.prepared_request.provider_name.clone(),
        provider_model: run.prepared_request.model.clone(),
        provider_endpoint: run.prepared_request.endpoint.clone(),
        provider_raw_output: run.provider_response.output_text.clone(),
        context_snapshot_path: "context/recent_contexts.json".into(),
        control_feedback_path: "control/latest.json".into(),
        reasoning_view_path: "reasoning/latest.json".into(),
        tool_record_paths: vec!["tools/recent_tool_records.json".into()],
        source_event_ids: run
            .events
            .iter()
            .map(|event| event.event_id.clone())
            .collect(),
    }
}

fn short_text(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.into()
    } else {
        let mut out = trimmed.chars().take(limit).collect::<String>();
        out.push('…');
        out
    }
}
