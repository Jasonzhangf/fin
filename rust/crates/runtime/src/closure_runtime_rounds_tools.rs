use super::*;
use serde_json::json;
use fin_provider::{ProviderToolSpec, ProviderToolCall, ProviderToolResult};
use fin_contracts::ToolCatalogEntry;
use crate::model_output::ModelToolCall;
use std::path::PathBuf;
use std::fs;

pub(crate) fn build_provider_tool_specs(context: &MinimalContextView) -> Vec<ProviderToolSpec> {
    context
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .model_tools
                .iter()
                .map(|tool| ProviderToolSpec {
                    name: tool.tool_name.clone(),
                    description: build_tool_description(tool),
                    input_schema: build_tool_input_schema(tool),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn build_tool_input_schema(tool: &ToolCatalogEntry) -> serde_json::Value {
    match tool.tool_name.as_str() {
        "write_file" => object_schema(
            tool,
            json!({
                "path": {
                    "type": "string",
                    "description": "workspace-relative file path to create or overwrite"
                },
                "content": {
                    "type": "string",
                    "description": "complete file content to write"
                }
            }),
            &["path", "content"],
        ),
        "apply_patch" => {
            let mut schema = object_schema(
                tool,
                json!({
                    "mode": {
                        "type": "string",
                        "enum": ["replace", "patch"],
                        "description": "replace for exact fragment/file edits; patch for V4A patch text"
                    },
                    "path": {
                        "type": "string",
                        "description": "target file path for replace mode"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "exact text to replace in replace mode"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "replacement text in replace mode"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "replace all matching occurrences in replace mode"
                    },
                    "patch": {
                        "type": "string",
                        "description": "V4A patch text for patch mode"
                    }
                }),
                &["mode"],
            );
            schema["anyOf"] = json!([
                {
                    "properties": { "mode": { "const": "replace" } },
                    "required": ["mode", "path", "old_string", "new_string"]
                },
                {
                    "properties": { "mode": { "const": "patch" } },
                    "required": ["mode", "patch"]
                }
            ]);
            schema
        }
        "exec_command" => object_schema(
            tool,
            json!({
                "cmd": {
                    "type": "string",
                    "description": "shell command to execute"
                },
                "cwd": {
                    "type": "string",
                    "description": "optional working directory"
                }
            }),
            &["cmd"],
        ),
        "write_stdin" => object_schema(
            tool,
            json!({
                "session_id": {
                    "type": "string",
                    "description": "interactive exec session id"
                },
                "chars": {
                    "type": "string",
                    "description": "stdin content to send; may be empty when polling"
                }
            }),
            &["session_id"],
        ),
        "project.task.status" => object_schema(
            tool,
            json!({
                "task_id": {
                    "type": "string",
                    "description": "managed task id to inspect"
                }
            }),
            &["task_id"],
        ),
        "project.task.list" => object_schema(
            tool,
            json!({
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "maximum number of task ids to return"
                }
            }),
            &[],
        ),
        "project.task.create" => object_schema(
            tool,
            json!({
                "title": {
                    "type": "string",
                    "description": "concrete managed task title"
                },
                "summary": {
                    "type": "string",
                    "description": "short task summary"
                },
                "status": {
                    "type": "string",
                    "description": "initial task status; defaults to ready"
                },
                "task_id": {
                    "type": "string",
                    "description": "optional explicit task id"
                },
                "epic_id": {
                    "type": "string",
                    "description": "optional parent epic id"
                },
                "review_owner_worker_id": {
                    "type": "string",
                    "description": "worker id that will review submissions"
                }
            }),
            &["title"],
        ),
        "project.task.update" => object_schema(
            tool,
            json!({
                "task_id": {
                    "type": "string",
                    "description": "managed task id to update"
                },
                "title": {
                    "type": "string",
                    "description": "new task title"
                },
                "summary": {
                    "type": "string",
                    "description": "new task summary"
                },
                "status": {
                    "type": "string",
                    "description": "new status (ready/cancelled/blocked/etc.)"
                },
                "epic_id": {
                    "type": "string",
                    "description": "parent epic id"
                },
                "review_owner_worker_id": {
                    "type": "string",
                    "description": "review owner worker id"
                }
            }),
            &["task_id"],
        ),
        "project.task.claim" => object_schema(
            tool,
            json!({
                "task_id": {
                    "type": "string",
                    "description": "managed task id to claim"
                },
                "worker_id": {
                    "type": "string",
                    "description": "worker id claiming the task"
                },
                "status": {
                    "type": "string",
                    "description": "optional status override; defaults to claimed"
                }
            }),
            &["task_id"],
        ),
        "project.task.submit" => object_schema(
            tool,
            json!({
                "task_id": {
                    "type": "string",
                    "description": "managed task id being submitted"
                },
                "result_summary": {
                    "type": "string",
                    "description": "result summary for owner review"
                },
                "artifact_refs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "artifact refs that support the result"
                }
            }),
            &["task_id", "result_summary"],
        ),
        "project.task.review" => object_schema(
            tool,
            json!({
                "task_id": {
                    "type": "string",
                    "description": "managed task id being reviewed"
                },
                "decision": {
                    "type": "string",
                    "enum": ["approve", "reopen", "block", "cancel"],
                    "description": "owner review decision"
                },
                "review_summary": {
                    "type": "string",
                    "description": "optional review rationale"
                }
            }),
            &["task_id", "decision"],
        ),
        "agent.assign" => {
            let mut schema = object_schema(
                tool,
                json!({
                    "target_worker_id": {
                        "type": "string",
                        "description": "worker id that should receive the assignment"
                    },
                    "peer_id": {
                        "type": "string",
                        "description": "peer id alternative when target_worker_id is unavailable"
                    },
                    "target_peer_id": {
                        "type": "string",
                        "description": "legacy alias for peer_id"
                    },
                    "task_summary": {
                        "type": "string",
                        "description": "clear execution instruction for the worker"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "optional managed task id to bind"
                    }
                }),
                &["task_summary"],
            );
            schema["anyOf"] = json!([
                { "required": ["target_worker_id"] },
                { "required": ["peer_id"] },
                { "required": ["target_peer_id"] }
            ]);
            schema
        }
        "update_plan" => object_schema(
            tool,
            json!({
                "explanation": {
                    "type": "string",
                    "description": "optional progress explanation"
                },
                "steps": {
                    "type": "array",
                    "minItems": 1,
                    "description": "ordered plan items",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "step": {
                                "type": "string",
                                "description": "step summary"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "plan step status"
                            }
                        },
                        "required": ["step", "status"]
                    }
                }
            }),
            &["steps"],
        ),
        _ => object_schema(tool, json!({}), &[]),
    }
}

