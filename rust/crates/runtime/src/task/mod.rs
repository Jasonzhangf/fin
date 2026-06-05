//! Task domain: project task board, managed tasks, handoffs, queue.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
pub mod assignment_queue;
pub mod board_snapshot;
pub mod handoff;
pub mod managed_board;
pub mod store;
