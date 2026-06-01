use super::*;

#[test]
fn ledger_track_kind_as_str_and_file_name() {
    let cases = [
        (
            LedgerTrackKind::SessionDetail,
            "session.detail",
            "session.detail.jsonl",
        ),
        (
            LedgerTrackKind::SessionSnapshot,
            "session.snapshot",
            "session.snapshot.jsonl",
        ),
        (LedgerTrackKind::Events, "events", "events.jsonl"),
        (LedgerTrackKind::Turns, "turns", "turns.jsonl"),
        (LedgerTrackKind::Steps, "steps", "steps.jsonl"),
        (LedgerTrackKind::Tools, "tools", "tools.jsonl"),
        (LedgerTrackKind::Provider, "provider", "provider.jsonl"),
        (LedgerTrackKind::Control, "control", "control.jsonl"),
        (LedgerTrackKind::Knowledge, "knowledge", "knowledge.jsonl"),
    ];
    for (kind, expected_str, expected_fn) in cases {
        assert_eq!(kind.as_str(), expected_str);
        assert_eq!(kind.file_name(), expected_fn);
    }
}

#[test]
fn ledger_record_envelope_json_roundtrip() {
    let envelope = LedgerRecordEnvelope {
        ledger_id: "lid-1".into(),
        seq: 42,
        ts: "2026-06-01T00:00:00Z".into(),
        track: LedgerTrackKind::Turns,
        record_id: "rid-1".into(),
        record_kind: "turn".into(),
        refs: LedgerRefs {
            entity: EntityRefs::default(),
            ..Default::default()
        },
        payload: serde_json::json!({"turn": 1}),
        caused_by: None,
        supersedes: None,
    };
    let json = serde_json::to_string(&envelope).unwrap();
    let parsed: LedgerRecordEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.ledger_id, "lid-1");
    assert_eq!(parsed.seq, 42);
    assert_eq!(parsed.track, LedgerTrackKind::Turns);
}

#[test]
fn ledger_timeline_index_record_roundtrip() {
    let record = LedgerTimelineIndexRecord {
        ledger_id: "lid-2".into(),
        seq: 7,
        ts: "2026-06-01T00:00:00Z".into(),
        track: LedgerTrackKind::Tools,
        record_id: "rid-2".into(),
        record_kind: "tool".into(),
        refs: LedgerRefs {
            entity: EntityRefs::default(),
            ..Default::default()
        },
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: LedgerTimelineIndexRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.seq, 7);
    assert_eq!(parsed.track, LedgerTrackKind::Tools);
}

#[test]
fn ledger_refs_default() {
    let refs = LedgerRefs::default();
    assert!(refs.entity.session_id.is_none());
    assert!(refs.agent_id.is_none());
    assert!(refs.ledger_id.is_none());
    assert!(refs.record_refs.is_empty());
}

#[test]
fn session_detail_record_optionals() {
    let json = r#"{"detail_id":"d-1","operation_id":"op-1","trace_id":"t-1","turn_id":"tr-1","created_at":"2026-06-01"}"#;
    let record: SessionDetailRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.detail_id, "d-1");
    assert!(record.user_input.is_none());
    assert!(record.assistant_visible_output.is_none());
    assert!(record.tool_refs.is_empty());
}

#[test]
fn ledger_identity_record_roundtrip() {
    let record = LedgerIdentityRecord {
        ledger_id: "lid-3".into(),
        schema_version: "1.0".into(),
        project_id: Some("fin".into()),
        session_id: None,
        created_at: "2026-06-01T00:00:00Z".into(),
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: LedgerIdentityRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.schema_version, "1.0");
    assert_eq!(parsed.project_id.as_deref(), Some("fin"));
}

#[test]
fn knowledge_ledger_record_roundtrip() {
    let record = KnowledgeLedgerRecord {
        knowledge_id: "k-1".into(),
        scope: "project".into(),
        statement: "auth uses jwt".into(),
        evidence_refs: vec!["ev-1".into()],
        source_record_ids: vec![],
        confidence: 85,
        created_at: "2026-06-01T00:00:00Z".into(),
        valid_from: "2026-06-01T00:00:00Z".into(),
        supersedes: None,
        tags: vec![],
    };
    let json = serde_json::to_string(&record).unwrap();
    let parsed: KnowledgeLedgerRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.confidence, 85);
}

#[test]
fn token_usage_record_defaults() {
    let usage = TokenUsageRecord::default();
    assert!(usage.prompt_tokens.is_none());
    assert!(usage.completion_tokens.is_none());
    assert_eq!(usage.usage_source, "");
}

#[test]
fn control_feedback_default_values() {
    let cf = ControlFeedback::default();
    assert_eq!(cf.origin, "");
    assert!(!cf.is_continuation);
    assert!(!cf.is_simple_query);
    assert!(cf.candidate_task_id.is_none());
    assert_eq!(cf.continuity_confidence, 0);
    assert_eq!(cf.topic_shift_confidence, 0);
    assert_eq!(cf.simple_query_confidence, 0);
    assert!(cf.note_candidate.is_empty());
}

#[test]
fn daemon_state_record_flatten_and_optionals() {
    let json = r#"{"daemon_id":"d-1","created_at":"t1","updated_at":"t2","service_kind":"agent","lifecycle_state":"running","supervision_state":"active","mode":"attached","recovery_needed":false,"status_summary":"ok"}"#;
    let record: DaemonStateRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.daemon_id, "d-1");
    assert!(record.pid.is_none());
    assert_eq!(record.recovery_needed, false);
    assert!(record.refs.session_id.is_none());
}
