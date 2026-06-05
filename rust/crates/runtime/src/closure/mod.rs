//! Closure domain: M1Runtime closure orchestration.
//!
//! Owning layer: runtime. Cross-domain helpers are not allowed.
//! Bridge strategy: #[path] points to sibling files in ../ to keep Rust namespace stable
//! while we prepare for physical relocation.

// Re-export crate-root items so `use super::*` in sub-modules resolves to the crate root
pub use crate::*;

// Main closure runtime (includes its own #[path] sub-module declarations)
#[path = "../closure_runtime.rs"]
pub mod closure_runtime;

// Standalone sub-module not declared in closure_runtime.rs
#[path = "../closure_runtime_rounds_tools.rs"]
pub mod closure_runtime_rounds_tools;
