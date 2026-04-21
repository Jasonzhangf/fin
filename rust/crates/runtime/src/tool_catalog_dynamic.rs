use crate::tool_catalog::build_tool_catalog_block;
use fin_contracts::{MinimalContextView, ToolCatalogBlock, ToolCatalogEntry, ToolExecutionRecord};
use std::fs;

pub(super) struct DynamicToolCatalogInput<'a> {
    pub(super) role_id: Option<&'a str>,
    pub(super) context: &'a MinimalContextView,
    pub(super) recent_tool_records: &'a [ToolExecutionRecord],
    pub(super) round_index: u32,
}

pub(super) fn build_dynamic_tool_catalog_block(
    input: &DynamicToolCatalogInput<'_>,
) -> ToolCatalogBlock {
    let mut block = build_tool_catalog_block();
    let has_runtime_home = input
        .context
        .project
        .as_ref()
        .and_then(|project| project.runtime_home.as_deref())
        .is_some_and(|value| !value.trim().is_empty());
    let has_project_scope = input.context.project.as_ref().is_some_and(|project| {
        project
            .cwd
            .as_deref()
            .or(project.project_root.as_deref())
            .is_some_and(|value| !value.trim().is_empty())
    });
    let has_peer_context = input
        .context
        .peer
        .as_ref()
        .is_some_and(|peer| !peer.peers.is_empty());
    let has_capabilities = input.context.peer.as_ref().is_some_and(|peer| {
        peer.peers
            .iter()
            .any(|item| !item.capability_ids.is_empty())
    });
    let has_exec_session = has_exec_session(input.context, input.recent_tool_records);
    let recent_model_tool_count = input
        .recent_tool_records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .count();
    let role_id = input.role_id.unwrap_or("project");

    for tool in &mut block.model_tools {
        match tool.tool_name.as_str() {
            "apply_patch" => {
                if !has_project_scope {
                    add_avoid(
                        tool,
                        "no project cwd/project_root is available in current context",
                    );
                }
            }
            "context_history.rebuild"
            | "project.task.status"
            | "project.task.list"
            | "project.task.create"
            | "project.task.claim"
            | "project.task.submit"
            | "project.task.review"
            | "agent.presence.list"
            | "project.supervision.list"
            | "update_plan"
            | "session.list" => {
                if !has_runtime_home {
                    add_avoid(
                        tool,
                        "current context has no runtime_home, so persistence-backed artifacts are unavailable",
                    );
                }
            }
            "write_stdin" => {
                if !has_runtime_home {
                    add_avoid(
                        tool,
                        "current context has no runtime_home, so exec replay sessions cannot be resolved",
                    );
                }
                if has_exec_session {
                    add_use(
                        tool,
                        "an exec replay session is available, so you can continue it with session_id + chars",
                    );
                } else {
                    add_avoid(
                        tool,
                        "no exec replay session is currently known; create one with exec_command(open_stdin_session=true) first",
                    );
                }
            }
            "peer.list" | "peer.describe" | "daemon.ensure_peer" | "agent.assign"
            | "mailbox.send" | "mailbox.poll" => {
                if !has_peer_context {
                    add_avoid(
                        tool,
                        "current peer context has no known peer descriptors, so peer routing facts are limited",
                    );
                }
            }
            "capability.invoke" => {
                if !has_peer_context || !has_capabilities {
                    add_avoid(
                        tool,
                        "no capability ids are present in current peer descriptors",
                    );
                }
            }
            _ => {}
        }
    }
    apply_role_tool_bias(&mut block, role_id);
    apply_owner_loop_bias(&mut block, input.context, role_id);

    if input.round_index > 1 {
        block.tool_selection_policy.push(format!(
            "round {} is an auto-tool follow-up; first inspect current Recent tool activity and executed tool results before choosing more tools",
            input.round_index
        ));
    }
    if recent_model_tool_count > 0 {
        block.tool_selection_policy.push(format!(
            "{} model-executed tool result(s) already exist in this turn; treat them as authoritative client-side facts and avoid repeating the same call without a new reason",
            recent_model_tool_count
        ));
    }
    if has_exec_session {
        block.tool_selection_policy.push(
            "write_stdin is actionable in the current context because an exec replay session exists"
                .into(),
        );
    } else {
        block.tool_selection_policy.push(
            "write_stdin usually follows exec_command(open_stdin_session=true); without session_id it will fail"
                .into(),
        );
    }

    block
}

