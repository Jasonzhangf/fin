use crate::tool_catalog_task_tools::build_project_task_model_tools;
use fin_contracts::{ToolCatalogBlock, ToolCatalogEntry};

pub(super) fn build_tool_catalog_block() -> ToolCatalogBlock {
    ToolCatalogBlock {
        model_tools: vec![
            model_tool(
                "update_plan",
                "persist one structured execution plan update",
                "record step/status progress as durable plan truth for the current runtime and session",
                vec![
                    "the task needs an explicit step plan or a plan status refresh".into(),
                    "you want progress to persist structured steps instead of only free-form notes"
                        .into(),
                ],
                vec![
                    "the task is a one-shot simple answer with no concrete steps".into(),
                    "you do not yet know the actual steps or statuses".into(),
                ],
                "steps[{step,status}] + optional explanation".into(),
                "plan update receipt + persisted current plan artifact".into(),
                vec![
                    "writes current plan artifact under runtime/session truth".into(),
                    "emits structured plan update event".into(),
                ],
                vec!["record plan: inspect code -> patch runtime -> run tests".into()],
            ),
            model_tool(
                "session.list",
                "list recently known sessions under the current runtime home",
                "inspect reusable sessions before deciding whether to resume, compare, or report previous work threads",
                vec![
                    "you need to discover existing sessions or recent session ids".into(),
                    "the user asks whether there is prior work history or resumable threads"
                        .into(),
                ],
                vec![
                    "the current turn already has the exact target session id".into(),
                    "session discovery is irrelevant to the current task".into(),
                ],
                "optional limit".into(),
                "recent session ids + short summary count".into(),
                vec!["reads runtime_home/sessions directory".into()],
                vec!["list the latest 10 sessions before deciding a resume strategy".into()],
            ),
            model_tool(
                "peer.list",
                "list peer descriptors available to the current runtime context",
                "inspect peer topology and basic presence before routing decisions",
                vec![
                    "a routing decision needs current peer topology visibility".into(),
                    "system/peer-router wants a peer inventory snapshot".into(),
                ],
                vec![
                    "the current turn can be resolved locally without any peer awareness".into(),
                    "you need live remote handshake truth; this M1 tool only provides placeholder context truth".into(),
                ],
                "optional limit + optional kind filter".into(),
                "peer descriptor list with kind/presence/binding support flags".into(),
                vec!["reads current peer context snapshot".into()],
                vec!["enumerate peers before selecting route target".into()],
            ),
            model_tool(
                "peer.describe",
                "describe one peer descriptor in detail",
                "inspect one peer for routeability, capability hints, and binding support",
                vec![
                    "a specific peer id has been selected and needs deeper inspection".into(),
                    "you need to verify whether peer supports session binding or agentic execution"
                        .into(),
                ],
                vec![
                    "no peer id is known yet; use peer.list first".into(),
                    "you need real-time remote health probe; this is context-snapshot level only in M1"
                        .into(),
                ],
                "peer_id".into(),
                "single peer descriptor + local binding/daemon hints".into(),
                vec!["reads one peer descriptor from current context snapshot".into()],
                vec!["inspect candidate peer before assignment".into()],
            ),
            model_tool(
                "daemon.ensure_peer",
                "request daemon supervision to ensure a local peer is present",
                "prepare the lifecycle layer for a required peer before dispatch",
                vec![
                    "routing requires a local project/agent peer that may need supervision".into(),
                    "system role wants explicit daemon-level ensure intent in the event chain".into(),
                ],
                vec![
                    "you expect a real spawn/restart side effect; current M1 skeleton is placeholder-only".into(),
                    "you only need passive topology read; use peer.list/peer.describe instead".into(),
                ],
                "peer_kind + optional peer_id".into(),
                "ensure receipt (placeholder in current M1)".into(),
                vec!["records daemon ensure intent as structured runtime fact".into()],
                vec!["request daemon ensure before project-agent assignment".into()],
            ),
            model_tool(
                "wait.remind",
                "schedule async wait + system self reminder",
                "avoid blocking turns when waiting more than one minute; let framework wake system role later",
                vec![
                    "you need to wait for external state and expected wait exceeds one minute".into(),
                    "you want a deterministic self reminder to continue the next reasoning cycle".into(),
                ],
                vec![
                    "you can continue immediately without waiting".into(),
                    "you only need a short sub-minute pause".into(),
                ],
                "wait_minutes + reminder".into(),
                "reminder schedule receipt for role=system self wakeup".into(),
                vec![
                    "writes pending reminder artifact for async wakeup".into(),
                    "injects due reminder as system message before next turn".into(),
                ],
                vec![
                    "wait 5 minutes for CI result then remind system to review logs".into(),
                ],
            ),
            model_tool(
                "reasoning.stop",
                "explicitly close the current reasoning cycle",
                "signal that this turn can stop; runtime closure should be decided by this tool call rather than provider finish_reason",
                vec![
                    "you have enough evidence and the current turn should terminate".into(),
                    "all required tool work for this turn is completed".into(),
                ],
                vec![
                    "you still need additional tool calls or verification in this turn".into(),
                ],
                "summary (optional)".into(),
                "explicit stop receipt".into(),
                vec!["emits reasoning.stopped event".into()],
                vec!["after finishing analysis, call reasoning.stop with concise summary".into()],
            ),
            model_tool(
                "apply_patch",
                "apply deterministic workspace file edits",
                "edit one or more workspace files using Hermes-style replace mode or V4A patch text",
                vec![
                    "you need to modify files deterministically instead of only describing edits".into(),
                    "you already know the exact old/new text or patch content to apply".into(),
                ],
                vec![
                    "you are still exploring and do not know the concrete change yet".into(),
                    "the edit target is outside the current project/workspace scope".into(),
                ],
                "mode=replace: path + old_string + new_string + replace_all? (use old_string=\"\" to create a new file) ; mode=patch: patch"
                    .into(),
                "patch receipt + modified file refs".into(),
                vec![
                    "writes workspace file content".into(),
                    "stores patch receipt under runtime_home when available".into(),
                ],
                vec![
                    "replace one exact function body in src/runtime.rs".into(),
                    "create a new file with mode=replace using old_string=\"\" and the full new_string body".into(),
                    "apply a multi-file V4A patch for a small deterministic refactor".into(),
                ],
            ),
            model_tool(
                "view_image",
                "inspect one image attachment or local image file reference",
                "read image attachment metadata and local file facts so the turn can reference the correct image object without guessing",
                vec![
                    "the current turn includes an image attachment or local image path you need to inspect".into(),
                    "you need the exact attachment/path/size/dimensions metadata before responding or delegating".into(),
                ],
                vec![
                    "the turn has no image attachment and no concrete local image path".into(),
                    "you need full vision reasoning over pixels; M1 only exposes image reference metadata".into(),
                ],
                "optional path or attachment identifier; defaults to first image attachment in current input".into(),
                "image metadata receipt + referenced artifact path/url".into(),
                vec![
                    "reads current attachment metadata".into(),
                    "may stat a local image file in workspace scope".into(),
                ],
                vec!["inspect the first uploaded screenshot before deciding the next tool".into()],
            ),
            model_tool(
                "context_history.rebuild",
                "refresh context rebuild index from current session artifacts",
                "recompute rebuild receipt/counts from current context artifacts when you need explicit continuity bookkeeping without a provider round-trip",
                vec![
                    "you need the latest rebuild receipt/counts for the active session".into(),
                    "you want framework-visible continuity bookkeeping after a materialized context refresh".into(),
                ],
                vec![
                    "the active session has no current context artifact yet".into(),
                    "you are trying to ask the model to summarize history manually instead of using durable artifacts".into(),
                ],
                "optional reason".into(),
                "rebuild receipt with recent context/digest/reasoning/tool counts".into(),
                vec![
                    "writes runtime/session rebuild-index artifacts".into(),
                    "refreshes recent_contexts index from current context artifact".into(),
                ],
                vec!["refresh rebuild bookkeeping after a context maintenance step".into()],
            ),
            model_tool(
                "exec_command",
                "execute one bounded shell command",
                "run a local command for discovery or validation and capture stdout/stderr summary",
                vec![
                    "you need concrete local command evidence for the current turn".into(),
                ],
                vec![
                    "a framework/internal artifact already provides the same fact".into(),
                ],
                "cmd (+ optional cwd)".into(),
                "exit code + stdout/stderr summary".into(),
                vec!["local subprocess execution".into()],
                vec!["run `ls` or `rg` to verify files before concluding".into()],
            ),
            model_tool(
                "write_stdin",
                "send stdin to an interactive command session",
                "continue an existing interactive execution session",
                vec![
                    "an interactive session id already exists and needs input".into(),
                ],
                vec![
                    "no interactive session exists; use exec_command instead".into(),
                ],
                "session_id + chars".into(),
                "stdin write receipt".into(),
                vec!["interactive process stdin write".into()],
                vec!["send follow-up input to a running REPL command".into()],
            ),
            model_tool(
                "mailbox.send",
                "enqueue one collaboration message",
                "send one structured note/request through the framework-owned agent mailbox",
                vec!["you need asynchronous peer collaboration handoff".into()],
                vec!["the task can be finished locally in this turn".into()],
                "target_peer_id or target_worker_id + message".into(),
                "mailbox message id receipt".into(),
                vec!["appends one durable agent mailbox message".into()],
                vec![
                    "send blocker details to project leader mailbox".into(),
                    "send build slice result to worker-b mailbox via target_worker_id".into(),
                ],
            ),
            model_tool(
                "mailbox.poll",
                "read pending collaboration messages",
                "read pending messages from the framework-owned agent mailbox",
                vec!["you need latest async collaboration updates".into()],
                vec!["no mailbox sync is needed for this turn".into()],
                "optional peer_id or worker_id + optional limit".into(),
                "pending message list".into(),
                vec!["reads durable agent mailbox messages".into()],
                vec![
                    "poll mailbox before deciding next delegation step".into(),
                    "poll worker-b mailbox after one delegated slice completes".into(),
                ],
            ),
            model_tool(
                "agent.assign",
                "request one project-worker assignment",
                "assign a bounded subtask to an agent peer or one project worker runtime",
                vec!["a bounded subtask should be delegated to another peer".into()],
                vec!["the task requires immediate local execution".into()],
                "peer_id or target_worker_id + task_summary".into(),
                "assignment receipt".into(),
                vec!["emits assignment intent event".into()],
                vec!["assign log validation to worker-b via target_worker_id".into()],
            ),
            model_tool(
                "capability.invoke",
                "invoke one capability peer endpoint",
                "call a non-agent capability peer using standardized capability contract",
                vec!["a capability peer can provide required data/action".into()],
                vec!["no capability peer path is available in current topology".into()],
                "capability_id + input".into(),
                "capability response summary".into(),
                vec!["emits capability invoke intent/response events".into()],
                vec!["invoke metadata capability from remote service peer".into()],
            ),
        ]
        .into_iter()
        .chain(build_project_task_model_tools())
        .collect(),
        framework_tools: vec![
            framework_tool(
                "provider.call",
                "framework-owned provider execution boundary",
                "dispatch the compiled prompt to the configured provider and normalize the response",
                vec!["the framework has already assembled context and needs a provider round-trip".into()],
                vec!["the model is deciding whether it should call a tool itself".into()],
                "no model-visible input schema; framework passes compiled prompt + provider path".into(),
                "normalized provider response event + sanitized debug snapshot".into(),
                vec!["network request to external provider".into()],
                vec!["framework auto-runs provider.call after inference operation acceptance".into()],
            ),
            framework_tool(
                "session.materialize",
                "session artifact write + revision advance",
                "persist messages, contexts, digests, and revision pointers to session truth",
                vec!["a closure has produced artifacts that must become channel render truth".into()],
                vec!["the model wants to directly write UI-visible state".into()],
                "framework-owned session artifacts bundle".into(),
                "updated session files + revision advance".into(),
                vec!["writes session files under ~/.fin/sessions".into()],
                vec!["framework materializes session artifacts before Web reads them".into()],
            ),
            framework_tool(
                "event.append",
                "append-only runtime fact recording",
                "record operation lifecycle and provider facts as immutable events",
                vec!["runtime state changes or side effects must become facts".into()],
                vec!["a channel only needs a projection refresh".into()],
                "structured event payload".into(),
                "event row appended to stream.jsonl".into(),
                vec!["appends raw event data to session/runtime event streams".into()],
                vec!["framework emits started/completed/failed events automatically".into()],
            ),
            framework_tool(
                "progress.update",
                "structured progress snapshot emission",
                "publish the current execution phase, health hint, and next step",
                vec!["execution phase changes and observers need a new progress snapshot".into()],
                vec!["nothing changed in execution state".into()],
                "progress block".into(),
                "latest progress snapshot".into(),
                vec!["updates progress/latest.json".into()],
                vec!["framework updates progress after provider completion".into()],
            ),
            framework_tool(
                "execution_note.append",
                "framework note persistence for ongoing execution",
                "persist concise execution note for later digest merge and inspection",
                vec!["a closure or step yields a durable lesson / decision / next step".into()],
                vec!["the content is only transient chain-of-thought".into()],
                "execution note block".into(),
                "notes/latest.json + note refs".into(),
                vec!["writes note artifact visible to debug tools".into()],
                vec!["framework records execution note after provider response normalization".into()],
            ),
            framework_tool(
                "digest.finalize",
                "closure compression and continuity carry-over",
                "compress the closure result into continuity tail, summary, and artifact candidates",
                vec!["a closure reaches a stable stop and continuity must roll forward".into()],
                vec!["the turn was interrupted and not closure-complete".into()],
                "closure result bundle".into(),
                "digest artifact for history + future context rebuild".into(),
                vec!["writes digest artifact and continuity tail".into()],
                vec!["framework finalizes digest only on successful closure stop".into()],
            ),
        ],
        tool_selection_policy: vec![
            "only model_tools are eligible for model-selected tool use".into(),
            "peer tools are currently contract-frozen and placeholder-only until runtime dispatch is wired"
                .into(),
            "disabled_tools are known tool families in fin design, but they are not callable in the current runtime"
                .into(),
            "if expected wait exceeds 1 minute, prefer wait.remind instead of busy waiting".into(),
            "do not rely on provider finish_reason for closure; use reasoning.stop when the turn should end".into(),
            "when editing files, prefer apply_patch replace mode for one bounded exact change; use patch mode only for multi-file or add/delete/move edits".into(),
            "framework_tools are runtime-owned capabilities and must not be hallucinated as direct tool calls"
                .into(),
            "if model_tools is empty, answer directly using current context and do not fabricate tool execution"
                .into(),
        ],
        disabled_tools: vec![
            "direct_fs_write".into(),
            "direct_channel_render".into(),
            "runtime_fact_mutation".into(),
        ],
        hard_guards: vec![
            "session artifacts are the only channel render truth".into(),
            "events are the only runtime fact truth".into(),
            "framework writes session files before UI consumes them".into(),
            "peer tool outputs may expose placeholder truth in M1; do not claim remote handshake success from them"
                .into(),
        ],
    }
}

fn framework_tool(
    name: &str,
    summary: &str,
    purpose: &str,
    when_to_use: Vec<String>,
    when_not_to_use: Vec<String>,
    input_schema_summary: String,
    output_schema_summary: String,
    side_effects: Vec<String>,
    example_uses: Vec<String>,
) -> ToolCatalogEntry {
    ToolCatalogEntry {
        tool_name: name.into(),
        kind: "framework_capability".into(),
        summary: summary.into(),
        purpose: purpose.into(),
        when_to_use,
        when_not_to_use,
        input_schema_summary,
        output_schema_summary,
        side_effects,
        example_uses,
    }
}

pub(super) fn model_tool(
    name: &str,
    summary: &str,
    purpose: &str,
    when_to_use: Vec<String>,
    when_not_to_use: Vec<String>,
    input_schema_summary: String,
    output_schema_summary: String,
    side_effects: Vec<String>,
    example_uses: Vec<String>,
) -> ToolCatalogEntry {
    ToolCatalogEntry {
        tool_name: name.into(),
        kind: "agent_tool".into(),
        summary: summary.into(),
        purpose: purpose.into(),
        when_to_use,
        when_not_to_use,
        input_schema_summary,
        output_schema_summary,
        side_effects,
        example_uses,
    }
}
