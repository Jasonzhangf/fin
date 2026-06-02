use crate::{
    WorkerRuntime,
    skill_loader::{LoadedSkill, load_global_skills, summarize_loaded_skills},
};
use fin_contracts::{DigestRecord, PromptLayerSummary, PromptModuleEntry, RolePromptBlock};

use super::context_view::ContextAssemblyInput;

pub(super) fn build_role_prompt_block(
    worker: &WorkerRuntime,
    recent_digests: &[DigestRecord],
    input: &ContextAssemblyInput,
) -> RolePromptBlock {
    let loaded_skills = load_global_skills(input.runtime_home.as_deref());
    let skill_summary = summarize_loaded_skills(&loaded_skills, 6);
    let role_id = worker.policy.role.role_id.as_str();
    let role_modules = role_baseline_modules(role_id);
    let stable_core_modules = stable_core_modules(!loaded_skills.is_empty(), &skill_summary);
    let prompt_layers = prompt_layers(
        &stable_core_modules,
        &role_modules,
        !loaded_skills.is_empty(),
        role_id,
    );

    let mut prompt_modules = Vec::new();
    prompt_modules.extend(stable_core_modules);
    prompt_modules.extend(role_modules);

    let mut prompt_lineage = vec![
        "stable core: fin agent-first framework truth + execution discipline".into(),
        format!("role baseline: {role_id}"),
        "request framing: framework-routed request, not direct user-model chat".into(),
    ];
    if !loaded_skills.is_empty() {
        prompt_lineage.push(format!("loaded global skill index: {skill_summary}"));
    }

    RolePromptBlock {
        role_id: role_id.to_string(),
        current_prompt_summary: current_prompt_summary(role_id, !loaded_skills.is_empty()),
        prompt_history: recent_digests
            .iter()
            .map(|digest| format!("continuity carried from {}", digest.summary))
            .collect(),
        prompt_lineage,
        prompt_modules,
        prompt_layers,
        behavior_rules: behavior_rules(role_id, &loaded_skills),
        output_contract: output_contract(role_id),
    }
}

fn prompt_layers(
    stable_core_modules: &[PromptModuleEntry],
    role_modules: &[PromptModuleEntry],
    has_skills: bool,
    role_id: &str,
) -> Vec<PromptLayerSummary> {
    vec![
        prompt_layer(
            "stable_core",
            "Stable Core",
            if has_skills {
                "agent-first framework truth, execution discipline, tool discipline, memory discipline, output discipline, and loaded global skill index"
            } else {
                "agent-first framework truth, execution discipline, tool discipline, memory discipline, and output discipline"
            }
            .into(),
            "stable_core",
            stable_core_modules,
        ),
        prompt_layer(
            "role_baseline",
            "Role Baseline",
            format!("stable owner/dispatcher/reviewer behavior modules for {role_id}"),
            "role_profile",
            role_modules,
        ),
    ]
}

fn current_prompt_summary(role_id: &str, has_skills: bool) -> String {
    let mut parts = vec![
        "agent-first prompt active".to_string(),
        format!("role={role_id}"),
        "request=framework-routed".into(),
    ];
    if has_skills {
        parts.push("global-skill-index=loaded".into());
    }
    format!(
        "{}; outputs must stay aligned with fin framework truth and role ownership boundaries",
        parts.join(", ")
    )
}

fn stable_core_modules(has_skills: bool, skill_summary: &str) -> Vec<PromptModuleEntry> {
    let mut modules = vec![
        prompt_module(
            "stable_core.agent_identity_and_runtime_position",
            "Stable Core / Agent Identity",
            "operate as a fin agent inside framework-managed session, event, note, digest, and projection pipelines".into(),
            "stable_core",
            120,
        ),
        prompt_module(
            "stable_core.framework_truth_rules",
            "Stable Core / Framework Truth",
            "session artifacts are render truth; runtime events are fact truth; do not invent missing state".into(),
            "stable_core",
            118,
        ),
        prompt_module(
            "stable_core.request_framing",
            "Stable Core / Request Framing",
            "treat current input as a framework-routed request or work item, not as direct user-model chat".into(),
            "stable_core",
            117,
        ),
        prompt_module(
            "stable_core.execution_discipline",
            "Stable Core / Execution",
            "verify before concluding; no silent failure; no destructive action without explicit authorization".into(),
            "stable_core",
            116,
        ),
        prompt_module(
            "stable_core.tool_usage_discipline",
            "Stable Core / Tool Usage",
            "distinguish model tools from framework capabilities and never fabricate tool calls".into(),
            "stable_core",
            114,
        ),
        prompt_module(
            "stable_core.memory_and_session_discipline",
            "Stable Core / Memory",
            "durable memory is for stable facts; continuity is rebuilt from digests, history, and knowledge artifacts".into(),
            "stable_core",
            112,
        ),
        prompt_module(
            "stable_core.output_discipline",
            "Stable Core / Output",
            "answer directly when grounded; distinguish facts, inference, and missing information".into(),
            "stable_core",
            110,
        ),
    ];
    if has_skills {
        modules.push(prompt_module(
            "stable_core.loaded_global_skill_index",
            "Stable Core / Global Skills",
            format!("{skill_summary}; use loaded global skills as reusable guidance, not as project truth"),
            "runtime_home.skills",
            108,
        ));
    }
    modules
}

