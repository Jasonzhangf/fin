use crate::RuntimeError;
use fin_contracts::{
    KnowledgeLedgerRecord, LedgerIdentityRecord, LedgerRecordEnvelope, LedgerRefs,
    LedgerTimelineIndexRecord, LedgerTrackKind,
};
use fin_shared::append_jsonl;
use fin_shared::require_non_empty;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const LEDGER_SCHEMA_VERSION: &str = "fin.ledger.v1";

#[derive(Debug, Clone)]
pub struct LedgerStore {
    root: PathBuf,
    ledger_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendLedgerRecordInput {
    pub ts: String,
    pub track: LedgerTrackKind,
    pub record_id: String,
    pub record_kind: String,
    pub refs: LedgerRefs,
    pub payload: Value,
    pub caused_by: Option<String>,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LedgerQuery {
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub track: Option<LedgerTrackKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshotRebuildReport {
    pub ledger_id: String,
    pub session_id: String,
    pub detail_count: usize,
    pub snapshot_count: usize,
    pub status: String,
    pub missing_snapshot_detail_ids: Vec<String>,
}

impl LedgerStore {
    pub fn for_session(
        runtime_home: impl AsRef<Path>,
        session_id: &str,
    ) -> Result<Self, RuntimeError> {
        require_non_empty("session_id", session_id)?;
        Ok(Self {
            root: runtime_home.as_ref().join("ledgers").join(session_id),
            ledger_id: session_id.to_string(),
        })
    }

    pub fn for_project(
        runtime_home: impl AsRef<Path>,
        project_id: &str,
    ) -> Result<Self, RuntimeError> {
        require_non_empty("project_id", project_id)?;
        Ok(Self {
            root: runtime_home
                .as_ref()
                .join("ledgers")
                .join(format!("project-{project_id}")),
            ledger_id: format!("project-{project_id}"),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ledger_id(&self) -> &str {
        &self.ledger_id
    }

    pub fn init(
        &self,
        project_id: Option<&str>,
        session_id: Option<&str>,
        created_at: &str,
    ) -> Result<LedgerIdentityRecord, RuntimeError> {
        require_non_empty("created_at", created_at)?;
        create_dir_all(&self.root)?;
        create_dir_all(&self.root.join("timeline"))?;
        create_dir_all(&self.root.join("tracks"))?;
        create_dir_all(&self.root.join("snapshots"))?;
        create_dir_all(&self.root.join("indexes"))?;
        for track in all_tracks() {
            ensure_file(&self.root.join("tracks").join(track.file_name()))?;
        }
        ensure_file(&self.root.join("timeline/index.jsonl"))?;
        let identity_path = self.root.join("ledger.json");
        if identity_path.exists() {
            let existing: LedgerIdentityRecord = read_json(&identity_path)?;
            if existing.ledger_id != self.ledger_id {
                return Err(RuntimeError::State(format!(
                    "ledger identity mismatch: path={} expected={} actual={}",
                    identity_path.display(),
                    self.ledger_id,
                    existing.ledger_id
                )));
            }
            return Ok(existing);
        }
        let identity = LedgerIdentityRecord {
            ledger_id: self.ledger_id.clone(),
            schema_version: LEDGER_SCHEMA_VERSION.into(),
            project_id: project_id.map(str::to_string),
            session_id: session_id.map(str::to_string),
            created_at: created_at.into(),
        };
        write_json(&identity_path, &identity)?;
        Ok(identity)
    }

    pub fn append(
        &self,
        input: AppendLedgerRecordInput,
    ) -> Result<LedgerRecordEnvelope, RuntimeError> {
        validate_append_input(&input)?;
        self.init(
            input.refs.entity.task_id.as_deref(),
            input.refs.entity.session_id.as_deref(),
            input.ts.as_str(),
        )?;
        let seq = self.next_seq()?;
        let envelope = LedgerRecordEnvelope {
            ledger_id: self.ledger_id.clone(),
            seq,
            ts: input.ts,
            track: input.track,
            record_id: input.record_id,
            record_kind: input.record_kind,
            refs: input.refs,
            payload: input.payload,
            caused_by: input.caused_by,
            supersedes: input.supersedes,
        };
        append_jsonl(
            &self.root.join("tracks").join(envelope.track.file_name()),
            &envelope,
        )?;
        let index = LedgerTimelineIndexRecord {
            ledger_id: envelope.ledger_id.clone(),
            seq: envelope.seq,
            ts: envelope.ts.clone(),
            track: envelope.track.clone(),
            record_id: envelope.record_id.clone(),
            record_kind: envelope.record_kind.clone(),
            refs: envelope.refs.clone(),
        };
        append_jsonl(&self.root.join("timeline/index.jsonl"), &index)?;
        Ok(envelope)
    }

    pub fn append_knowledge(
        &self,
        record: KnowledgeLedgerRecord,
        refs: LedgerRefs,
    ) -> Result<LedgerRecordEnvelope, RuntimeError> {
        if record.evidence_refs.is_empty() {
            return Err(RuntimeError::State(
                "knowledge ledger record requires at least one evidence ref".into(),
            ));
        }
        for evidence in &record.evidence_refs {
            self.resolve_record_ref(evidence)?;
        }
        self.append(AppendLedgerRecordInput {
            ts: record.created_at.clone(),
            track: LedgerTrackKind::Knowledge,
            record_id: record.knowledge_id.clone(),
            record_kind: "knowledge".into(),
            refs,
            payload: serde_json::to_value(record)?,
            caused_by: None,
            supersedes: None,
        })
    }

    pub fn query(
        &self,
        query: &LedgerQuery,
    ) -> Result<Vec<LedgerTimelineIndexRecord>, RuntimeError> {
        let records = read_jsonl_or_empty::<LedgerTimelineIndexRecord>(
            &self.root.join("timeline/index.jsonl"),
        )?;
        Ok(records
            .into_iter()
            .filter(|record| {
                query
                    .track
                    .as_ref()
                    .is_none_or(|track| &record.track == track)
            })
            .filter(|record| {
                query
                    .session_id
                    .as_deref()
                    .is_none_or(|value| record.refs.entity.session_id.as_deref() == Some(value))
            })
            .filter(|record| {
                query
                    .task_id
                    .as_deref()
                    .is_none_or(|value| record.refs.entity.task_id.as_deref() == Some(value))
            })
            .filter(|record| {
                query
                    .agent_id
                    .as_deref()
                    .is_none_or(|value| record.refs.agent_id.as_deref() == Some(value))
            })
            .collect())
    }

    pub fn read_track(
        &self,
        track: LedgerTrackKind,
    ) -> Result<Vec<LedgerRecordEnvelope>, RuntimeError> {
        read_jsonl_or_empty::<LedgerRecordEnvelope>(
            &self.root.join("tracks").join(track.file_name()),
        )
    }

    pub fn rebuild_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<SessionSnapshotRebuildReport, RuntimeError> {
        require_non_empty("session_id", session_id)?;
        let details = read_jsonl_or_empty::<LedgerRecordEnvelope>(
            &self.root.join("tracks/session.detail.jsonl"),
        )?
        .into_iter()
        .filter(|record| record.refs.entity.session_id.as_deref() == Some(session_id))
        .collect::<Vec<_>>();
        let snapshots = read_jsonl_or_empty::<LedgerRecordEnvelope>(
            &self.root.join("tracks/session.snapshot.jsonl"),
        )?
        .into_iter()
        .filter(|record| record.refs.entity.session_id.as_deref() == Some(session_id))
        .collect::<Vec<_>>();
        let snapshot_causes = snapshots
            .iter()
            .filter_map(|record| record.caused_by.as_deref())
            .collect::<std::collections::BTreeSet<_>>();
        let missing_snapshot_detail_ids = details
            .iter()
            .filter(|detail| !snapshot_causes.contains(detail.record_id.as_str()))
            .map(|detail| detail.record_id.clone())
            .collect::<Vec<_>>();
        let status = if missing_snapshot_detail_ids.is_empty() {
            "ok"
        } else {
            "drift"
        };
        let report = SessionSnapshotRebuildReport {
            ledger_id: self.ledger_id.clone(),
            session_id: session_id.into(),
            detail_count: details.len(),
            snapshot_count: snapshots.len(),
            status: status.into(),
            missing_snapshot_detail_ids,
        };
        write_json(&self.root.join("snapshots/current_session.json"), &report)?;
        Ok(report)
    }

    pub fn resolve_record_ref(&self, record_ref: &str) -> Result<(), RuntimeError> {
        require_non_empty("record_ref", record_ref)?;
        let records = self.query(&LedgerQuery::default())?;
        if records.iter().any(|record| {
            record.record_id == record_ref
                || format!("{}:{}", record.track.as_str(), record.record_id) == record_ref
        }) {
            return Ok(());
        }
        Err(RuntimeError::State(format!(
            "ledger evidence ref not found: {record_ref}"
        )))
    }

    fn next_seq(&self) -> Result<u64, RuntimeError> {
        let records = read_jsonl_or_empty::<LedgerTimelineIndexRecord>(
            &self.root.join("timeline/index.jsonl"),
        )?;
        Ok(records.last().map_or(1, |record| record.seq + 1))
    }
}

fn all_tracks() -> [LedgerTrackKind; 9] {
    [
        LedgerTrackKind::SessionDetail,
        LedgerTrackKind::SessionSnapshot,
        LedgerTrackKind::Events,
        LedgerTrackKind::Turns,
        LedgerTrackKind::Steps,
        LedgerTrackKind::Tools,
        LedgerTrackKind::Provider,
        LedgerTrackKind::Control,
        LedgerTrackKind::Knowledge,
    ]
}

fn validate_append_input(input: &AppendLedgerRecordInput) -> Result<(), RuntimeError> {
    require_non_empty("record_id", &input.record_id)?;
    require_non_empty("record_kind", &input.record_kind)?;
    require_non_empty("ts", &input.ts)?;
    Ok(())
}

fn create_dir_all(path: &Path) -> Result<(), RuntimeError> {
    fs::create_dir_all(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn ensure_file(path: &Path) -> Result<(), RuntimeError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    fs::File::create(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn read_jsonl_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, RuntimeError> {
    let content = fs::read_to_string(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&content).map_err(RuntimeError::Serialize)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RuntimeError> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(path, body).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}
