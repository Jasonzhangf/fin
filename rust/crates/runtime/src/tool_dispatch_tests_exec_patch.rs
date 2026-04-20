use super::*;

#[test]
fn exec_command_and_write_stdin_replay_session_work() {
    let runtime_home = temp_runtime_home("exec");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let first = execute_model_tools(
        "op-tools",
        "trace-tools",
        &refs(),
        "2026-04-18T21:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "exec_command".into(),
            arguments: json!({
                "cmd": "cat",
                "open_stdin_session": true,
            }),
        }],
    );
    let record = first
        .tool_records
        .iter()
        .find(|item| item.tool_name == "exec_command")
        .expect("exec_command record");
    assert_eq!(record.status, "completed");
    let session_path = record.artifact_refs.first().expect("session artifact path");
    let session_id = Path::new(session_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .expect("session file stem")
        .to_string();

    let second = execute_model_tools(
        "op-tools",
        "trace-tools",
        &refs(),
        "2026-04-18T21:00:01+08:00",
        &context,
        2,
        &[ModelToolCall {
            tool_name: "write_stdin".into(),
            arguments: json!({
                "session_id": session_id,
                "chars": "hello-from-stdin",
            }),
        }],
    );
    let write_record = second
        .tool_records
        .iter()
        .find(|item| item.tool_name == "write_stdin")
        .expect("write_stdin record");
    assert_eq!(write_record.status, "completed");
    assert!(
        write_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("hello-from-stdin")
    );
    assert!(
        second
            .events
            .iter()
            .any(|(event_type, _)| event_type == "tool.write_stdin_completed")
    );
}

#[test]
fn apply_patch_replace_mode_updates_file_and_writes_receipt() {
    let runtime_home = temp_runtime_home("apply-patch-replace");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let file_path = workspace.join("sample.txt");
    fs::write(
        &file_path,
        "alpha
beta
",
    )
    .expect("seed file");
    let context = context_with_runtime_home_and_cwd(&runtime_home, &workspace);

    let result = execute_model_tools(
        "op-apply-patch-replace",
        "trace-apply-patch-replace",
        &refs(),
        "2026-04-20T12:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "apply_patch".into(),
            arguments: json!({
                "path": "sample.txt",
                "old_string": "beta",
                "new_string": "gamma",
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "apply_patch")
        .expect("apply_patch record");
    assert_eq!(record.status, "completed");
    assert_eq!(
        fs::read_to_string(&file_path).expect("patched file"),
        "alpha
gamma
"
    );
    assert!(
        result
            .events
            .iter()
            .any(|(event_type, _)| event_type == "tool.apply_patch_completed")
    );
    assert!(
        record
            .artifact_refs
            .iter()
            .any(|value| value.contains("sample.txt"))
    );
}

#[test]
fn apply_patch_patch_mode_supports_update_and_add() {
    let runtime_home = temp_runtime_home("apply-patch-v4a");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let file_path = workspace.join("src.txt");
    fs::write(
        &file_path,
        "before
stay
",
    )
    .expect("seed file");
    let context = context_with_runtime_home_and_cwd(&runtime_home, &workspace);

    let result = execute_model_tools(
        "op-apply-patch-v4a",
        "trace-apply-patch-v4a",
        &refs(),
        "2026-04-20T12:00:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "apply_patch".into(),
            arguments: json!({
                "mode": "patch",
                "patch": "*** Begin Patch
*** Update File: src.txt
@@
-before
+after
*** Add File: added.txt
+hello
+world
*** End Patch
",
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "apply_patch")
        .expect("apply_patch record");
    assert_eq!(record.status, "completed");
    assert_eq!(
        fs::read_to_string(&file_path).expect("updated file"),
        "after
stay
"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("added.txt")).expect("added file"),
        "hello
world
"
    );
    assert!(
        record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("created=1")
    );
}