fn object_schema(
    tool: &ToolCatalogEntry,
    properties: serde_json::Value,
    required: &[&str],
) -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "description": tool.input_schema_summary,
        "properties": properties,
        "required": required,
    })
}

fn build_tool_description(tool: &ToolCatalogEntry) -> String {
    let mut parts = Vec::new();
    if !tool.summary.trim().is_empty() {
        parts.push(tool.summary.trim().to_string());
    }
    if !tool.purpose.trim().is_empty() {
        parts.push(format!("purpose: {}", tool.purpose.trim()));
    }
    if !tool.when_to_use.is_empty() {
        parts.push(format!("use: {}", tool.when_to_use.join(" | ")));
    }
    if !tool.when_not_to_use.is_empty() {
        parts.push(format!("avoid: {}", tool.when_not_to_use.join(" | ")));
    }
    if !tool.output_schema_summary.trim().is_empty() {
        parts.push(format!("returns: {}", tool.output_schema_summary.trim()));
    }
    if !tool.example_uses.is_empty() {
        parts.push(format!("examples: {}", tool.example_uses.join(" | ")));
    }
    parts.join("\n")
}

pub(crate) fn model_tool_call_to_provider_tool_call(call: &ModelToolCall) -> ProviderToolCall {
    ProviderToolCall {
        tool_call_id: call
            .tool_call_id
            .clone()
            .unwrap_or_else(|| format!("tool-call-{}", call.tool_name)),
        name: call.tool_name.clone(),
        arguments: call.arguments.clone(),
    }
}

