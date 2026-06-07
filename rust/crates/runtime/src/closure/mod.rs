//! Closure domain: M1Runtime closure orchestration.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
//! All sub-modules live in this directory with short names.
//! closure_runtime.rs and closure_runtime_rounds_tools.rs are still at parent
//! level with #[path] bridges for incremental migration.

pub use crate::*;

mod checkpoint;
mod error_events;
mod error_pipeline;
mod events;
mod finalize;
mod records;
mod retry;
mod rounds;
mod state;

// Main closure runtime (includes its own #[path] sub-module declarations)
#[path = "../closure_runtime.rs"]
pub mod closure_runtime;

// Standalone sub-module not declared in closure_runtime.rs
#[path = "../closure_runtime_rounds_tools.rs"]
pub mod closure_runtime_rounds_tools;