fn role_baseline_modules(role_id: &str) -> Vec<PromptModuleEntry> {
    match role_id {
        "system" => vec![
            prompt_module(
                "role.system.identity",
                "System Role / Identity",
                "the only user-facing entry and frontstage coordinator; act as leader, dispatcher, reviewer, and orchestrator".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.system.framework_state_first",
                "System Role / Framework State First",
                "before routing, dispatch, reprioritization, or recovery, inspect framework-owned task board, agent presence, supervision, and peer state instead of inferring control state from chat text alone".into(),
                "role_profile",
                99,
            ),
            prompt_module(
                "role.system.backlog_first",
                "System Role / Backlog First",
                "inspect the current backlog/task board before reacting to a new request in isolation".into(),
                "role_profile",
                98,
            ),
            prompt_module(
                "role.system.dispatch_and_priority",
                "System Role / Dispatch And Priority",
                "compare new work against active/waiting/blocked/ready tasks; high-priority work should be minimally analyzed and dispatched quickly".into(),
                "role_profile",
                96,
            ),
            prompt_module(
                "role.system.review_and_unblock",
                "System Role / Review And Unblock",
                "treat completion as a scheduling signal; review unblock impact, reprioritize, and route the next owner".into(),
                "role_profile",
                94,
            ),
            prompt_module(
                "role.system.direct_execution_budget",
                "System Role / Direct Execution Budget",
                "simple work may be handled directly, but 2-3 closures without clear closure must escalate into plan plus delegation".into(),
                "role_profile",
                92,
            ),
        ],
        _ => vec![
            prompt_module(
                "role.project.identity",
                "Project Role / Identity",
                "own continuous progress inside a single project as project-scoped owner, dispatcher, reviewer, and delivery manager".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.project.task_board_first",
                "Project Role / Task Board First",
                "inspect the current project task board before choosing the next action".into(),
                "role_profile",
                98,
            ),
            prompt_module(
                "role.project.dispatch",
                "Project Role / Dispatch",
                "dispatch ready and unblocked tasks to workers when resources allow instead of hoarding all execution locally".into(),
                "role_profile",
                96,
            ),
            prompt_module(
                "role.project.review",
                "Project Role / Review",
                "as the task owner, review submitted work before marking progress complete or reopening execution".into(),
                "role_profile",
                94,
            ),
            prompt_module(
                "role.project.scope_discipline",
                "Project Role / Scope Discipline",
                "use project rules, selected paths, and current scope to keep work bounded toward minimal verified closure".into(),
                "role_profile",
                92,
            ),
        ],
    }
}

fn behavior_rules(role_id: &str, loaded_skills: &[LoadedSkill]) -> Vec<String> {
    let mut rules = vec![
        "respect session artifacts as render truth".into(),
        "treat events as runtime fact truth".into(),
        "treat the current input as a framework-routed request, not as direct user-model chat"
            .into(),
        "keep outputs structured for note, digest, and projection recording".into(),
        "do not treat framework-owned capabilities as model-selected tools".into(),
        "if expected wait exceeds 1 minute, use wait.remind with wait_minutes + reminder instead of busy waiting".into(),
        "when the turn should terminate, call reasoning.stop; do not rely on provider finish_reason for closure".into(),
        "when a bounded file edit is needed, call apply_patch instead of only describing the patch".into(),
        "prefer apply_patch replace mode for one exact change; for creating a new file use replace mode with old_string=\"\"; use patch mode only for multi-file or add/delete/move edits".into(),
    ];
    match role_id {
        "system" => rules.extend([
            "inspect the current backlog/task board before treating a new request in isolation".into(),
            "before routing, dispatch, reprioritization, or recovery, inspect framework-owned task board, agent presence, supervision, and peer state instead of inferring control state from chat text alone".into(),
            "when work is complex enough for managed execution, create/update task truth first, then dispatch or review through the task system instead of relying on chat memory alone".into(),
            "prefer dispatch, review, unblock analysis, and coordinated reporting over long single-slice execution".into(),
            "if direct work does not show clear closure after 2-3 closures, escalate into plan plus delegation".into(),
        ]),
        _ => rules.extend([
            "inspect the current project task board before selecting the next execution step".into(),
            "for managed work, use task-system truth to claim, submit, and review progress instead of free-form status only".into(),
            "prefer dispatching ready and unblocked tasks to workers when resources allow".into(),
            "prefer minimal project-scoped closure and align changes with owning-layer boundaries".into(),
        ]),
    }
    if !loaded_skills.is_empty() {
        rules.push(
            "prefer matching loaded global skills before inventing a new reusable workflow rule"
                .into(),
        );
    }
    rules
}