pub(crate) fn build_provider_tool_results(
    context: &MinimalContextView,
    tool_results: &[ToolExecutionRecord],
) -> Vec<ProviderToolResult> {
    tool_results
        .iter()
        .map(|record| ProviderToolResult {
            tool_call_id: record.tool_call_id.clone(),
            name: record.tool_name.clone(),
            content: build_tool_result_content(context, record),
            is_error: record.status != "completed",
        })
        .collect()
}

fn build_tool_result_content(context: &MinimalContextView, record: &ToolExecutionRecord) -> String {
    if let Some(receipt) = load_authoritative_receipt(context, record) {
        return receipt;
    }
    let payload = json!({
        "tool_call_id": record.tool_call_id,
        "tool_name": record.tool_name,
        "status": record.status,
        "title": record.title,
        "purpose": record.purpose,
        "target_kind": record.target_kind,
        "target_ref": record.target_ref,
        "input_summary": record.input_summary,
        "output_summary": record.output_summary,
        "error_summary": record.error_summary,
        "artifact_refs": record.artifact_refs,
        "side_effects": record.side_effects,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| record.tool_name.clone())
}

fn load_authoritative_receipt(
    context: &MinimalContextView,
    record: &ToolExecutionRecord,
) -> Option<String> {
    record.artifact_refs.iter().find_map(|artifact_ref| {
        if !is_authoritative_receipt_ref(artifact_ref) {
            return None;
        }
        resolve_artifact_path(context, artifact_ref).and_then(|path| fs::read_to_string(path).ok())
    })
}

fn is_authoritative_receipt_ref(artifact_ref: &str) -> bool {
    crate::tools::tool_dispatch::authoritative_receipt_ref(artifact_ref)
}

fn resolve_artifact_path(context: &MinimalContextView, artifact_ref: &str) -> Option<PathBuf> {
    let as_path = PathBuf::from(artifact_ref);
    if as_path.is_absolute() {
        return Some(as_path);
    }
    context
        .project
        .as_ref()
        .and_then(|project| project.runtime_home.as_deref())
        .map(PathBuf::from)
        .map(|runtime_home| runtime_home.join(artifact_ref))
}

#[cfg(test)]
mod tests {
    use super::*;
use serde_json::json;
use fin_provider::{ProviderToolSpec, ProviderToolCall, ProviderToolResult};
use fin_contracts::ToolCatalogEntry;
use crate::model_output::ModelToolCall;
use std::path::PathBuf;
use std::fs;
    use crate::tools::tool_catalog::build_tool_catalog_block;

    #[test]
    fn provider_tool_specs_include_required_fields_for_managed_task_tools() {
        let context = MinimalContextView {
            tools: Some(build_tool_catalog_block()),
            ..MinimalContextView::default()
        };
        let specs = build_provider_tool_specs(&context);

        let create = specs
            .iter()
            .find(|tool| tool.name == "project.task.create")
            .expect("project.task.create schema");
        assert_eq!(create.input_schema["type"], "object");
        assert_eq!(create.input_schema["properties"]["title"]["type"], "string");
        assert_eq!(create.input_schema["required"], json!(["title"]));

        let status = specs
            .iter()
            .find(|tool| tool.name == "project.task.status")
            .expect("project.task.status schema");
        assert_eq!(status.input_schema["required"], json!(["task_id"]));

        let assign = specs
            .iter()
            .find(|tool| tool.name == "agent.assign")
            .expect("agent.assign schema");
        assert_eq!(assign.input_schema["required"], json!(["task_summary"]));
        assert!(assign.input_schema["anyOf"].is_array());

        let update_plan = specs
            .iter()
            .find(|tool| tool.name == "update_plan")
            .expect("update_plan schema");
        assert_eq!(update_plan.input_schema["required"], json!(["steps"]));
        assert_eq!(
            update_plan.input_schema["properties"]["steps"]["items"]["required"],
            json!(["step", "status"])
        );

        let exec_command = specs
            .iter()
            .find(|tool| tool.name == "exec_command")
            .expect("exec_command schema");
        assert_eq!(exec_command.input_schema["required"], json!(["cmd"]));
    }
}
