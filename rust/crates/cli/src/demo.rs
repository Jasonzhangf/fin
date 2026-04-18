use crate::CliError;
use crate::time::local_timestamp_now;
use fin_config::SystemConfig;
use fin_contracts::{DigestRecord, EntityRefs};
use fin_provider::InferenceProvider;
use fin_runtime::{
    ClosureRun, ContextAssemblyInput, ContextViewBuilder, InferenceOperationBuilder,
    InferenceRequest, M1Runtime, WorkerRuntime,
};
use std::{env, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DemoIdentity {
    pub(crate) operation_id: String,
    pub(crate) trace_id: String,
    pub(crate) session_id: String,
    pub(crate) task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DemoRequest {
    pub(crate) operation_id: String,
    pub(crate) trace_id: String,
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) input: String,
    pub(crate) recent_messages: Vec<String>,
    pub(crate) recent_digests: Vec<DigestRecord>,
    pub(crate) recent_reasoning_views: Vec<fin_contracts::ReasoningViewRecord>,
    pub(crate) recent_tool_records: Vec<fin_contracts::ToolExecutionRecord>,
    pub(crate) project_label: Option<String>,
    pub(crate) runtime_home: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) selected_paths: Vec<String>,
    pub(crate) submitted_at: String,
}

pub(crate) fn run_demo(
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    input: &str,
) -> Result<ClosureRun, CliError> {
    let demo_ids = demo_identity(demo_namespace_from_env().as_deref());
    run_demo_request(
        system,
        provider,
        DemoRequest {
            operation_id: demo_ids.operation_id,
            trace_id: demo_ids.trace_id,
            session_id: demo_ids.session_id,
            task_id: demo_ids.task_id,
            input: input.to_string(),
            recent_messages: Vec::new(),
            recent_digests: Vec::new(),
            recent_reasoning_views: Vec::new(),
            recent_tool_records: Vec::new(),
            project_label: Some("fin".into()),
            runtime_home: runtime_home_override_from_env().map(|path| path.display().to_string()),
            cwd: env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            selected_paths: Vec::new(),
            submitted_at: local_timestamp_now(),
        },
    )
}

pub(crate) fn run_demo_request(
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    request: DemoRequest,
) -> Result<ClosureRun, CliError> {
    let mut runtime = M1Runtime::default();
    let worker =
        WorkerRuntime::from_system(system, "agent-cli-demo", "worker-cli-demo", "cli", None)?;
    let refs = EntityRefs {
        session_id: Some(request.session_id.clone()),
        task_id: Some(request.task_id.clone()),
        worker_id: Some(worker.worker_id.clone()),
        ..EntityRefs::default()
    };
    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: request.operation_id.clone(),
            trace_id: request.trace_id.clone(),
            refs: refs.clone(),
            input: request.input.clone(),
            source: "cli.user".into(),
            recent_messages: request.recent_messages,
            recent_digests: request.recent_digests,
            recent_reasoning_views: request.recent_reasoning_views,
            recent_tool_records: request.recent_tool_records,
            project_label: request.project_label,
            runtime_home: request.runtime_home,
            cwd: request.cwd,
            selected_paths: request.selected_paths,
        },
    );
    let operation = InferenceOperationBuilder.build(
        &worker,
        InferenceRequest {
            operation_id: request.operation_id,
            trace_id: request.trace_id,
            submitted_at: request.submitted_at,
            refs,
            input: request.input,
            context,
        },
    )?;
    Ok(runtime.run_closure(operation, provider)?)
}

pub(crate) fn runtime_home_override_from_env() -> Option<PathBuf> {
    env::var("FIN_RUNTIME_HOME_OVERRIDE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

pub(crate) fn demo_namespace_from_env() -> Option<String> {
    env::var("FIN_SESSION_NAMESPACE")
        .ok()
        .map(|raw| sanitize_id_fragment(&raw))
        .filter(|value| !value.is_empty())
}

pub(crate) fn demo_identity(namespace: Option<&str>) -> DemoIdentity {
    let scope = namespace.unwrap_or("cli-demo");
    DemoIdentity {
        operation_id: format!("op-{scope}"),
        trace_id: format!("trace-{scope}"),
        session_id: format!("session-{scope}"),
        task_id: format!("task-{scope}"),
    }
}

pub(crate) fn sanitize_id_fragment(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
