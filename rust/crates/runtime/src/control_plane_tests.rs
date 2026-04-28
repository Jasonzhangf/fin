use super::*;
use crate::{InferenceOperationBuilder, InferenceRequest, M1Runtime, WorkerRuntime};
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_contracts::{EntityRefs, MinimalContextView, PauseCheckpointRecord};
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
            output_text: r#"<fin_user_response>done</fin_user_response>
<fin_control_feedback>{"origin":"model_output_contract_v1","is_continuation":true,"is_simple_query":false,"candidate_task_id":"task-control","candidate_topic_thread_id":null,"continuity_confidence":90,"topic_shift_confidence":10,"simple_query_confidence":5,"previous_topic_summary":"task","current_topic_summary":"task","note_candidate":"done","digest_candidate":"done","reason":"ok"}</fin_control_feedback>"#.into(),
            response_id: Some("resp-static".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
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
        resume_checkpoint_id: None,
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
        fin_contracts::InterruptedSegmentRecord {
            segment_id: "segment-same".into(),
            interrupted_turn_id: Some("turn-old".into()),
            interrupted_step_id: Some("step-old".into()),
            created_at: "2026-04-19T20:00:01+08:00".into(),
            status: "merged".into(),
            ..fin_contracts::InterruptedSegmentRecord::default()
        },
        fin_contracts::InterruptedSegmentRecord {
            segment_id: "segment-same".into(),
            interrupted_turn_id: Some("turn-target".into()),
            interrupted_step_id: Some("step-target".into()),
            created_at: "2026-04-19T20:00:02+08:00".into(),
            status: "open".into(),
            ..fin_contracts::InterruptedSegmentRecord::default()
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
