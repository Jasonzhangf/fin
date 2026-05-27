mod agent_presence;
#[cfg(test)]
mod agent_presence_tests;
mod agent_registry_status;
mod assignment_runtime_resume;
mod attached_control_plane;
#[cfg(test)]
mod attached_control_plane_tests;
mod channel_peer;
mod channel_peer_activity_delivery;
mod channel_peer_activity_signature;
mod channel_peer_commands;
mod channel_peer_connectivity;
mod channel_peer_conversations;
#[cfg(test)]
mod channel_peer_conversations_tests;
mod channel_peer_qqbot_bridge;
mod channel_peer_store;
#[cfg(test)]
mod channel_peer_tests;
mod chat_policy;
mod cli;
mod command;
mod config;
mod control_boundary_scenario;
mod daemon_state;
mod daemon_state_support;
#[cfg(test)]
mod daemon_state_tests;
mod error;
mod execution_checkpoint;
mod execution_segments;
mod execution_state;
mod formalize_planning;
mod fs_utils;
mod headless_daemon;
mod headless_daemon_project_resume;
#[cfg(test)]
mod headless_daemon_tests;
mod install_flow;
mod install_smoke;
mod local_command_notice;
mod local_multi_agent_lifecycle_harness;
mod local_multi_agent_lifecycle_harness_support;
#[cfg(test)]
mod local_multi_agent_lifecycle_harness_tests;
mod local_multi_agent_lifecycle_harness_wait;
mod local_multi_agent_llm_task;
mod local_multi_agent_node;
mod local_multi_agent_rpc;
mod mainline_scenario;
mod process_utils;
mod project_agent_harness_commands;
mod project_execution_handoff;
mod project_recovery;
mod project_runtime_pickup;
#[cfg(test)]
mod project_runtime_pickup_tests;
mod project_runtime_resume;
#[cfg(test)]
mod project_runtime_resume_tests;
mod project_supervision;
mod provider_live_smoke;
mod qqbot_live_receipt;
mod reminder_scheduler;
mod routing_prompt_state;
mod runtime_current_snapshot;
mod runtime_home;
mod scheduler_driver;
#[cfg(test)]
mod scheduler_driver_tests;
mod scheduler_tick;
#[cfg(test)]
mod scheduler_tick_tests;
mod session_binding;
mod session_command_blocks;
mod session_commands;
mod session_ledger_read;
mod session_routing_commands;
mod session_run;
mod shared_io;
mod startup_control_summary;
mod startup_daemon_ensure;
mod startup_project_task_scan;
mod startup_topology;
mod startup_wakeup;
mod status_probe;
mod supervisor_cycle;
#[cfg(test)]
mod supervisor_cycle_tests;
mod supervisor_heartbeat;
#[cfg(test)]
mod supervisor_heartbeat_tests;
mod time;
mod transcript;
mod turn_ids;
mod versioning;
mod web_debug;
mod web_debug_entry;
mod web_debug_support;
#[cfg(test)]
mod web_debug_tests_status;
#[cfg(test)]
mod web_debug_tests_support;

#[cfg(test)]
mod test_env;
#[cfg(test)]
mod tests;

pub use cli::{run, run_with_runtime_home};
pub use error::CliError;
