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
    let model_name = worker.policy.provider_path.primary_target().model.as_str();
    let overlay_modules = model_overlay_modules(model_name);
    let role_modules = role_baseline_modules(worker.policy.role.role_id.as_str());
    let stable_core_modules = stable_core_modules(!loaded_skills.is_empty(), &skill_summary);
    let prompt_layers = prompt_layers(
        &stable_core_modules,
        &role_modules,
        &overlay_modules,
        !loaded_skills.is_empty(),
        worker.policy.role.role_id.as_str(),
        model_overlay_name(model_name),
    );

    let mut prompt_modules = Vec::new();
    prompt_modules.extend(stable_core_modules);
    prompt_modules.extend(role_modules);
    prompt_modules.extend(overlay_modules);

    let mut prompt_lineage = vec![
        "stable core: fin framework truth rules + execution discipline".into(),
        format!("role baseline: {}", worker.policy.role.role_id.as_str()),
        format!(
            "provider path: {}.{}",
            worker.policy.provider_path.primary_target().provider_name,
            worker.policy.provider_path.primary_target().model
        ),
    ];
    if !loaded_skills.is_empty() {
        prompt_lineage.push(format!("loaded global skill index: {skill_summary}"));
    }
    if let Some(overlay) = model_overlay_name(model_name) {
        prompt_lineage.push(format!("model overlay: {overlay}"));
    }

    RolePromptBlock {
        role_id: worker.policy.role.role_id.as_str().to_string(),
        current_prompt_summary: current_prompt_summary(
            worker.policy.role.role_id.as_str(),
            model_overlay_name(model_name),
            !loaded_skills.is_empty(),
        ),
        prompt_history: recent_digests
            .iter()
            .map(|digest| format!("continuity prompt carried from {}", digest.summary))
            .collect(),
        prompt_lineage,
        prompt_modules,
        prompt_layers,
        behavior_rules: behavior_rules(worker.policy.role.role_id.as_str(), &loaded_skills),
        output_contract: output_contract(worker.policy.role.role_id.as_str()),
    }
}

fn prompt_layers(
    stable_core_modules: &[PromptModuleEntry],
    role_modules: &[PromptModuleEntry],
    overlay_modules: &[PromptModuleEntry],
    has_skills: bool,
    role_id: &str,
    overlay_name: Option<&'static str>,
) -> Vec<PromptLayerSummary> {
    let mut layers = vec![
        prompt_layer(
            "stable_core",
            "Stable Core",
            if has_skills {
                "framework truth, execution discipline, tool discipline, memory discipline, output discipline, and loaded global skill index"
            } else {
                "framework truth, execution discipline, tool discipline, memory discipline, and output discipline"
            }
            .into(),
            "stable_core",
            stable_core_modules,
        ),
        prompt_layer(
            "role_baseline",
            "Role Baseline",
            format!("stable role behavior modules for {role_id}"),
            "role_profile",
            role_modules,
        ),
    ];
    if !overlay_modules.is_empty() {
        layers.push(prompt_layer(
            "model_overlay",
            "Model Overlay",
            format!(
                "{} behavior corrections for tool persistence, prerequisite checks, and verification",
                overlay_name.unwrap_or("model_overlay")
            ),
            "model_overlay",
            overlay_modules,
        ));
    }
    layers
}

fn current_prompt_summary(
    role_id: &str,
    overlay_name: Option<&'static str>,
    has_skills: bool,
) -> String {
    let mut parts = vec!["stable core active".to_string(), format!("role={role_id}")];
    if let Some(overlay_name) = overlay_name {
        parts.push(format!("overlay={overlay_name}"));
    }
    if has_skills {
        parts.push("global-skill-index=loaded".into());
    }
    format!(
        "{}; outputs must stay aligned with fin framework truth and structured prompt layers",
        parts.join(", ")
    )
}

fn stable_core_modules(has_skills: bool, skill_summary: &str) -> Vec<PromptModuleEntry> {
    let mut modules = vec![
        prompt_module(
            "stable_core.identity_and_runtime_position",
            "Stable Core / Identity",
            "operate inside the fin framework and respect framework ownership boundaries".into(),
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
                "role.system.purpose",
                "System Role / Purpose",
                "own multi-project orchestration, routing, health, recovery, and delegation".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.system.decision",
                "System Role / Decision",
                "prioritize routing, ownership, and coordination state before direct execution".into(),
                "role_profile",
                98,
            ),
        ],
        "worker" => vec![
            prompt_module(
                "role.worker.purpose",
                "Worker Role / Purpose",
                "execute a bounded slice accurately and return clear evidence and blockers".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.worker.decision",
                "Worker Role / Decision",
                "respect assigned boundaries and avoid broadening scope without permission".into(),
                "role_profile",
                98,
            ),
        ],
        "reviewer" | "analyzer" => vec![
            prompt_module(
                "role.reviewer.purpose",
                "Reviewer Role / Purpose",
                "review, diagnose, compare, and validate with findings-first output".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.reviewer.decision",
                "Reviewer Role / Decision",
                "separate confirmed findings, hypotheses, and open questions; prioritize regression and verification gaps".into(),
                "role_profile",
                98,
            ),
        ],
        _ => vec![
            prompt_module(
                "role.project.purpose",
                "Project Role / Purpose",
                "own continuous progress inside a single project and prefer minimal usable closure".into(),
                "role_profile",
                100,
            ),
            prompt_module(
                "role.project.decision",
                "Project Role / Decision",
                "keep work bounded by project scope, selected paths, and owning-layer rules".into(),
                "role_profile",
                98,
            ),
        ],
    }
}

