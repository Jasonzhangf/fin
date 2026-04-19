use fin_contracts::{RoutingActionRecord, RoutingDecisionRecord};

pub(super) fn derive_routing_action(decision: &RoutingDecisionRecord) -> RoutingActionRecord {
    let (action_kind, apply_immediately, prompt_user, prompt_text, confidence) = match decision
        .disposition
        .as_str()
    {
        "continue_current_task" => (
            "continue_current_task",
            true,
            false,
            None,
            decision.continuity_confidence,
        ),
        "tentative_simple_chat" => (
            "stay_tentative_session",
            true,
            false,
            None,
            decision.simple_query_confidence,
        ),
        "candidate_existing_task" => (
            "ask_reuse_existing_task",
            false,
            true,
            Some(match decision.candidate_task_id.as_deref() {
                Some(task_id) => format!("框架判断你可能想切换到已有任务 {task_id}，是否切换？"),
                None => "框架判断你可能想切换到已有任务，是否切换？".into(),
            }),
            decision
                .continuity_confidence
                .max(100 - decision.topic_shift_confidence),
        ),
        "candidate_topic_switch" => (
            "ask_topic_switch",
            false,
            true,
            Some("框架判断当前话题可能已经变化，是否切换到新话题？".into()),
            decision.topic_shift_confidence,
        ),
        _ => ("observe_only", false, false, None, 0),
    };

    RoutingActionRecord {
        action_id: format!("routing-action-{}", decision.operation_id),
        decision_id: decision.decision_id.clone(),
        operation_id: decision.operation_id.clone(),
        trace_id: decision.trace_id.clone(),
        refs: decision.refs.clone(),
        created_at: decision.created_at.clone(),
        action_kind: action_kind.into(),
        source_disposition: decision.disposition.clone(),
        apply_immediately,
        prompt_user,
        prompt_text,
        suggested_task_id: decision.candidate_task_id.clone(),
        suggested_topic_thread_id: decision.candidate_topic_thread_id.clone(),
        confidence,
        reason: decision.reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_contracts::EntityRefs;

    #[test]
    fn routing_action_asks_for_topic_switch_when_shift_confidence_is_high() {
        let action = derive_routing_action(&RoutingDecisionRecord {
            decision_id: "routing-op-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            refs: EntityRefs::default(),
            created_at: "2026-04-19T21:30:00+08:00".into(),
            disposition: "candidate_topic_switch".into(),
            requires_user_confirmation: true,
            candidate_task_id: None,
            candidate_topic_thread_id: Some("topic-new".into()),
            continuity_confidence: 22,
            topic_shift_confidence: 81,
            simple_query_confidence: 10,
            previous_topic_summary: Some("old".into()),
            current_topic_summary: Some("new".into()),
            reason: "topic changed".into(),
        });
        assert_eq!(action.action_kind, "ask_topic_switch");
        assert!(action.prompt_user);
        assert!(!action.apply_immediately);
        assert_eq!(action.confidence, 81);
    }
}
