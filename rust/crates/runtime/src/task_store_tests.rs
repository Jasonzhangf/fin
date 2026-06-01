use super::*;

#[test]
fn stored_task_record_serde_roundtrip() {
    let task = StoredTaskRecord {
        task_id: "t-1".into(),
        session_id: "s-1".into(),
        title: "fix bug".into(),
        summary: "in auth".into(),
        epic_id: Some("e-1".into()),
        status: "created".into(),
        created_at: "2026-06-01T00:00:00Z".into(),
        updated_at: "2026-06-01T00:00:00Z".into(),
        ..Default::default()
    };
    let json = serde_json::to_string(&task).unwrap();
    let parsed: StoredTaskRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.task_id, "t-1");
    assert_eq!(parsed.epic_id, Some("e-1".into()));
}

#[test]
fn stored_task_record_optionals_default_to_none() {
    let json = r#"{"task_id":"t-1","session_id":"s-1","title":"t","summary":"","status":"c","created_at":"t","updated_at":"t"}"#;
    let task: StoredTaskRecord = serde_json::from_str(json).unwrap();
    assert!(task.epic_id.is_none());
    assert!(task.creator_worker_id.is_none());
    assert!(task.claimed_by_worker_id.is_none());
    assert!(task.artifact_refs.is_empty());
}

#[test]
fn task_mutation_receipt_construction() {
    let task = StoredTaskRecord {
        task_id: "t-1".into(),
        session_id: "s-1".into(),
        title: "fix".into(),
        summary: "desc".into(),
        status: "closed".into(),
        created_at: "t".into(),
        updated_at: "t".into(),
        ..Default::default()
    };
    let receipt = TaskMutationReceipt {
        task: task.clone(),
        artifact_refs: vec!["a.rs".into()],
    };
    assert_eq!(receipt.task.task_id, "t-1");
    assert_eq!(receipt.artifact_refs.len(), 1);
}
