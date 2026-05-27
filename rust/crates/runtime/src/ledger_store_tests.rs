use super::ledger_store::{
    AppendLedgerRecordInput, LedgerQuery, LedgerStore,
};
use fin_contracts::{EntityRefs, KnowledgeLedgerRecord, LedgerRefs, LedgerTrackKind};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_home(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "fin-ledger-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn refs() -> EntityRefs {
    EntityRefs {
        session_id: Some("session-ledger".into()),
        task_id: Some("task-ledger".into()),
        worker_id: Some("worker-system".into()),
        ..EntityRefs::default()
    }
}

fn ledger_refs_from_entity(entity: EntityRefs, record_refs: Vec<String>) -> LedgerRefs {
    LedgerRefs {
        agent_id: entity.worker_id.clone(),
        entity,
        ledger_id: None,
        record_refs,
    }
}

#[test]
fn ledger_store_initializes_identity_and_appends_monotonic_timeline() {
    let home = temp_home("init");
    let store = LedgerStore::for_session(&home, "session-ledger").expect("store");
    let first = store
        .append(AppendLedgerRecordInput {
            ts: "2026-05-23T12:00:00+08:00".into(),
            track: LedgerTrackKind::SessionDetail,
            record_id: "detail-1".into(),
            record_kind: "session_detail".into(),
            refs: ledger_refs_from_entity(refs(), Vec::new()),
            payload: json!({"turn":"one"}),
            caused_by: None,
            supersedes: None,
        })
        .expect("append first");
    let second = store
        .append(AppendLedgerRecordInput {
            ts: "2026-05-23T12:00:01+08:00".into(),
            track: LedgerTrackKind::SessionSnapshot,
            record_id: "snapshot-1".into(),
            record_kind: "session_snapshot".into(),
            refs: ledger_refs_from_entity(refs(), vec!["detail-1".into()]),
            payload: json!({"summary":"one"}),
            caused_by: Some("detail-1".into()),
            supersedes: None,
        })
        .expect("append second");
    assert_eq!(first.seq, 1);
    assert_eq!(second.seq, 2);
    assert!(store.root().join("ledger.json").exists());
    assert!(store.root().join("timeline/index.jsonl").exists());
    assert!(store.root().join("tracks/session.detail.jsonl").exists());

    let session_records = store
        .query(&LedgerQuery {
            session_id: Some("session-ledger".into()),
            ..LedgerQuery::default()
        })
        .expect("query");
    assert_eq!(session_records.len(), 2);
}

#[test]
fn snapshot_rebuild_reports_missing_snapshot_detail_refs() {
    let home = temp_home("snapshot");
    let store = LedgerStore::for_session(&home, "session-ledger").expect("store");
    store
        .append(AppendLedgerRecordInput {
            ts: "2026-05-23T12:00:00+08:00".into(),
            track: LedgerTrackKind::SessionDetail,
            record_id: "detail-1".into(),
            record_kind: "session_detail".into(),
            refs: ledger_refs_from_entity(refs(), Vec::new()),
            payload: json!({"turn":"one"}),
            caused_by: None,
            supersedes: None,
        })
        .expect("append detail");
    let report = store
        .rebuild_session_snapshot("session-ledger")
        .expect("rebuild");
    assert_eq!(report.status, "drift");
    assert_eq!(report.missing_snapshot_detail_ids, vec!["detail-1"]);
}

#[test]
fn knowledge_track_requires_existing_evidence() {
    let home = temp_home("knowledge");
    let store = LedgerStore::for_session(&home, "session-ledger").expect("store");
    store
        .append(AppendLedgerRecordInput {
            ts: "2026-05-23T12:00:00+08:00".into(),
            track: LedgerTrackKind::SessionDetail,
            record_id: "detail-1".into(),
            record_kind: "session_detail".into(),
            refs: ledger_refs_from_entity(refs(), Vec::new()),
            payload: json!({"turn":"one"}),
            caused_by: None,
            supersedes: None,
        })
        .expect("append detail");

    let missing = KnowledgeLedgerRecord {
        knowledge_id: "knowledge-missing".into(),
        scope: "project".into(),
        statement: "missing evidence should fail".into(),
        evidence_refs: vec!["missing-record".into()],
        source_record_ids: Vec::new(),
        confidence: 80,
        created_at: "2026-05-23T12:00:01+08:00".into(),
        valid_from: "2026-05-23T12:00:01+08:00".into(),
        supersedes: None,
        tags: Vec::new(),
    };
    assert!(
        store
            .append_knowledge(missing, ledger_refs_from_entity(refs(), Vec::new()))
            .is_err()
    );

    let valid = KnowledgeLedgerRecord {
        knowledge_id: "knowledge-1".into(),
        scope: "project".into(),
        statement: "detail evidence exists".into(),
        evidence_refs: vec!["detail-1".into()],
        source_record_ids: vec!["detail-1".into()],
        confidence: 90,
        created_at: "2026-05-23T12:00:02+08:00".into(),
        valid_from: "2026-05-23T12:00:02+08:00".into(),
        supersedes: None,
        tags: vec!["test".into()],
    };
    let written = store
        .append_knowledge(valid, ledger_refs_from_entity(refs(), Vec::new()))
        .expect("append knowledge");
    assert_eq!(written.track, LedgerTrackKind::Knowledge);
}
