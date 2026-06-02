use crate::{
    CliError, project_runtime_resume::drive_ready_project_runtime_resumes,
    web_debug::CliDebugActionHandler,
};
use fin_config::SystemConfig;
use fin_provider::InferenceProvider;
use std::path::Path;

pub(crate) fn drive_headless_project_runtime_resumes(
    runtime_home: &Path,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    handler: &CliDebugActionHandler,
    executed_at: &str,
) -> Result<usize, CliError> {
    let report = drive_ready_project_runtime_resumes(
        runtime_home,
        system,
        "daemon_headless_project_resume",
        executed_at,
        |binding, agent_name, message, source, attachments, merge_segment| {
            handler.run_project_turn_with_provider(
                runtime_home,
                binding,
                message,
                &source,
                attachments,
                provider,
                merge_segment,
                agent_name.as_deref(),
            )
        },
    )?;
    Ok(report.drove_count)
}
