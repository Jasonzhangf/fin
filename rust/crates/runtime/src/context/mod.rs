//! Context domain: minimal context view rendering.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
pub use crate::*;

pub mod block_render;
pub mod blocks;
pub mod project_support;
pub mod view;

#[cfg(test)]
mod view_registry_tests;
#[cfg(test)]
mod view_task_board_tests;
#[cfg(test)]
pub(super) mod view_tests;
#[cfg(test)]
mod view_tests_peer_state;
#[cfg(test)]
mod view_tests_rich_blocks;
