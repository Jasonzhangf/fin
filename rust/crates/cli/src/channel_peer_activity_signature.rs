use fin_contracts::{SourceActivityCardView, UserActivityCardView};

pub(crate) fn signature_user_card(card: &UserActivityCardView) -> serde_json::Value {
    serde_json::json!({
        "owner_source_id": card.owner_source_id,
        "header": card.header,
        "state": card.state,
        "focus_source_id": card.focus_source_id,
        "focus_summary": card.focus_summary,
        "stage": card.stage,
        "recent_items": card.recent_items,
        "active_sources": card.active_sources,
        "waiting_detail": card.waiting_detail,
        "failure_detail": card.failure_detail,
    })
}

pub(crate) fn signature_source_card(card: &SourceActivityCardView) -> serde_json::Value {
    serde_json::json!({
        "source_id": card.source_id,
        "source_kind": card.source_kind,
        "title": card.title,
        "visibility": card.visibility,
        "state": card.state,
        "summary": card.summary,
        "focus_label": card.focus_label,
        "auto_promoted": card.auto_promoted,
        "current_activity": card.current_activity,
        "recent_actions": card.recent_actions,
        "waiting_detail": card.waiting_detail,
        "failure_detail": card.failure_detail,
        "session_id": card.session_id,
        "task_id": card.task_id,
    })
}
