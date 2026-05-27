use fin_contracts::ToolSemanticView;
use std::path::Path;

pub(crate) fn project_recent_actions(
    runtime_home: &Path,
    peer_id: &str,
    peer_kind: &str,
) -> Vec<ToolSemanticView> {
    if peer_kind != "project_agent" {
        return Vec::new();
    }
    let Some(ledger_id) = project_ledger_id(runtime_home, peer_id) else {
        return Vec::new();
    };
    let tools = runtime_home
        .join("ledgers")
        .join(&ledger_id)
        .join("tracks/tools.jsonl");
    let provider = runtime_home
        .join("ledgers")
        .join(&ledger_id)
        .join("tracks/provider.jsonl");
    let mut actions = Vec::new();
    actions.extend(read_project_track_actions(&tools, "tool"));
    actions.extend(read_project_track_actions(&provider, "provider"));
    actions.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    actions.truncate(4);
    actions
}

fn project_ledger_id(runtime_home: &Path, peer_id: &str) -> Option<String> {
    let stripped = peer_id.strip_prefix("local.").unwrap_or(peer_id);
    let candidates = [
        peer_id.replace('.', "-"),
        stripped.replace('.', "-"),
        format!("{}-agent", stripped.replace('.', "-")),
    ];
    candidates.into_iter().find(|candidate| {
        runtime_home
            .join("ledgers")
            .join(candidate)
            .join("tracks")
            .exists()
    })
}

fn read_project_track_actions(path: &Path, category: &str) -> Vec<ToolSemanticView> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|record| project_action_from_record(&record, category))
        .collect()
}

fn project_action_from_record(record: &serde_json::Value, category: &str) -> ToolSemanticView {
    let payload = record.get("payload").and_then(serde_json::Value::as_object);
    let status = payload
        .and_then(|item| item.get("status"))
        .map(|value| value.to_string().trim_matches('"').to_string())
        .unwrap_or_else(|| "completed".into());
    let summary = payload
        .and_then(|item| item.get("summary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(category)
        .to_string();
    let tool_call_id = payload
        .and_then(|item| item.get("tool_call_id"))
        .or_else(|| payload.and_then(|item| item.get("request_id")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("project-action")
        .to_string();
    ToolSemanticView {
        tool_call_id,
        operation_id: record
            .get("record_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("project-ledger")
            .into(),
        tool_name: if category == "provider" {
            "provider.call"
        } else {
            "project.tool"
        }
        .into(),
        category: category.into(),
        verb: if category == "provider" {
            "call"
        } else {
            "execute"
        }
        .into(),
        object_kind: "project_agent".into(),
        object_label: category.into(),
        summary,
        detail: None,
        status: if status == "200" {
            "completed".into()
        } else {
            status
        },
        started_at: record
            .get("ts")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .into(),
        ended_at: None,
        artifact_refs: Vec::new(),
    }
}