fn model_overlay_modules(model_name: &str) -> Vec<PromptModuleEntry> {
    if is_gpt_codex_model(model_name) {
        vec![
            prompt_module(
                "overlay.gpt_codex.tool_persistence",
                "GPT/Codex Overlay / Tool Persistence",
                "do not stop early when another grounded step or tool-backed step is still needed".into(),
                "model_overlay",
                90,
            ),
            prompt_module(
                "overlay.gpt_codex.prerequisite_checks",
                "GPT/Codex Overlay / Prerequisites",
                "check discovery and upstream dependencies before acting on an obvious-looking final step".into(),
                "model_overlay",
                88,
            ),
            prompt_module(
                "overlay.gpt_codex.verification",
                "GPT/Codex Overlay / Verification",
                "before concluding, verify correctness, grounding, scope fit, and side effects".into(),
                "model_overlay",
                86,
            ),
        ]
    } else {
        Vec::new()
    }
}

fn model_overlay_name(model_name: &str) -> Option<&'static str> {
    is_gpt_codex_model(model_name).then_some("gpt_codex_v1")
}

fn is_gpt_codex_model(model_name: &str) -> bool {
    let lowered = model_name.to_ascii_lowercase();
    lowered.contains("gpt") || lowered.contains("codex")
}

fn behavior_rules(role_id: &str, loaded_skills: &[LoadedSkill]) -> Vec<String> {
    let mut rules = vec![
        "respect session artifacts as render truth".into(),
        "treat events as runtime fact truth".into(),
        "keep outputs structured for note, digest, and projection recording".into(),
        "do not treat framework-owned capabilities as model-selected tools".into(),
        "if expected wait exceeds 1 minute, use wait.remind with wait_minutes + reminder instead of busy waiting".into(),
        "when the turn should terminate, call reasoning.stop; do not rely on provider finish_reason for closure".into(),
        "when a bounded file edit is needed, call apply_patch instead of only describing the patch".into(),
        "prefer apply_patch replace mode for one exact change; use patch mode only for multi-file or add/delete/move edits".into(),
    ];
    match role_id {
        "system" => rules.push(
            "prioritize routing, recovery, and coordination clarity over long single-slice execution"
                .into(),
        ),
        "worker" => rules.push(
            "stay inside the assigned slice and surface blockers instead of expanding scope"
                .into(),
        ),
        "reviewer" | "analyzer" => rules.push(
            "lead with findings and distinguish confirmed evidence from hypotheses".into(),
        ),
        _ => rules.push(
            "prefer minimal project-scoped closure and align changes with owning-layer boundaries"
                .into(),
        ),
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
        "answer the current user turn directly when no model tools are available".into(),
        "do not claim framework internals as model-selected tools".into(),
        "keep wording consistent with session continuity and current project scope".into(),
        "output exactly two top-level blocks and no extra prose before or after them".into(),
        "when a model tool call is needed, append an optional third block <fin_tool_calls>...</fin_tool_calls> after the two mandatory blocks".into(),
        "the fin_tool_calls block must be JSON (object or array) using {\"tool_name\":\"...\",\"arguments\":{...}} items".into(),
        "when the turn is complete, include reasoning.stop in fin_tool_calls; runtime closure tracks this signal instead of provider finish_reason".into(),
        "when a bounded edit is required, prefer apply_patch with {path, old_string, new_string}; use mode=patch only for multi-file or add/delete/move edits".into(),
        "when emitting structured control feedback, wrap the user-visible answer in <fin_user_response>...</fin_user_response>".into(),
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
        "system" => contract.push(
            "highlight orchestration judgment, coordination risks, and whether delegation or recovery is needed"
                .into(),
        ),
        "worker" => contract.push(
            "highlight slice completion, blockers, and upstream handoff clarity".into(),
        ),
        "reviewer" | "analyzer" => contract.push(
            "lead with findings, then open questions or residual risks".into(),
        ),
        _ => contract.push(
            "highlight project scope, concrete change status, and the next verify step".into(),
        ),
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
        "fin_tool_calls JSON must use tool_name + arguments; for waits longer than 1 minute prefer wait.remind".into(),
        "for a single exact file edit, prefer apply_patch replace mode with path + old_string + new_string; reserve mode=patch for multi-file/add/delete/move edits".into(),
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
