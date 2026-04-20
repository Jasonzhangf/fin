use fin_contracts::{
    EntityRefs, ExecutionStateRecord, RoutingActionRecord, SchedulerDecisionRecord,
};

pub fn derive_scheduler_decision(
    refs: &EntityRefs,
    state: Option<&ExecutionStateRecord>,
    pending_input_count: usize,
    routing_action: Option<&RoutingActionRecord>,
    created_at: &str,
) -> SchedulerDecisionRecord {
    let state_status = state
        .map(|value| value.status.clone())
        .unwrap_or_else(|| "unavailable".into());
    let latest_routing_action_kind = routing_action.map(|value| value.action_kind.clone());

    let (action_kind, continue_until_blocked, blocked_by, reason) = match state_status.as_str() {
        "paused" => (
            "wait_paused",
            false,
            Some("paused".into()),
            "execution is paused".into(),
        ),
        "running" => (
            "wait_running",
            false,
            Some("running".into()),
            "closure is still running".into(),
        ),
        "waiting_external" => (
            "wait_external",
            false,
            Some("waiting_external".into()),
            "waiting external reminder or upstream result".into(),
        ),
        "idle" => {
            if routing_action.is_some_and(|value| value.prompt_user) {
                (
                    "await_user_confirmation",
                    false,
                    Some("routing_prompt_user".into()),
                    "latest routing action requires explicit user confirmation".into(),
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
            2,
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
            1,
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
            2,
            None,
            "2026-04-20T10:00:00+08:00",
        );
        assert_eq!(decision.action_kind, "resume_checkpoint");
        assert!(decision.continue_until_blocked);
    }
}
