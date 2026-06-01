use crate::managed_task_board::{ManagedTaskBoardTruth, load_managed_task_board_truth};
use fin_contracts::{EntityRefs, OwnerLoopActionRecord};
use std::path::Path;

pub fn derive_owner_loop_action_for_runtime(
    runtime_home: &Path,
    refs: &EntityRefs,
    session_id: Option<&str>,
    preferred_task_id: Option<&str>,
    created_at: &str,
) -> Result<OwnerLoopActionRecord, crate::RuntimeError> {
    let truth = load_managed_task_board_truth(runtime_home, session_id, preferred_task_id)
        .map_err(crate::RuntimeError::State)?;
    Ok(derive_owner_loop_action(refs, truth.as_ref(), created_at))
}

pub(crate) fn derive_owner_loop_action(
    refs: &EntityRefs,
    truth: Option<&ManagedTaskBoardTruth>,
    created_at: &str,
) -> OwnerLoopActionRecord {
    let (action_kind, target_task_ids, task_status_counts, active_task_id, reason) = match truth {
        Some(value) if !value.submitted_task_ids.is_empty() => (
            "review_submitted_task",
            value.submitted_task_ids.clone(),
            value.task_status_counts.clone(),
            value.active_task_id.clone(),
            value.owner_loop_summary.clone(),
        ),
        Some(value) if !value.ready_task_ids.is_empty() => (
            "dispatch_ready_task",
            value.ready_task_ids.clone(),
            value.task_status_counts.clone(),
            value.active_task_id.clone(),
            value.owner_loop_summary.clone(),
        ),
        Some(value) if !value.working_task_ids.is_empty() => (
            "wait_worker_feedback",
            value.working_task_ids.clone(),
            value.task_status_counts.clone(),
            value.active_task_id.clone(),
            value.owner_loop_summary.clone(),
        ),
        Some(value) => (
            "stay_idle_no_actionable_task",
            Vec::new(),
            value.task_status_counts.clone(),
            value.active_task_id.clone(),
            value.owner_loop_summary.clone(),
        ),
        None => (
            "no_managed_tasks",
            Vec::new(),
            Vec::new(),
            refs.task_id.clone(),
            "no managed task registry entries".into(),
        ),
    };

    OwnerLoopActionRecord {
        action_id: format!(
            "owner-loop-{}-{}",
            refs.session_id.as_deref().unwrap_or("tentative"),
            created_at
        ),
        created_at: created_at.into(),
        refs: refs.clone(),
        source: "managed_task_registry".into(),
        action_kind: action_kind.into(),
        active_task_id,
        target_task_ids,
        task_status_counts,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> EntityRefs {
        EntityRefs {
            session_id: Some("session-owner-loop".into()),
            task_id: Some("task-owner-loop".into()),
            ..EntityRefs::default()
        }
    }

    fn truth() -> ManagedTaskBoardTruth {
        ManagedTaskBoardTruth {
            active_task_id: Some("task-a".into()),
            known_task_ids: vec!["task-a".into(), "task-b".into()],
            task_status_counts: vec!["submitted=1".into(), "ready=1".into()],
            ready_task_ids: vec!["task-b".into()],
            submitted_task_ids: vec!["task-a".into()],
            working_task_ids: Vec::new(),
            task_board_summary: "board".into(),
            owner_loop_summary: "review_submitted_tasks count=1 [task-a]".into(),
        }
    }

    #[test]
    fn owner_loop_prefers_submitted_before_ready() {
        let action = derive_owner_loop_action(&refs(), Some(&truth()), "2026-04-21T10:00:00+08:00");
        assert_eq!(action.action_kind, "review_submitted_task");
        assert_eq!(action.target_task_ids, vec!["task-a".to_string()]);
    }

    #[test]
    fn owner_loop_dispatches_ready_when_no_submitted_task_exists() {
        let mut snapshot = truth();
        snapshot.submitted_task_ids.clear();
        snapshot.owner_loop_summary = "dispatch_ready_tasks count=1 [task-b]".into();
        let action =
            derive_owner_loop_action(&refs(), Some(&snapshot), "2026-04-21T10:00:00+08:00");
        assert_eq!(action.action_kind, "dispatch_ready_task");
        assert_eq!(action.target_task_ids, vec!["task-b".to_string()]);
    }

    #[test]
    fn owner_loop_stays_idle_when_no_actionable_managed_task_exists() {
        let mut snapshot = truth();
        snapshot.submitted_task_ids.clear();
        snapshot.ready_task_ids.clear();
        snapshot.working_task_ids.clear();
        snapshot.task_status_counts = vec!["done=2".into()];
        snapshot.owner_loop_summary = "no_actionable_managed_tasks".into();
        let action =
            derive_owner_loop_action(&refs(), Some(&snapshot), "2026-04-21T10:00:00+08:00");
        assert_eq!(action.action_kind, "stay_idle_no_actionable_task");
        assert!(action.target_task_ids.is_empty());
    }

    #[test]
    fn owner_loop_waits_for_worker_when_working_not_empty() {
        let mut snapshot = truth();
        snapshot.submitted_task_ids.clear();
        snapshot.ready_task_ids.clear();
        snapshot.working_task_ids = vec!["task-c".into()];
        snapshot.owner_loop_summary = "wait_worker_feedback count=1 [task-c]".into();
        let action = derive_owner_loop_action(&refs(), Some(&snapshot), "2026-06-01T00:00:00Z");
        assert_eq!(action.action_kind, "wait_worker_feedback");
    }

    #[test]
    fn owner_loop_no_managed_when_truth_is_none() {
        let action = derive_owner_loop_action(&refs(), None, "2026-06-01T00:00:00Z");
        assert_eq!(action.action_kind, "no_managed_tasks");
    }
}
