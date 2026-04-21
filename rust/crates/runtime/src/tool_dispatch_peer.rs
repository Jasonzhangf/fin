use crate::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
    runtime_home_from_context,
};
use fin_contracts::DaemonEnsurePeerRequestRecord;
use fin_contracts::ToolExecutionRecord;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PeerLeaseRecord {
    peer_id: String,
    peer_kind: String,
    lease_ttl_ms: u64,
    issued_at: String,
    expires_at_hint_ms: u64,
    owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PeerStateRecord {
    peer_id: String,
    peer_kind: String,
    lifecycle_state: String,
    last_heartbeat_at: String,
    reconnect_backoff_ms: u64,
}

pub(super) fn handle_peer_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let limit = read_u64(arguments, "limit").unwrap_or(20) as usize;
    let kind_filter = read_string(arguments, "kind")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let peers = input
        .context
        .peer
        .as_ref()
        .map(|peer| {
            peer.peers
                .iter()
                .filter(|item| {
                    kind_filter.is_empty() || item.peer_kind.eq_ignore_ascii_case(&kind_filter)
                })
                .take(limit)
                .map(|item| {
                    format!(
                        "{} [{}:{}]",
                        item.peer_id, item.peer_kind, item.presence_state
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let output_summary = if peers.is_empty() {
        "peer context block has no matching peers".to_string()
    } else {
        format!("{} peer(s): {}", peers.len(), peers.join(", "))
    };
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "peer.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Peer List".into(),
        purpose: "inspect peer topology snapshot before routing decisions".into(),
        target_kind: Some("peer_topology".into()),
        target_ref: Some("context.peer".into()),
        input_summary: Some(format!(
            "limit={}{}",
            limit,
            if kind_filter.is_empty() {
                String::new()
            } else {
                format!(", kind={kind_filter}")
            }
        )),
        output_summary: Some(output_summary),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_peer_context_snapshot".into()],
        artifact_refs: vec!["context/recent_contexts.json".into()],
        error_summary: None,
    });
    outcome.events.push((
        "peer.listed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": peers.len(),
            "kind_filter": if kind_filter.is_empty() { Value::Null } else { Value::String(kind_filter) },
        }),
    ));
    true
}

pub(super) fn handle_peer_describe(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(peer_id) = read_string(arguments, "peer_id") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "peer.describe",
            "missing required argument: peer_id",
        ));
        return true;
    };
    let maybe_peer = input.context.peer.as_ref().and_then(|block| {
        block
            .peers
            .iter()
            .find(|item| item.peer_id == peer_id)
            .cloned()
    });
    match maybe_peer {
        Some(peer) => {
            let output_summary = format!(
                "peer {} kind={} presence={} capabilities={}",
                peer.peer_id,
                peer.peer_kind,
                peer.presence_state,
                peer.capability_ids.join(", ")
            );
            outcome.tool_records.push(ToolExecutionRecord {
                tool_call_id: tool_call_id.into(),
                operation_id: input.operation_id.into(),
                trace_id: input.trace_id.into(),
                refs: input.refs.clone(),
                tool_name: "peer.describe".into(),
                tool_kind: "agent_tool".into(),
                title: "Peer Describe".into(),
                purpose: "inspect one peer for capability and binding hints".into(),
                target_kind: Some("peer".into()),
                target_ref: Some(peer.peer_id.clone()),
                input_summary: Some(format!("peer_id={peer_id}")),
                output_summary: Some(output_summary),
                status: "completed".into(),
                started_at: input.occurred_at.into(),
                ended_at: Some(input.occurred_at.into()),
                duration_ms: Some(0),
                side_effects: vec!["read_peer_descriptor".into()],
                artifact_refs: vec!["context/recent_contexts.json".into()],
                error_summary: None,
            });
            outcome.events.push((
                "peer.described".into(),
                json!({
                    "tool_call_id": tool_call_id,
                    "peer_id": peer.peer_id,
                    "peer_kind": peer.peer_kind,
                    "presence_state": peer.presence_state,
                }),
            ));
        }
        None => outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "peer.describe",
            &format!("peer_id '{peer_id}' not found in current context snapshot"),
        )),
    }
    true
}

