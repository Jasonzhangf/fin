use super::dispatch;
use super::dispatch::execute_model_tools;
use super::test_helpers::{
    context_with_runtime_home, context_with_runtime_home_and_cwd, refs, temp_runtime_home,
};
use crate::model::parser::ModelToolCall;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "dispatch_tests_collab.rs"]
mod collab;
#[path = "dispatch_tests_exec_patch.rs"]
mod exec_patch;
#[path = "dispatch_tests_stateful.rs"]
mod stateful;
