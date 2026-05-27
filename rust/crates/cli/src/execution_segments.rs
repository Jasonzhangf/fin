use crate::CliError;
use fin_contracts::{InterruptedSegmentRecord, PauseCheckpointRecord, SegmentMergeRecord};
use fin_debug_server::DebugBinding;
use fin_runtime::{ClosureRun, apply_segment_merge, interrupted_segment, segment_merge};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const RECENT_SEGMENT_LIMIT: usize = 32;
const RECENT_MERGE_LIMIT: usize = 32;

#[derive(Debug, Clone)]
struct SessionSegmentPaths {
    session_dir: PathBuf,
}

impl SessionSegmentPaths {
    fn recent_segments_path(&self) -> PathBuf {
        self.session_dir.join("interrupts/recent_segments.json")
    }

    fn latest_segment_path(&self) -> PathBuf {
        self.session_dir.join("interrupts/latest_segment.json")
    }

    fn recent_merges_path(&self) -> PathBuf {
        self.session_dir.join("interrupts/recent_merges.json")
    }

    fn latest_merge_path(&self) -> PathBuf {
        self.session_dir.join("interrupts/latest_merge.json")
    }
}

pub(crate) fn create_interrupted_segment(
    runtime_home: &Path,
    binding: &DebugBinding,
    checkpoint: &PauseCheckpointRecord,
) -> Result<Option<InterruptedSegmentRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    ensure_dirs(&paths)?;
    let refs = fin_contracts::EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..fin_contracts::EntityRefs::default()
    };
    let segment = interrupted_segment(&refs, binding.session_id.as_deref(), checkpoint);
    persist_segment(runtime_home, &paths, &segment)?;
    Ok(Some(segment))
}

pub(crate) fn latest_open_segment(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<InterruptedSegmentRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    let segments = read_json_or_empty::<InterruptedSegmentRecord>(&paths.recent_segments_path())?;
    Ok(segments
        .into_iter()
        .rev()
        .find(|segment| segment.status == "open"))
}

pub(crate) fn merge_segment_into_run(
    runtime_home: &Path,
    binding: &DebugBinding,
    segment: &InterruptedSegmentRecord,
    run: &ClosureRun,
) -> Result<SegmentMergeRecord, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Err(CliError::Usage);
    };
    ensure_dirs(&paths)?;
    let merge = segment_merge(segment, run);

    let mut segments =
        read_json_or_empty::<InterruptedSegmentRecord>(&paths.recent_segments_path())?;
    let updated_segment = apply_segment_merge(
        &mut segments,
        segment,
        &run.turn_record.turn_id,
        &run.operation.operation_id,
        &run.note.created_at,
    );
    trim_head(&mut segments, RECENT_SEGMENT_LIMIT);
    write_json(&paths.recent_segments_path(), &segments)?;
    if let Some(latest) = segments.last() {
        write_json(&paths.latest_segment_path(), latest)?;
    }
    write_json(
        &runtime_home.join("runtime/current/current_interrupted_segment.json"),
        updated_segment.as_ref().unwrap_or(segment),
    )?;

    let mut merges = read_json_or_empty::<SegmentMergeRecord>(&paths.recent_merges_path())?;
    merges.push(merge.clone());
    trim_head(&mut merges, RECENT_MERGE_LIMIT);
    write_json(&paths.recent_merges_path(), &merges)?;
    write_json(&paths.latest_merge_path(), &merge)?;
    write_json(
        &runtime_home.join("runtime/current/current_segment_merge.json"),
        &merge,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_interrupted_segment_path": "runtime/current/current_interrupted_segment.json",
            "current_segment_merge_path": "runtime/current/current_segment_merge.json"
        }),
    )?;
    Ok(merge)
}

fn persist_segment(
    runtime_home: &Path,
    paths: &SessionSegmentPaths,
    segment: &InterruptedSegmentRecord,
) -> Result<(), CliError> {
    let mut segments =
        read_json_or_empty::<InterruptedSegmentRecord>(&paths.recent_segments_path())?;
    segments.push(segment.clone());
    trim_head(&mut segments, RECENT_SEGMENT_LIMIT);
    write_json(&paths.recent_segments_path(), &segments)?;
    write_json(&paths.latest_segment_path(), segment)?;
    write_json(
        &runtime_home.join("runtime/current/current_interrupted_segment.json"),
        segment,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_interrupted_segment_path": "runtime/current/current_interrupted_segment.json"
        }),
    )
}

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SessionSegmentPaths>, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|dir| SessionSegmentPaths { session_dir: dir }))
}

