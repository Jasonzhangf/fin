//! Tools domain: tool catalog + dispatch.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
//! All tool modules are co-located in this directory and re-exported
//! as `crate::tools::tool_X`.

pub(crate) mod catalog;
pub(crate) mod catalog_dynamic;
pub(crate) mod catalog_task_tools;

pub(crate) mod dispatch;
pub(crate) mod dispatch_assignment;
pub(crate) mod dispatch_control;
pub(crate) mod dispatch_control_args;

pub(crate) mod dispatch_collab;
pub(crate) mod dispatch_collab_coordination;
pub(crate) mod dispatch_collab_mailbox;
pub(crate) mod dispatch_exec;
pub(crate) mod dispatch_exec_receipts;
pub(crate) mod dispatch_patch;
pub(crate) mod dispatch_patch_apply;
pub(crate) mod dispatch_query;
pub(crate) mod dispatch_query_control;
pub(crate) mod dispatch_query_history;
pub(crate) mod dispatch_query_image;
pub(crate) mod dispatch_query_task;
pub(crate) mod dispatch_task_write;

pub(crate) mod dispatch_peer;
pub(crate) mod dispatch_peer_records;
pub(crate) mod dispatch_result_receipts;
pub(crate) mod history_render;
pub(crate) mod semantics;

#[cfg(test)]
mod dispatch_query_tests;
#[cfg(test)]
mod dispatch_task_write_tests;
#[cfg(test)]
mod dispatch_tests;
#[cfg(test)]
mod dispatch_tests_collab;
#[cfg(test)]
mod dispatch_tests_exec_patch;
#[cfg(test)]
mod dispatch_tests_stateful;
#[cfg(test)]
mod test_helpers;
