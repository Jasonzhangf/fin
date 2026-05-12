use super::*;

fn authoritative_receipt(
    runtime_home: &Path,
    record: &fin_contracts::ToolExecutionRecord,
) -> serde_json::Value {
    let receipt_ref = record
        .artifact_refs
        .iter()
        .find(|value| {
            value.contains("runtime/tools/tool_receipts/")
                || value.contains("runtime/tools/exec_receipts/")
                || value.contains("runtime/tools/write_stdin_receipts/")
                || value.contains("runtime/tools/patch_receipts/")
        })
        .expect("authoritative receipt ref");
    serde_json::from_str(&fs::read_to_string(runtime_home.join(receipt_ref)).expect("receipt text"))
        .expect("receipt json")
}

#[test]
fn exec_command_non_zero_receipt_exposes_retry_guidance() {
    let runtime_home = temp_runtime_home("exec-non-zero");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let result = execute_model_tools(
        "op-exec-non-zero",
        "trace-exec-non-zero",
        &refs(),
        "2026-04-23T23:10:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_call_id: Some("tool-exec-non-zero-1".into()),
            tool_name: "exec_command".into(),
            arguments: json!({
                "cmd": "exit 7",
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "exec_command")
        .expect("exec_command record");
    assert_eq!(record.status, "failed");
    let receipt = authoritative_receipt(&runtime_home, record);
    assert_eq!(receipt["failure_kind"], "command_non_zero_exit");
    assert!(
        receipt["retry_hint"]
            .as_str()
            .unwrap_or_default()
            .contains("stdout/stderr")
    );
}

#[test]
fn write_stdin_missing_exec_session_receipt_exposes_recovery_hint() {
    let runtime_home = temp_runtime_home("write-stdin-missing-session");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let result = execute_model_tools(
        "op-write-stdin-missing-session",
        "trace-write-stdin-missing-session",
        &refs(),
        "2026-04-23T23:10:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_call_id: Some("tool-write-stdin-missing-1".into()),
            tool_name: "write_stdin".into(),
            arguments: json!({
                "session_id": "missing-session",
                "chars": "hello"
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "write_stdin")
        .expect("write_stdin record");
    assert_eq!(record.status, "failed");
    let receipt = authoritative_receipt(&runtime_home, record);
    assert_eq!(receipt["failure_kind"], "missing_session_reference");
    assert!(
        receipt["retry_hint"]
            .as_str()
            .unwrap_or_default()
            .contains("open_stdin_session=true")
    );
}
