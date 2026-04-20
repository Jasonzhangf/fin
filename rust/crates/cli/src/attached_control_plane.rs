use crate::{
    CliError,
    daemon_state::{DaemonStateOutcome, refresh_attached_daemon_state},
    execution_state::clear_waiting_if_due,
    project_runtime_resume::{ProjectRuntimeResumeReport, drive_ready_project_runtime_resumes},
    reminder_scheduler::inject_due_reminders,
    startup_wakeup::refresh_startup_control_plane,
    supervisor_cycle::{SupervisorCycleOutcome, run_supervisor_cycle},
    supervisor_heartbeat::{SupervisorHeartbeatOutcome, run_supervisor_heartbeat},
};
use fin_config::SystemConfig;
use fin_contracts::{InputAttachmentSummary, InterruptedSegmentRecord};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct AttachedControlPlaneOutcome {
    #[allow(dead_code)]
    pub(crate) heartbeat: SupervisorHeartbeatOutcome,
    #[allow(dead_code)]
    pub(crate) reminder_cycle: Option<SupervisorCycleOutcome>,
    #[allow(dead_code)]
    pub(crate) project_resume: ProjectRuntimeResumeReport,
    #[allow(dead_code)]
    pub(crate) daemon_state: DaemonStateOutcome,
}

pub(crate) fn run_attached_control_plane_cycle<FE, FP>(
    runtime_home: &Path,
    system: &SystemConfig,
    binding: &DebugBinding,
    source: &str,
    mut run_entry_turn: FE,
    mut run_project_turn: FP,
) -> Result<AttachedControlPlaneOutcome, CliError>
where
    FE: FnMut(
        DebugBinding,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
    FP: FnMut(
        DebugBinding,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let heartbeat = run_supervisor_heartbeat(
        runtime_home,
        binding,
        source,
        system.runtime.heartbeat_interval_ms,
        &system.runtime.retention,
        system.runtime.retention.recent_routing_decision_limit,
        &mut run_entry_turn,
    )?;

    let fired = inject_due_reminders(runtime_home, binding)?;
    let reminder_cycle = if fired > 0 {
        clear_waiting_if_due(runtime_home, binding, &crate::time::local_timestamp_now())?;
        Some(run_supervisor_cycle(
            runtime_home,
            binding,
            "reminder_fired",
            system.runtime.heartbeat_interval_ms,
            &system.runtime.retention,
            system.runtime.retention.recent_routing_decision_limit,
            &mut run_entry_turn,
        )?)
    } else {
        None
    };

    let now = crate::time::local_timestamp_now();
    let _ = refresh_startup_control_plane(runtime_home, system, &now)?;
    let project_resume = drive_ready_project_runtime_resumes(
        runtime_home,
        system,
        "project_runtime_resume",
        &now,
        &mut run_project_turn,
    )?;
    let _ =
        refresh_startup_control_plane(runtime_home, system, &crate::time::local_timestamp_now())?;
    let daemon_state = refresh_attached_daemon_state(
        runtime_home,
        system,
        binding,
        source,
        &system.runtime.retention,
        system.runtime.retention.recent_routing_decision_limit,
    )?;

    Ok(AttachedControlPlaneOutcome {
        heartbeat,
        reminder_cycle,
        project_resume,
        daemon_state,
    })
}
