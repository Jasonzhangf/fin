use fin_contracts::{
    EntityRefs, ExecutionStateRecord, OwnerLoopActionRecord, PendingInputRecord,
    RoutingActionRecord, SchedulerDecisionRecord,
};

pub fn derive_scheduler_decision(
    refs: &EntityRefs,
    state: Option<&ExecutionStateRecord>,
    pending_inputs: &[PendingInputRecord],
    routing_action: Option<&RoutingActionRecord>,
    owner_loop_action: Option<&OwnerLoopActionRecord>,
    created_at: &str,
) -> SchedulerDecisionRecord {
    let state_status = state
        .map(|value| value.status.clone())
        .unwrap_or_else(|| "unavailable".into());
    let latest_routing_action_kind = routing_action.map(|value| value.action_kind.clone());
    let pending_input_count = pending_inputs.len();
    let parallel_pending_count = pending_inputs
        .iter()
        .filter(|value| is_parallel_pending(value))
        .count();

    let (action_kind, continue_until_blocked, blocked_by, reason) = match state_status.as_str() {
        "paused" => (
            if parallel_pending_count > 0 {
                "run_next_parallel"
            } else {
                "wait_paused"
            },
            parallel_pending_count > 0,
            if parallel_pending_count > 0 {
                None
            } else {
                Some("paused".into())
            },
            if parallel_pending_count > 0 {
                format!("paused with {parallel_pending_count} parallel user inputs ready")
            } else {
                "execution is paused".into()
            },
        ),
        "running" => (
            "wait_running",
            false,
            Some("running".into()),
            "closure is still running".into(),
        ),
        "waiting_external" => {
            if parallel_pending_count > 0 {
                (
                    "run_next_parallel",
                    true,
                    None,
                    format!(
                        "waiting_external with {parallel_pending_count} parallel user inputs ready"
                    ),
                )
            } else {
                (
                    "wait_external",
                    false,
                    Some("waiting_external".into()),
                    "waiting external reminder or upstream result".into(),
                )
            }
        }
        "idle" => {
            if routing_action.is_some_and(|value| value.prompt_user) {
                (
                    "await_user_confirmation",
                    false,
                    Some("routing_prompt_user".into()),
                    "latest routing action requires explicit user confirmation".into(),
                )
            } else if parallel_pending_count > 0 {
                (
                    "run_next_parallel",
                    true,
                    None,
                    format!("idle with {parallel_pending_count} parallel user inputs"),
                )
            } else if state.is_some_and(|value| value.resume_checkpoint_ready) {
                (
                    "resume_checkpoint",
                    true,
                    None,
                    "idle with open resumable execution checkpoint".into(),
                )
            } else if pending_input_count > 0 {
                (
                    "run_next_pending",
                    true,
                    None,
                    format!("idle with {pending_input_count} pending inputs"),
                )
            } else if owner_loop_action.is_some_and(is_actionable_owner_loop) {
                let action = owner_loop_action.expect("owner loop action checked");
                (
                    action.action_kind.as_str(),
                    false,
                    Some(action.action_kind.clone()),
                    action.reason.clone(),
                )
            } else {
                (
                    "stay_idle",
                    false,
                    None,
                    "idle and no pending inputs".into(),
                )
            }
        }
        _ => (
            "observe_only",
            false,
            Some("state_unavailable".into()),
            "execution state unavailable".into(),
        ),
    };

    SchedulerDecisionRecord {
        decision_id: format!(
            "scheduler-{}-{}",
            refs.session_id.as_deref().unwrap_or("tentative"),
            created_at
        ),
        created_at: created_at.into(),
        refs: refs.clone(),
        state_status,
        pending_input_count,
        latest_routing_action_kind,
        action_kind: action_kind.into(),
        continue_until_blocked,
        blocked_by,
        reason,
    }
}

fn is_actionable_owner_loop(action: &OwnerLoopActionRecord) -> bool {
    matches!(
        action.action_kind.as_str(),
        "review_submitted_task" | "dispatch_ready_task" | "wait_worker_feedback"
    )
}

