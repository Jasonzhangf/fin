use crate::ClosureRun;
use fin_contracts::{
    EntityRefs, ExecutionStateRecord, InputAttachmentSummary, InterruptedSegmentRecord,
    PauseCheckpointRecord, PendingInputRecord, SegmentMergeRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInputDequeue {
    pub dequeued: PendingInputRecord,
    pub remaining: Vec<PendingInputRecord>,
}

pub fn running_state(
    refs: &EntityRefs,
    operation_id: &str,
    submitted_at: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("exec-state-{operation_id}"),
        refs: refs.clone(),
        status: "running".into(),
        active_turn_id: Some(format!("turn-{operation_id}")),
        active_step_id: Some(format!("step-{operation_id}-01-context_build")),
        resume_from_step_id: Some(format!("step-{operation_id}-01-context_build")),
        pending_input_count,
        accepts_user_input: false,
        reason: Some("active closure running".into()),
        updated_at: submitted_at.into(),
    }
}

pub fn state_after_run(run: &ClosureRun, pending_input_count: usize) -> ExecutionStateRecord {
    let last_step_id = run.step_records.last().map(|step| step.step_id.clone());
    let is_waiting = run
        .tool_records
        .iter()
        .any(|record| record.tool_name == "wait.remind" && record.status == "completed");
    ExecutionStateRecord {
        state_id: format!("exec-state-{}", run.operation.operation_id),
        refs: run.operation.refs.clone(),
        status: if is_waiting {
            "waiting_external".into()
        } else {
            "idle".into()
        },
        active_turn_id: Some(run.turn_record.turn_id.clone()),
        active_step_id: last_step_id.clone(),
        resume_from_step_id: last_step_id,
        pending_input_count,
        accepts_user_input: true,
        reason: Some(if is_waiting {
            "waiting for reminder or external result".into()
        } else {
            "closure completed".into()
        }),
        updated_at: run.note.created_at.clone(),
    }
}

pub fn failed_state(
    refs: &EntityRefs,
    operation_id: &str,
    now: &str,
    reason: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("exec-state-{operation_id}-failed"),
        refs: refs.clone(),
        status: "idle".into(),
        active_turn_id: Some(format!("turn-{operation_id}")),
        active_step_id: None,
        resume_from_step_id: None,
        pending_input_count,
        accepts_user_input: true,
        reason: Some(reason.into()),
        updated_at: now.into(),
    }
}

pub fn paused_state(
    refs: &EntityRefs,
    session_id: Option<&str>,
    current_state: Option<&ExecutionStateRecord>,
    now: &str,
    reason: Option<String>,
    pending_input_count: usize,
) -> (ExecutionStateRecord, PauseCheckpointRecord) {
    let (turn_id, step_id) = current_state
        .map(|state| (state.active_turn_id.clone(), state.active_step_id.clone()))
        .unwrap_or((None, None));
    let checkpoint = PauseCheckpointRecord {
        checkpoint_id: format!("pause-{}", session_id.unwrap_or("tentative")),
        refs: refs.clone(),
        turn_id: turn_id.clone(),
        active_step_id: step_id.clone(),
        resume_from_step_id: step_id.clone(),
        reason: reason.clone(),
        paused_at: now.into(),
    };
    let state = ExecutionStateRecord {
        state_id: checkpoint.checkpoint_id.clone(),
        refs: refs.clone(),
        status: "paused".into(),
        active_turn_id: turn_id,
        active_step_id: step_id.clone(),
        resume_from_step_id: step_id,
        pending_input_count,
        accepts_user_input: false,
        reason,
        updated_at: now.into(),
    };
    (state, checkpoint)
}

pub fn resumed_state(
    refs: &EntityRefs,
    session_id: Option<&str>,
    checkpoint: Option<&PauseCheckpointRecord>,
    now: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("resume-{}", session_id.unwrap_or("tentative")),
        refs: refs.clone(),
        status: "idle".into(),
        active_turn_id: checkpoint.and_then(|value| value.turn_id.clone()),
        active_step_id: checkpoint.and_then(|value| value.active_step_id.clone()),
        resume_from_step_id: checkpoint.and_then(|value| value.resume_from_step_id.clone()),
        pending_input_count,
        accepts_user_input: true,
        reason: Some("manual resume".into()),
        updated_at: now.into(),
    }
}

pub fn new_pending_input(
    refs: &EntityRefs,
    session_id: Option<&str>,
    next_index: usize,
    input_kind: &str,
    source: &str,
    message: &str,
    attachments: &[InputAttachmentSummary],
    enqueue_reason: &str,
    now: &str,
) -> PendingInputRecord {
    PendingInputRecord {
        pending_input_id: format!(
            "pending-{}-{next_index:02}",
            session_id.unwrap_or("tentative")
        ),
        refs: refs.clone(),
        input_kind: input_kind.into(),
        source: source.into(),
        message: message.trim().to_string(),
        attachments: attachments.to_vec(),
        status: "pending".into(),
        enqueue_reason: enqueue_reason.into(),
        enqueued_at: now.into(),
    }
}

