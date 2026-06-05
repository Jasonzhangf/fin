//! Tools domain: tool catalog + dispatch.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
//! All tool modules are co-located in this directory and re-exported
//! as `crate::tools::tool_X`.

pub(crate) mod tool_catalog;
pub(crate) mod tool_catalog_dynamic;
pub(crate) mod tool_catalog_task_tools;

pub(crate) mod tool_dispatch;
pub(crate) mod tool_dispatch_assignment;
pub(crate) mod tool_dispatch_control;
pub(crate) mod tool_dispatch_control_support;

pub(crate) mod tool_dispatch_extended;
pub(crate) mod tool_dispatch_extended_collab;
pub(crate) mod tool_dispatch_extended_collab_coordination;
pub(crate) mod tool_dispatch_extended_collab_mailbox;
pub(crate) mod tool_dispatch_extended_exec;
pub(crate) mod tool_dispatch_extended_exec_receipts;
pub(crate) mod tool_dispatch_extended_patch;
pub(crate) mod tool_dispatch_extended_patch_v4a;
pub(crate) mod tool_dispatch_extended_query;
pub(crate) mod tool_dispatch_extended_query_control;
pub(crate) mod tool_dispatch_extended_query_history;
pub(crate) mod tool_dispatch_extended_query_image;
pub(crate) mod tool_dispatch_extended_query_task;
pub(crate) mod tool_dispatch_extended_task_write;

pub(crate) mod tool_dispatch_peer;
pub(crate) mod tool_dispatch_peer_support;
pub(crate) mod tool_dispatch_result_receipts;
pub(crate) mod tool_history_render;
pub(crate) mod tool_semantics;

#[cfg(test)]
mod tool_dispatch_query_tests;
#[cfg(test)]
mod tool_dispatch_task_write_tests;
#[cfg(test)]
mod tool_dispatch_tests;
#[cfg(test)]
#[cfg(test)]
#[cfg(test)]
#[cfg(test)]
#[cfg(test)]
mod test_helpers;
