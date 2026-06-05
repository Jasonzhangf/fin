use super::test_helpers::{context_with_runtime_home, context_with_runtime_home_and_cwd, refs, temp_runtime_home};
use crate::model_output::ModelToolCall;
use super::tool_dispatch::execute_model_tools;
use super::tool_dispatch as tool_dispatch;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "tool_dispatch_tests_collab.rs"]
mod collab;
#[path = "tool_dispatch_tests_exec_patch.rs"]
mod exec_patch;
#[path = "tool_dispatch_tests_stateful.rs"]
mod stateful;