fn find_session_dir(runtime_home: &Path, session_id: &str) -> Option<PathBuf> {
    let root = runtime_home.join("sessions");
    let years = fs::read_dir(root).ok()?;
    for year in years.flatten() {
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        let year_name = year.file_name().to_string_lossy().to_string();
        if year_name.len() != 4 || !year_name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let months = fs::read_dir(year_path).ok()?;
        for month in months.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let month_name = month.file_name().to_string_lossy().to_string();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let dir = month_path.join(session_id);
            if dir.is_dir() {
                return Some(dir);
            }
        }
    }
    None
}

fn ensure_dirs(paths: &SessionSegmentPaths) -> Result<(), CliError> {
    fs::create_dir_all(paths.session_dir.join("interrupts")).map_err(|source| CliError::WriteFile {
        path: paths.session_dir.join("interrupts").display().to_string(),
        source,
    })
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn update_last_run_paths(runtime_home: &Path, updates: Value) -> Result<(), CliError> {
    let last_run_path = runtime_home.join("runtime/current/last_run.json");
    let mut value = match fs::read_to_string(&last_run_path) {
        Ok(content) => serde_json::from_str::<Value>(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: last_run_path.display().to_string(),
                source,
            });
        }
    };
    if !value.is_object() {
        value = json!({});
    }
    let object = value.as_object_mut().expect("object");
    for (key, val) in updates.as_object().into_iter().flatten() {
        object.insert(key.clone(), val.clone());
    }
    write_json(&last_run_path, &value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_segment_id_is_unique_across_pause_timestamps() {
        let binding = DebugBinding {
            project_id: "fin".into(),
            project_label: "fin".into(),
            runtime_home: "/tmp/fin".into(),
            session_id: Some("session-alpha".into()),
            task_id: Some("task-alpha".into()),
            session_messages_path: None,
            recent_contexts_path: None,
            recent_digests_path: None,
        };
        let first = PauseCheckpointRecord {
            checkpoint_id: "pause-1".into(),
            refs: fin_contracts::EntityRefs::default(),
            turn_id: Some("turn-op-1".into()),
            active_step_id: Some("step-op-1-05-tool_dispatch".into()),
            resume_from_step_id: Some("step-op-1-05-tool_dispatch".into()),
            resume_checkpoint_id: None,
            reason: Some("manual pause".into()),
            paused_at: "2026-04-19T20:00:01+08:00".into(),
        };
        let second = PauseCheckpointRecord {
            paused_at: "2026-04-19T20:00:02+08:00".into(),
            ..first.clone()
        };
        let refs = fin_contracts::EntityRefs {
            session_id: binding.session_id.clone(),
            task_id: binding.task_id.clone(),
            ..fin_contracts::EntityRefs::default()
        };
        assert_ne!(
            interrupted_segment(&refs, binding.session_id.as_deref(), &first).segment_id,
            interrupted_segment(&refs, binding.session_id.as_deref(), &second).segment_id
        );
    }

    #[test]
    fn update_segment_merge_status_only_updates_exact_open_match() {
        let mut segments = vec![
            InterruptedSegmentRecord {
                segment_id: "segment-same".into(),
                interrupted_turn_id: Some("turn-old".into()),
                interrupted_step_id: Some("step-old".into()),
                created_at: "2026-04-19T20:00:01+08:00".into(),
                status: "merged".into(),
                ..InterruptedSegmentRecord::default()
            },
            InterruptedSegmentRecord {
                segment_id: "segment-same".into(),
                interrupted_turn_id: Some("turn-target".into()),
                interrupted_step_id: Some("step-target".into()),
                created_at: "2026-04-19T20:00:02+08:00".into(),
                status: "open".into(),
                ..InterruptedSegmentRecord::default()
            },
        ];
        let target = segments[1].clone();
        let updated = apply_segment_merge(
            &mut segments,
            &target,
            "turn-resumed",
            "op-resumed",
            "2026-04-19T20:05:00+08:00",
        )
        .expect("updated segment");
        assert_eq!(updated.status, "merged");
        assert_eq!(segments[0].interrupted_turn_id, Some("turn-old".into()));
        assert_eq!(segments[0].status, "merged");
        assert_eq!(
            segments[1].merged_into_operation_id.as_deref(),
            Some("op-resumed")
        );
    }
}