fn output_contract(role_id: &str) -> Vec<String> {
    let mut contract = vec![
        "answer the current request directly when no model tools are available".into(),
        "do not claim framework internals as model-selected tools".into(),
        "keep wording consistent with session continuity and current project scope".into(),
        "output exactly two top-level blocks and no extra prose before or after them".into(),
        "when a model tool call is needed, append an optional third block <fin_tool_calls>...</fin_tool_calls> after the two mandatory blocks".into(),
        "the fin_tool_calls block must be JSON (object or array) using provider-style {\"name\":\"...\",\"arguments\":{...}} items; runtime still accepts tool_name as a legacy alias".into(),
        "when the turn is complete, include reasoning.stop in fin_tool_calls; runtime closure tracks this signal instead of provider finish_reason".into(),
        "when a bounded edit is required, prefer apply_patch with {path, old_string, new_string}; for a new file set old_string=\"\"; use mode=patch only for multi-file or add/delete/move edits".into(),
        "when emitting structured control feedback, wrap the agent-visible answer in <fin_user_response>...</fin_user_response>".into(),
        "when emitting structured control feedback, wrap a JSON ControlFeedback object in <fin_control_feedback>...</fin_control_feedback>".into(),
        "the JSON ControlFeedback object must use fin keys such as is_continuation, is_simple_query, continuity_confidence, topic_shift_confidence, simple_query_confidence, current_topic_summary, note_candidate, digest_candidate, and reason".into(),
        "do not emit extra control-feedback keys outside the fin whitelist; unknown project/reporting keys belong in the user response, not in the control JSON".into(),
        "use JSON booleans for is_continuation/is_simple_query and integer confidences in the range 0-100; do not emit 0.18/1.0 style fractional confidence values".into(),
        "common failures forbidden: fractional confidences like 0.98/1.0, quoted booleans such as \"true\", or extra project/reporting keys in the control JSON".into(),
        "copy the fin control JSON key set exactly and replace only values; keep origin as model_output_contract_v1 when you follow the exact schema".into(),
        format!(
            "exact control feedback JSON shape example: {}",
            exact_control_feedback_schema_example()
        ),
    ];
    match role_id {
        "system" => contract.extend([
            "highlight orchestration judgment, backlog/task-board changes, and whether dispatch, reprioritization, or recovery is needed".into(),
            "state which task is in focus, which tasks were unblocked or remain blocked, and who owns the next action".into(),
        ]),
        _ => contract.extend([
            "highlight current project scope, epic/task progress, review outcome, and the next verify or delivery step".into(),
            "state what changed, what remains blocked, and whether docs, skills, tests, or runtime wiring still lag behind".into(),
        ]),
    }
    contract
}

pub(crate) fn exact_control_feedback_schema_example() -> &'static str {
    r#"{"origin":"model_output_contract_v1","is_continuation":false,"is_simple_query":false,"candidate_task_id":null,"candidate_topic_thread_id":null,"continuity_confidence":24,"topic_shift_confidence":76,"simple_query_confidence":12,"previous_topic_summary":"short previous topic summary","current_topic_summary":"short current topic summary","note_candidate":"one concise durable note","digest_candidate":"one concise closure digest","reason":"short routing/control reason"}"#
}

pub(crate) fn mandatory_response_format_lines() -> Vec<String> {
    vec![
        "output exactly two top-level blocks and no extra prose before or after them".into(),
        "emit <fin_user_response>...</fin_user_response> first and <fin_control_feedback>...</fin_control_feedback> second".into(),
        "if and only if a model tool call is needed, append <fin_tool_calls>...</fin_tool_calls> as an optional third block".into(),
        "fin_tool_calls JSON should use provider-style name + arguments; runtime still accepts legacy tool_name as an alias. For waits longer than 1 minute prefer wait.remind".into(),
        "for a single exact file edit, prefer apply_patch replace mode with path + old_string + new_string; for a new file set old_string=\"\"; reserve mode=patch for multi-file/add/delete/move edits".into(),
        "if the turn should end now, include reasoning.stop in fin_tool_calls".into(),
        "the control JSON must stay within the fin whitelist; do not add project/reporting/debug keys".into(),
        "confidence fields must be integers in the range 0-100; convert 0.98 -> 98 and 1.0 -> 100 before output".into(),
        "use JSON booleans true/false, not quoted strings such as \"true\" or \"false\"".into(),
        "copy the exact JSON key set and replace only values".into(),
    ]
}

fn prompt_layer(
    layer_id: &str,
    title: &str,
    summary: String,
    source: &str,
    modules: &[PromptModuleEntry],
) -> PromptLayerSummary {
    PromptLayerSummary {
        layer_id: layer_id.into(),
        title: title.into(),
        summary,
        source: source.into(),
        module_ids: modules
            .iter()
            .map(|module| module.module_id.clone())
            .collect(),
    }
}

fn prompt_module(
    module_id: &str,
    title: &str,
    summary: String,
    source: &str,
    priority: u8,
) -> PromptModuleEntry {
    PromptModuleEntry {
        module_id: module_id.into(),
        title: title.into(),
        summary,
        source: source.into(),
        priority,
    }
}
