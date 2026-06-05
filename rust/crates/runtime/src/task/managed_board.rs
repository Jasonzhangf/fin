use crate::task::store::{StoredTaskRecord, list_registered_tasks};
use std::{cmp::Reverse, collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ManagedTaskBoardTruth {
    pub(crate) active_task_id: Option<String>,
    pub(crate) known_task_ids: Vec<String>,
    pub(crate) task_status_counts: Vec<String>,
    pub(crate) ready_task_ids: Vec<String>,
    pub(crate) submitted_task_ids: Vec<String>,
    pub(crate) working_task_ids: Vec<String>,
    pub(crate) task_board_summary: String,
    pub(crate) owner_loop_summary: String,
}

pub(crate) fn load_managed_task_board_truth(
    runtime_home: &Path,
    session_id: Option<&str>,
    preferred_task_id: Option<&str>,
) -> Result<Option<ManagedTaskBoardTruth>, String> {
    let all_tasks = list_registered_tasks(runtime_home)?;
    if all_tasks.is_empty() {
        return Ok(None);
    }
    let mut tasks = all_tasks
        .into_iter()
        .filter(|(_, (summary, task))| {
            preferred_task_id.is_some_and(|task_id| task.task_id == task_id)
                || session_id.is_some_and(|current_session| summary.session_id == current_session)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if tasks.is_empty() {
        tasks = list_registered_tasks(runtime_home)?;
    }

    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut ready_task_ids = Vec::new();
    let mut submitted_task_ids = Vec::new();
    let mut working_task_ids = Vec::new();
    let mut known_task_ids = tasks.keys().cloned().collect::<Vec<_>>();
    known_task_ids.sort_by_key(|task_id| Reverse(task_id.clone()));
    known_task_ids.truncate(8);

    for (task_id, (_, task)) in &tasks {
        *status_counts.entry(task.status.clone()).or_insert(0) += 1;
        if is_ready_unclaimed(task) {
            ready_task_ids.push(task_id.clone());
        }
        if task.status == "submitted" {
            submitted_task_ids.push(task_id.clone());
        }
        if matches!(task.status.as_str(), "claimed" | "working" | "reviewing") {
            working_task_ids.push(task_id.clone());
        }
    }

    ready_task_ids.sort();
    submitted_task_ids.sort();
    working_task_ids.sort();

    let active_task_id = preferred_task_id
        .and_then(|task_id| tasks.get(task_id).map(|_| task_id.to_string()))
        .or_else(|| {
            session_id.and_then(|current_session| {
                tasks
                    .iter()
                    .filter(|(_, (summary, _))| summary.session_id == current_session)
                    .map(|(task_id, _)| task_id.clone())
                    .max()
            })
        });
    let task_status_counts = status_counts
        .iter()
        .map(|(status, count)| format!("{status}={count}"))
        .collect::<Vec<_>>();
    let owner_loop_summary = if !submitted_task_ids.is_empty() {
        format!(
            "review_submitted_tasks count={} [{}]",
            submitted_task_ids.len(),
            submitted_task_ids.join(", ")
        )
    } else if !ready_task_ids.is_empty() {
        format!(
            "dispatch_ready_tasks count={} [{}]",
            ready_task_ids.len(),
            ready_task_ids.join(", ")
        )
    } else if !working_task_ids.is_empty() {
        format!(
            "wait_for_worker_feedback count={} [{}]",
            working_task_ids.len(),
            working_task_ids.join(", ")
        )
    } else {
        "no_actionable_managed_tasks".into()
    };
    let task_board_summary = format!(
        "managed_tasks={} · statuses={} · ready={} · submitted={}{}",
        tasks.len(),
        if task_status_counts.is_empty() {
            "none".into()
        } else {
            task_status_counts.join(",")
        },
        ready_task_ids.len(),
        submitted_task_ids.len(),
        active_task_id
            .as_deref()
            .map(|task_id| format!(" · active_task={task_id}"))
            .unwrap_or_default()
    );

    Ok(Some(ManagedTaskBoardTruth {
        active_task_id,
        known_task_ids,
        task_status_counts,
        ready_task_ids,
        submitted_task_ids,
        working_task_ids,
        task_board_summary,
        owner_loop_summary,
    }))
}

fn is_ready_unclaimed(task: &StoredTaskRecord) -> bool {
    matches!(task.status.as_str(), "created" | "ready") && task.claimed_by_worker_id.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::store::create_task_record;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-managed-task-board-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
        path
    }

    fn make_session(home: &Path, session_id: &str) {
        let session_dir = home.join("sessions/2026/04").join(session_id);
        fs::create_dir_all(session_dir.join("tasks/registry")).expect("session registry dir");
        fs::create_dir_all(session_dir.join("tasks/board")).expect("session board dir");
    }

    fn task(task_id: &str, session_id: &str, status: &str) -> StoredTaskRecord {
        StoredTaskRecord {
            task_id: task_id.into(),
            session_id: session_id.into(),
            title: task_id.into(),
            summary: format!("{task_id} summary"),
            epic_id: None,
            status: status.into(),
            creator_worker_id: None,
            creator_role_id: None,
            review_owner_worker_id: None,
            claimed_by_worker_id: None,
            submitted_by_worker_id: None,
            reviewed_by_worker_id: None,
            latest_submission_summary: None,
            latest_review_decision: None,
            latest_review_summary: None,
            artifact_refs: Vec::new(),
            created_at: "2026-04-21T12:00:00+08:00".into(),
            updated_at: "2026-04-21T12:00:00+08:00".into(),
        }
    }

    #[test]
    fn managed_task_board_scopes_to_current_session_when_available() {
        let home = temp_runtime_home("session-scope");
        make_session(&home, "session-a");
        make_session(&home, "session-b");
        create_task_record(
            &home,
            "session-a",
            task("task-a-ready", "session-a", "ready"),
        )
        .expect("task a");
        create_task_record(
            &home,
            "session-b",
            task("task-b-submitted", "session-b", "submitted"),
        )
        .expect("task b");

        let truth = load_managed_task_board_truth(&home, Some("session-a"), None)
            .expect("truth")
            .expect("managed truth");

        assert_eq!(truth.ready_task_ids, vec!["task-a-ready".to_string()]);
        assert!(truth.submitted_task_ids.is_empty());
        assert_eq!(truth.task_status_counts, vec!["ready=1".to_string()]);
        assert_eq!(
            truth.owner_loop_summary,
            "dispatch_ready_tasks count=1 [task-a-ready]"
        );
    }
}