fn apply_role_tool_bias(block: &mut ToolCatalogBlock, role_id: &str) {
    match role_id {
        "system" => {
            block.tool_selection_policy.push(
                "system role uses the same runtime/tooling foundation as project role, but should prefer framework-owned backlog/task-board state, agent presence, supervision, peer visibility, dispatch, review, and unblock decisions before deep local execution"
                    .into(),
            );
            for tool_name in [
                "peer.list",
                "peer.describe",
                "daemon.ensure_peer",
                "mailbox.send",
                "mailbox.poll",
                "agent.assign",
                "capability.invoke",
                "update_plan",
                "project.task.status",
                "project.task.list",
                "project.task.create",
                "project.task.review",
                "agent.presence.list",
                "project.supervision.list",
                "session.list",
            ] {
                if let Some(tool) = find_tool_mut(block, tool_name) {
                    add_use(
                        tool,
                        "system role: use this when routing, dispatch, delegation, peer visibility, review, or coordination state is the primary need",
                    );
                }
            }
        }
        _ => {
            block.tool_selection_policy.push(
                "project role uses the same runtime/tooling foundation as system role, but should prefer project-scoped closure, task-board execution management, worker dispatch, review, and delivery progress inside the same project role"
                    .into(),
            );
            for tool_name in [
                "agent.assign",
                "update_plan",
                "apply_patch",
                "context_history.rebuild",
                "project.task.status",
                "project.task.list",
                "project.task.create",
                "project.task.claim",
                "project.task.submit",
                "project.task.review",
                "exec_command",
            ] {
                if let Some(tool) = find_tool_mut(block, tool_name) {
                    add_use(
                        tool,
                        "project role: use this when it advances the current project task board toward a minimal verified closure",
                    );
                }
            }
        }
    }
}

fn apply_owner_loop_bias(
    block: &mut ToolCatalogBlock,
    context: &MinimalContextView,
    role_id: &str,
) {
    let Some(project) = context.project.as_ref() else {
        return;
    };
    if !project.submitted_task_ids.is_empty() {
        block.tool_selection_policy.push(format!(
            "owner-loop truth: submitted managed tasks exist [{}]; review these before dispatching more work or doing deep local execution",
            project.submitted_task_ids.join(", ")
        ));
        if let Some(tool) = find_tool_mut(block, "project.task.review") {
            add_use(
                tool,
                "owner-loop truth: submitted managed tasks are waiting for review owner decision",
            );
        }
        if let Some(tool) = find_tool_mut(block, "project.task.status") {
            add_use(
                tool,
                "owner-loop truth: inspect submitted task evidence before approve/reopen/block",
            );
        }
        return;
    }
    if !project.ready_task_ids.is_empty() {
        block.tool_selection_policy.push(format!(
            "owner-loop truth: ready unclaimed managed tasks exist [{}]; dispatch or claim them before inventing new managed work",
            project.ready_task_ids.join(", ")
        ));
        if let Some(tool) = find_tool_mut(block, "project.task.claim") {
            add_use(
                tool,
                "owner-loop truth: a ready unclaimed managed task is available for execution",
            );
        }
        if let Some(tool) = find_tool_mut(block, "agent.assign") {
            add_use(
                tool,
                "owner-loop truth: dispatch a ready managed task to an available worker when resources allow",
            );
        }
        if role_id == "system" {
            if let Some(tool) = find_tool_mut(block, "project.task.list") {
                add_use(
                    tool,
                    "owner-loop truth: system role should inspect ready tasks before prioritizing new requests in isolation",
                );
            }
        }
    }
}

fn find_tool_mut<'a>(
    block: &'a mut ToolCatalogBlock,
    tool_name: &str,
) -> Option<&'a mut ToolCatalogEntry> {
    block
        .model_tools
        .iter_mut()
        .find(|tool| tool.tool_name == tool_name)
}

fn add_use(tool: &mut ToolCatalogEntry, value: &str) {
    if !tool.when_to_use.iter().any(|item| item == value) {
        tool.when_to_use.push(value.to_string());
    }
}

fn add_avoid(tool: &mut ToolCatalogEntry, value: &str) {
    if !tool.when_not_to_use.iter().any(|item| item == value) {
        tool.when_not_to_use.push(value.to_string());
    }
}

fn has_exec_session(context: &MinimalContextView, records: &[ToolExecutionRecord]) -> bool {
    if records.iter().any(|record| {
        record.tool_name == "exec_command"
            && record.status == "completed"
            && (record
                .output_summary
                .as_deref()
                .is_some_and(|value| value.contains("session_id="))
                || record
                    .artifact_refs
                    .iter()
                    .any(|value| value.contains("exec_sessions/")))
    }) {
        return true;
    }

    let Some(runtime_home) = context
        .project
        .as_ref()
        .and_then(|project| project.runtime_home.as_deref())
        .filter(|value| !value.trim().is_empty())
    else {
        return false;
    };
    let path = std::path::Path::new(runtime_home).join("runtime/tools/exec_sessions");
    fs::read_dir(path)
        .ok()
        .is_some_and(|mut entries| entries.next().is_some())
}
