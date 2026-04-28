use crate::{
    CliError, assignment_runtime_resume::apply_explicit_assignment_resumes,
    web_debug::CliDebugActionHandler,
};
use fin_config::SystemConfig;
use fin_provider::InferenceProvider;
use std::path::Path;

pub(crate) fn drive_headless_assignment_resumes(
    runtime_home: &Path,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    handler: &CliDebugActionHandler,
    executed_at: &str,
) -> Result<usize, CliError> {
    let report = apply_explicit_assignment_resumes(
        runtime_home,
        "daemon_headless_assignment_resume_drive",
        executed_at,
        |binding, agent_name, message, source, attachments, merge_segment| {
            let role_id = agent_name.as_deref().unwrap_or("system");
            if role_id == "project" {
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
            } else {
                handler.run_chat_turn_with_provider(
                    runtime_home,
                    binding,
                    message,
                    &source,
                    attachments,
                    provider,
                    merge_segment,
                )
            }
        },
    )?;
    Ok(report.drove_count)
}
