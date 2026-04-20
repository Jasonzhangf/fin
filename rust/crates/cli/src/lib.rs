mod agent_presence;
#[cfg(test)]
mod agent_presence_tests;
mod agent_registry_status;
mod channel_peer;
mod channel_peer_activity_delivery;
mod channel_peer_activity_signature;
mod channel_peer_commands;
mod channel_peer_connectivity;
mod channel_peer_conversations;
mod channel_peer_qqbot_bridge;
mod channel_peer_store;
#[cfg(test)]
mod channel_peer_tests;
mod chat_policy;
mod cli;
mod command;
mod config;
mod control_boundary_demo;
mod daemon_state;
mod daemon_state_support;
#[cfg(test)]
mod daemon_state_tests;
mod demo;
mod error;
mod execution_segments;
mod execution_state;
mod fs_utils;
mod install_flow;
mod install_smoke;
mod local_command_notice;
mod mainline_demo;
mod process_utils;
mod project_execution_handoff;
mod project_recovery;
mod project_runtime_pickup;
mod project_supervision;
mod reminder_scheduler;
mod runtime_home;
mod scheduler_driver;
#[cfg(test)]
mod scheduler_driver_tests;
mod scheduler_tick;
#[cfg(test)]
mod scheduler_tick_tests;
mod session_binding;
mod session_commands;
mod startup_control_summary;
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
mod tests;

pub use cli::{run, run_with_runtime_home};
pub use error::CliError;