fn is_parallel_pending(input: &PendingInputRecord) -> bool {
    matches!(
        input.input_kind.as_str(),
        "parallel_chat" | "parallel_channel_ingress"
    ) || matches!(
        input.source.as_str(),
        "cli.parallel_user" | "channel.parallel_user"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> EntityRefs {
        EntityRefs {
            session_id: Some("session-scheduler".into()),
            task_id: Some("task-scheduler".into()),
            ..EntityRefs::default()
        }
    }

    #[test]
    fn scheduler_runs_next_pending_when_idle_and_no_prompt_is_required() {
        let decision = derive_scheduler_decision(
            &refs(),
            Some(&ExecutionStateRecord {
                state_id: "exec-1".into(),
                refs: refs(),
                status: "idle".into(),
                active_turn_id: None,
                active_step_id: None,
                resume_from_step_id: None,
                resume_checkpoint_ready: false,
                resume_checkpoint_id: None,
                pending_input_count: 2,
                accepts_user_input: true,
                reason: None,
                updated_at: "2026-04-19T22:00:00+08:00".into(),
            }),
            &vec![
                PendingInputRecord {
                    pending_input_id: "pending-1".into(),
                    refs: refs(),
                    input_kind: "chat".into(),
                    source: "cli.user".into(),
                    message: "queued".into(),
                    attachments: Vec::new(),
                    status: "pending".into(),
                    enqueue_reason: "idle".into(),
                    enqueued_at: "2026-04-19T22:00:00+08:00".into(),
                },
                PendingInputRecord {
                    pending_input_id: "pending-2".into(),
                    refs: refs(),
                    input_kind: "chat".into(),
                    source: "cli.user".into(),
                    message: "queued-2".into(),
                    attachments: Vec::new(),
                    status: "pending".into(),
                    enqueue_reason: "idle".into(),
                    enqueued_at: "2026-04-19T22:00:00+08:00".into(),
                },
            ],
            Some(&RoutingActionRecord {
                action_id: "routing-action-1".into(),
                decision_id: "routing-1".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                refs: refs(),
                created_at: "2026-04-19T22:00:00+08:00".into(),
                action_kind: "continue_current_task".into(),
                source_disposition: "continue_current_task".into(),
                apply_immediately: true,
                prompt_user: false,
                prompt_text: None,
                suggested_task_id: None,
                suggested_topic_thread_id: None,
                confidence: 92,
                reason: "same task".into(),
            }),
            None,
            "2026-04-19T22:00:00+08:00",
        );
        assert_eq!(decision.action_kind, "run_next_pending");
        assert!(decision.continue_until_blocked);
    }

    #[test]
    fn scheduler_blocks_when_routing_action_requires_user_confirmation() {
        let decision = derive_scheduler_decision(
            &refs(),
            Some(&ExecutionStateRecord {
                state_id: "exec-2".into(),
                refs: refs(),
                status: "idle".into(),
                active_turn_id: None,
                active_step_id: None,
                resume_from_step_id: None,
                resume_checkpoint_ready: false,
                resume_checkpoint_id: None,
                pending_input_count: 1,
                accepts_user_input: true,
                reason: None,
                updated_at: "2026-04-19T22:00:01+08:00".into(),
            }),
            &vec![PendingInputRecord {
                pending_input_id: "pending-1".into(),
                refs: refs(),
                input_kind: "chat".into(),
                source: "cli.user".into(),
                message: "queued".into(),
                attachments: Vec::new(),
                status: "pending".into(),
                enqueue_reason: "idle".into(),
                enqueued_at: "2026-04-19T22:00:01+08:00".into(),
            }],
            Some(&RoutingActionRecord {
                action_id: "routing-action-2".into(),
                decision_id: "routing-2".into(),
                operation_id: "op-2".into(),
                trace_id: "trace-2".into(),
                refs: refs(),
                created_at: "2026-04-19T22:00:01+08:00".into(),
                action_kind: "ask_topic_switch".into(),
                source_disposition: "candidate_topic_switch".into(),
                apply_immediately: false,
                prompt_user: true,
                prompt_text: Some("switch?".into()),
                suggested_task_id: None,
                suggested_topic_thread_id: Some("topic-2".into()),
                confidence: 81,
                reason: "topic changed".into(),
            }),
            None,
            "2026-04-19T22:00:01+08:00",
        );
        assert_eq!(decision.action_kind, "await_user_confirmation");
        assert_eq!(decision.blocked_by.as_deref(), Some("routing_prompt_user"));
    }

    #[test]
    fn scheduler_prefers_resume_checkpoint_before_pending_queue() {
        let decision = derive_scheduler_decision(
            &refs(),
            Some(&ExecutionStateRecord {
                state_id: "exec-3".into(),
                refs: refs(),
                status: "idle".into(),
                active_turn_id: Some("turn-op-1".into()),
                active_step_id: Some("step-op-1-04-tool_dispatch".into()),
                resume_from_step_id: Some("step-op-1-04-tool_dispatch".into()),
                resume_checkpoint_ready: true,
                resume_checkpoint_id: Some("checkpoint-op-1-r02".into()),
                pending_input_count: 2,
                accepts_user_input: true,
                reason: Some("checkpoint ready".into()),
                updated_at: "2026-04-20T10:00:00+08:00".into(),
            }),
            &vec![
                PendingInputRecord {
                    pending_input_id: "pending-1".into(),
                    refs: refs(),
                    input_kind: "chat".into(),
                    source: "cli.user".into(),
                    message: "queued".into(),
                    attachments: Vec::new(),
                    status: "pending".into(),
                    enqueue_reason: "checkpoint".into(),
                    enqueued_at: "2026-04-20T10:00:00+08:00".into(),
                },
                PendingInputRecord {
                    pending_input_id: "pending-2".into(),
                    refs: refs(),
                    input_kind: "chat".into(),
                    source: "cli.user".into(),
                    message: "queued-2".into(),
                    attachments: Vec::new(),
                    status: "pending".into(),
                    enqueue_reason: "checkpoint".into(),
                    enqueued_at: "2026-04-20T10:00:00+08:00".into(),
                },
            ],
            None,
            None,
            "2026-04-20T10:00:00+08:00",
        );
        assert_eq!(decision.action_kind, "resume_checkpoint");
        assert!(decision.continue_until_blocked);
    }

    #[test]
    fn scheduler_runs_parallel_pending_before_wait_external_block() {
        let decision = derive_scheduler_decision(
            &refs(),
            Some(&ExecutionStateRecord {
                state_id: "exec-4".into(),
                refs: refs(),
                status: "waiting_external".into(),
                active_turn_id: Some("turn-op-2".into()),
                active_step_id: Some("step-op-2-05-finalize".into()),
                resume_from_step_id: Some("step-op-2-05-finalize".into()),
                resume_checkpoint_ready: true,
                resume_checkpoint_id: Some("checkpoint-op-2-r02".into()),
                pending_input_count: 1,
                accepts_user_input: true,
                reason: Some("waiting".into()),
                updated_at: "2026-04-21T10:00:00+08:00".into(),
            }),
            &vec![PendingInputRecord {
                pending_input_id: "pending-1".into(),
                refs: refs(),
                input_kind: "parallel_chat".into(),
                source: "cli.parallel_user".into(),
                message: "parallel".into(),
                attachments: Vec::new(),
                status: "pending".into(),
                enqueue_reason: "waiting_external".into(),
                enqueued_at: "2026-04-21T10:00:00+08:00".into(),
            }],
            None,
            None,
            "2026-04-21T10:00:00+08:00",
        );
        assert_eq!(decision.action_kind, "run_next_parallel");
        assert!(decision.continue_until_blocked);
    }

    #[test]
    fn scheduler_surfaces_owner_loop_action_when_idle_without_pending_inputs() {
        let decision = derive_scheduler_decision(
            &refs(),
            Some(&ExecutionStateRecord {
                state_id: "exec-5".into(),
                refs: refs(),
                status: "idle".into(),
                active_turn_id: None,
                active_step_id: None,
                resume_from_step_id: None,
                resume_checkpoint_ready: false,
                resume_checkpoint_id: None,
                pending_input_count: 0,
                accepts_user_input: true,
                reason: None,
                updated_at: "2026-04-21T10:05:00+08:00".into(),
            }),
            &[],
            None,
            Some(&OwnerLoopActionRecord {
                action_id: "owner-loop-1".into(),
                created_at: "2026-04-21T10:05:00+08:00".into(),
                refs: refs(),
                source: "managed_task_registry".into(),
                action_kind: "review_submitted_task".into(),
                active_task_id: Some("task-review".into()),
                target_task_ids: vec!["task-review".into()],
                task_status_counts: vec!["submitted=1".into()],
                reason: "review_submitted_tasks count=1 [task-review]".into(),
            }),
            "2026-04-21T10:05:00+08:00",
        );
        assert_eq!(decision.action_kind, "review_submitted_task");
        assert_eq!(
            decision.blocked_by.as_deref(),
            Some("review_submitted_task")
        );
    }
}
