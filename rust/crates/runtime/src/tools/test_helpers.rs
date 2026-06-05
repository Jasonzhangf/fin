//! Shared test helpers for tools/ test modules.
//! All siblings in this directory #[path]-include these or call them directly.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model_output::ModelToolCall;
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};

pub(crate) fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-runtime-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

pub(crate) fn context_with_runtime_home(runtime_home: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

pub(crate) fn context_with_runtime_home_and_cwd(
    runtime_home: &Path,
    cwd: &Path,
) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: Some(cwd.display().to_string()),
            project_root: Some(cwd.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

pub(crate) fn refs() -> EntityRefs {
    EntityRefs {
        session_id: Some("session-tool-dispatch".into()),
        task_id: Some("task-tool-dispatch".into()),
        worker_id: Some("worker-tool-dispatch".into()),
        ..EntityRefs::default()
    }
}

pub(crate) fn sample_tool_call(name: &str, args: serde_json::Value) -> ModelToolCall {
    ModelToolCall {
        tool_name: name.into(),
        tool_call_id: None,
        arguments: args,
    }
}
