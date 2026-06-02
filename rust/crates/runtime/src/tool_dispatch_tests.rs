use crate::{model_output::ModelToolCall, tool_dispatch::execute_model_tools};
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-runtime-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn context_with_runtime_home(runtime_home: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

fn context_with_runtime_home_and_cwd(runtime_home: &Path, cwd: &Path) -> MinimalContextView {
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

fn refs() -> EntityRefs {
    EntityRefs {
        session_id: Some("session-tool-dispatch".into()),
        task_id: Some("task-tool-dispatch".into()),
        worker_id: Some("worker-tool-dispatch".into()),
        ..EntityRefs::default()
    }
}

#[path = "tool_dispatch_tests_collab.rs"]
mod collab;
#[path = "tool_dispatch_tests_exec_patch.rs"]
mod exec_patch;
#[path = "tool_dispatch_tests_stateful.rs"]
mod stateful;