pub fn dequeue_pending_input(pending: &[PendingInputRecord]) -> Option<PendingInputDequeue> {
    let (first, rest) = pending.split_first()?;
    let mut dequeued = first.clone();
    dequeued.status = "dequeued".into();
    Some(PendingInputDequeue {
        dequeued,
        remaining: rest.to_vec(),
    })
}

pub fn state_with_pending_count(
    state: &ExecutionStateRecord,
    pending_input_count: usize,
    now: &str,
) -> ExecutionStateRecord {
    let mut updated = state.clone();
    updated.pending_input_count = pending_input_count;
    updated.updated_at = now.into();
    updated
}

pub fn clear_waiting_state_if_due(
    state: &ExecutionStateRecord,
    now: &str,
) -> Option<ExecutionStateRecord> {
    if state.status != "waiting_external" {
        return None;
    }
    let mut updated = state.clone();
    updated.status = "idle".into();
    updated.accepts_user_input = true;
    updated.reason = Some("external reminder fired".into());
    updated.updated_at = now.into();
    Some(updated)
}

pub fn interrupted_segment(
    refs: &EntityRefs,
    session_id: Option<&str>,
    checkpoint: &PauseCheckpointRecord,
) -> InterruptedSegmentRecord {
    InterruptedSegmentRecord {
        segment_id: build_segment_id(session_id, checkpoint),
        refs: refs.clone(),
        interrupted_turn_id: checkpoint.turn_id.clone(),
        interrupted_step_id: checkpoint.active_step_id.clone(),
        resume_from_step_id: checkpoint.resume_from_step_id.clone(),
        status: "open".into(),
        reason: checkpoint.reason.clone(),
        created_at: checkpoint.paused_at.clone(),
        merged_into_turn_id: None,
        merged_into_operation_id: None,
        merged_at: None,
    }
}

pub fn segment_merge(segment: &InterruptedSegmentRecord, run: &ClosureRun) -> SegmentMergeRecord {
    SegmentMergeRecord {
        merge_id: format!("merge-{}", run.operation.operation_id),
        segment_id: segment.segment_id.clone(),
        refs: run.operation.refs.clone(),
        interrupted_turn_id: segment.interrupted_turn_id.clone(),
        resumed_turn_id: run.turn_record.turn_id.clone(),
        resumed_operation_id: run.operation.operation_id.clone(),
        strategy: "resume_as_new_closure".into(),
        created_at: run.note.created_at.clone(),
    }
}

pub fn apply_segment_merge(
    segments: &mut [InterruptedSegmentRecord],
    segment: &InterruptedSegmentRecord,
    resumed_turn_id: &str,
    resumed_operation_id: &str,
    merged_at: &str,
) -> Option<InterruptedSegmentRecord> {
    let target_index = segments.iter().rposition(|item| {
        item.segment_id == segment.segment_id
            && item.created_at == segment.created_at
            && item.interrupted_turn_id == segment.interrupted_turn_id
            && item.interrupted_step_id == segment.interrupted_step_id
            && item.status == "open"
    })?;
    let item = &mut segments[target_index];
    item.status = "merged".into();
    item.merged_into_turn_id = Some(resumed_turn_id.into());
    item.merged_into_operation_id = Some(resumed_operation_id.into());
    item.merged_at = Some(merged_at.into());
    Some(item.clone())
}

fn build_segment_id(session_id: Option<&str>, checkpoint: &PauseCheckpointRecord) -> String {
    let session = sanitize_id_fragment(session_id.unwrap_or("tentative"));
    let turn = sanitize_id_fragment(checkpoint.turn_id.as_deref().unwrap_or("turn"));
    let paused_at = sanitize_id_fragment(&checkpoint.paused_at);
    format!("segment-{session}-{turn}-{paused_at}")
}