pub(super) fn handle_daemon_ensure_peer(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let peer_kind = read_string(arguments, "peer_kind").unwrap_or_else(|| "agent".into());
    let peer_id =
        read_string(arguments, "peer_id").unwrap_or_else(|| format!("peer-{peer_kind}-local"));
    let lease_ttl_ms = read_u64(arguments, "lease_ttl_ms").unwrap_or(60_000);

    let runtime_home = match runtime_home_from_context(input.context) {
        Some(path) => path,
        None => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "daemon.ensure_peer",
                "missing context.project.runtime_home, cannot persist lease/heartbeat state",
            ));
            return true;
        }
    };

    let lease_path = runtime_home.join(format!("runtime/peers/leases/{peer_id}.json"));
    let state_path = runtime_home.join(format!("runtime/peers/state/{peer_id}.json"));
    let request_path = runtime_home.join(format!("runtime/peers/ensure_requests/{peer_id}.json"));
    let request_index_path = runtime_home.join("runtime/peers/ensure_requests.json");
    let lease = PeerLeaseRecord {
        peer_id: peer_id.clone(),
        peer_kind: peer_kind.clone(),
        lease_ttl_ms,
        issued_at: input.occurred_at.to_string(),
        expires_at_hint_ms: lease_ttl_ms,
        owner: input
            .refs
            .worker_id
            .clone()
            .unwrap_or_else(|| "worker-unknown".into()),
    };
    let state = PeerStateRecord {
        peer_id: peer_id.clone(),
        peer_kind: peer_kind.clone(),
        lifecycle_state: "online".into(),
        last_heartbeat_at: input.occurred_at.to_string(),
        reconnect_backoff_ms: 0,
    };
    let project_id = read_string(arguments, "project_id").or_else(|| {
        input.context.project.as_ref().and_then(|project| {
            project
                .primary_project
                .as_ref()
                .map(|project| project.project_id.clone())
        })
    });
    let request = DaemonEnsurePeerRequestRecord {
        request_id: format!("ensure-{peer_id}-{}", sanitize_id(input.occurred_at)),
        peer_id: peer_id.clone(),
        peer_kind: peer_kind.clone(),
        project_id: project_id.clone(),
        agent_name: read_string(arguments, "agent_name"),
        mode_hint: read_string(arguments, "mode").or_else(|| read_string(arguments, "mode_hint")),
        project_root: read_string(arguments, "project_root"),
        endpoint: read_string(arguments, "endpoint"),
        lease_ttl_ms,
        requested_at: input.occurred_at.to_string(),
        requested_by_worker_id: input.refs.worker_id.clone(),
        status: "pending".into(),
        consumed_at: None,
    };

    if let Err(err) = write_json(&lease_path, &lease)
        .and_then(|_| write_json(&state_path, &state))
        .and_then(|_| write_json(&request_path, &request))
        .and_then(|_| upsert_ensure_request(&request_index_path, &request))
    {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "daemon.ensure_peer",
            &format!("failed to persist peer lifecycle state: {err}"),
        ));
        return true;
    }

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "daemon.ensure_peer".into(),
        tool_kind: "agent_tool".into(),
        title: "Daemon Ensure Peer".into(),
        purpose: "ensure peer lifecycle lease + heartbeat state for local/remote execution chain"
            .into(),
        target_kind: Some("daemon".into()),
        target_ref: Some(peer_id.clone()),
        input_summary: Some(format!(
            "peer_kind={peer_kind}, peer_id={peer_id}, lease_ttl_ms={lease_ttl_ms}"
        )),
        output_summary: Some("lease + heartbeat persisted".into()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec![
            "write_peer_lease".into(),
            "write_peer_heartbeat".into(),
            "enable_reconnect_supervision".into(),
        ],
        artifact_refs: vec![
            relative_artifact(input.context, &lease_path),
            relative_artifact(input.context, &state_path),
            relative_artifact(input.context, &request_path),
            relative_artifact(input.context, &request_index_path),
        ],
        error_summary: None,
    });
    outcome.events.push((
        "daemon.ensure_peer_requested".into(),
        json!({
            "tool_call_id": tool_call_id,
            "request_id": request.request_id,
            "peer_kind": peer_kind,
            "peer_id": peer_id,
            "project_id": project_id,
            "lease_ttl_ms": lease_ttl_ms,
            "placeholder": false,
        }),
    ));
    outcome.events.push((
        "peer.lease_opened".into(),
        json!({
            "peer_id": peer_id,
            "lease_ttl_ms": lease_ttl_ms,
        }),
    ));
    outcome.events.push((
        "peer.heartbeat_recorded".into(),
        json!({
            "peer_id": state.peer_id,
            "heartbeat_at": state.last_heartbeat_at,
        }),
    ));
    true
}

fn upsert_ensure_request(
    path: &Path,
    request: &DaemonEnsurePeerRequestRecord,
) -> Result<(), String> {
    let mut requests = match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<Vec<DaemonEnsurePeerRequestRecord>>(&content)
            .map_err(|err| err.to_string())?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.to_string()),
    };
    if let Some(existing) = requests
        .iter_mut()
        .find(|item| item.peer_id == request.peer_id)
    {
        *existing = request.clone();
    } else {
        requests.push(request.clone());
    }
    write_json(path, &requests)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn relative_artifact(context: &fin_contracts::MinimalContextView, absolute: &Path) -> String {
    runtime_home_from_context(context)
        .and_then(|home| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}