fn sanitize_id_fragment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    sanitized
        .trim_matches('-')
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InferenceOperationBuilder, InferenceRequest, M1Runtime, WorkerRuntime};
    use fin_config::{
        ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
        UserProviderConfig,
    };
    use fin_contracts::MinimalContextView;
    use fin_provider::{
        InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
    };
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    struct StaticProvider {
        descriptor: ProviderDescriptor,
    }

    impl StaticProvider {
        fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
                    name: "openai".into(),
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    credential: ProviderCredential::ApiKeyEnv {
                        env_var: "OPENAI_API_KEY".into(),
                    },
                    user_agent: None,
                    headers: BTreeMap::new(),
                }),
            }
        }
    }

    impl InferenceProvider for StaticProvider {
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
                output_text: "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-control\",\"candidate_topic_thread_id\":null,\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"task\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"ok\"}</fin_control_feedback>".into(),
                response_id: Some("resp-static".into()),
                stop_reason: Some("end_turn".into()),
                status: 200,
            })
        }
    }

    fn refs() -> EntityRefs {
        EntityRefs {
            session_id: Some("session-control".into()),
            task_id: Some("task-control".into()),
            ..EntityRefs::default()
        }
    }

    fn worker_runtime() -> WorkerRuntime {
        let user = UserConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    user_agent: None,
                    headers: BTreeMap::new(),
                },
            )]),
            runtime: fin_config::UserRuntimeConfig::default(),
        };
        let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
        WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
            .expect("worker runtime")
    }

    fn build_run(operation_id: &str) -> ClosureRun {
        let worker = worker_runtime();
        let operation = InferenceOperationBuilder
            .build(
                &worker,
                InferenceRequest {
                    operation_id: operation_id.into(),
                    trace_id: format!("trace-{operation_id}"),
                    submitted_at: "2026-04-19T21:00:00+08:00".into(),
                    refs: refs(),
                    input: "continue".into(),
                    context: MinimalContextView {
                        control: Some(fin_contracts::ContextControlBlock {
                            task_id: Some("task-control".into()),
                            ..Default::default()
                        }),
                        summary: Some("summary".into()),
                        ..Default::default()
                    },
                },
            )
            .expect("operation");
        let mut runtime = M1Runtime::default();
        runtime
            .run_closure(operation, &StaticProvider::new())
            .expect("closure")
    }

    #[test]
    fn paused_state_uses_current_state_turn_and_step() {
        let current = running_state(&refs(), "op-1", "2026-04-19T21:00:00+08:00", 2);
        let (state, checkpoint) = paused_state(
            &refs(),
            Some("session-control"),
            Some(&current),
            "2026-04-19T21:01:00+08:00",
            Some("manual pause".into()),
            2,
        );
        assert_eq!(state.status, "paused");
        assert_eq!(checkpoint.turn_id, current.active_turn_id);
        assert_eq!(checkpoint.active_step_id, current.active_step_id);
    }

    #[test]
    fn dequeue_pending_input_marks_first_item_dequeued() {
        let pending = vec![
            new_pending_input(
                &refs(),
                Some("session-control"),
                1,
                "chat",
                "cli.user",
                "first",
                &[],
                "queued",
                "2026-04-19T21:00:00+08:00",
            ),
            new_pending_input(
                &refs(),
                Some("session-control"),
                2,
                "chat",
                "cli.user",
                "second",
                &[],
                "queued",
                "2026-04-19T21:00:01+08:00",
            ),
        ];
        let dequeue = dequeue_pending_input(&pending).expect("dequeue");
        assert_eq!(dequeue.dequeued.message, "first");
        assert_eq!(dequeue.dequeued.status, "dequeued");
        assert_eq!(dequeue.remaining.len(), 1);
        assert_eq!(dequeue.remaining[0].message, "second");
    }

    #[test]
    fn interrupted_segment_id_is_unique_by_pause_timestamp() {
        let first = PauseCheckpointRecord {
            checkpoint_id: "pause-1".into(),
            refs: refs(),
            turn_id: Some("turn-op-1".into()),
            active_step_id: Some("step-op-1-05-tool_dispatch".into()),
            resume_from_step_id: Some("step-op-1-05-tool_dispatch".into()),
            reason: Some("manual pause".into()),
            paused_at: "2026-04-19T20:00:01+08:00".into(),
        };
        let second = PauseCheckpointRecord {
            paused_at: "2026-04-19T20:00:02+08:00".into(),
            ..first.clone()
        };
        let first_segment = interrupted_segment(&refs(), Some("session-control"), &first);
        let second_segment = interrupted_segment(&refs(), Some("session-control"), &second);
        assert_ne!(first_segment.segment_id, second_segment.segment_id);
    }

    #[test]
    fn apply_segment_merge_updates_only_exact_open_match() {
        let mut segments = vec![
            InterruptedSegmentRecord {
                segment_id: "segment-same".into(),
                interrupted_turn_id: Some("turn-old".into()),
                interrupted_step_id: Some("step-old".into()),
                created_at: "2026-04-19T20:00:01+08:00".into(),
                status: "merged".into(),
                ..InterruptedSegmentRecord::default()
            },
            InterruptedSegmentRecord {
                segment_id: "segment-same".into(),
                interrupted_turn_id: Some("turn-target".into()),
                interrupted_step_id: Some("step-target".into()),
                created_at: "2026-04-19T20:00:02+08:00".into(),
                status: "open".into(),
                ..InterruptedSegmentRecord::default()
            },
        ];
        let target = segments[1].clone();
        let updated = apply_segment_merge(
            &mut segments,
            &target,
            "turn-resumed",
            "op-resumed",
            "2026-04-19T20:05:00+08:00",
        )
        .expect("updated");
        assert_eq!(updated.status, "merged");
        assert_eq!(segments[0].interrupted_turn_id, Some("turn-old".into()));
        assert_eq!(
            segments[1].merged_into_operation_id.as_deref(),
            Some("op-resumed")
        );
    }

    #[test]
    fn state_after_run_marks_waiting_external_when_wait_tool_completed() {
        let run = build_run("op-control");
        let state = state_after_run(&run, 0);
        assert_eq!(state.status, "idle");
        assert!(state.accepts_user_input);
    }
}
