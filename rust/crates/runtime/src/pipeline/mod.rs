//! Pipeline domain: input / reason / feedback / error pipelines.
//!
//! Re-exports the four pipeline modules under a single `pipeline` namespace.
//! Owning layer: runtime. Cross-domain helpers are not allowed.

#[path = "error_pipeline.rs"]
pub mod error;
#[cfg(test)]
#[path = "error_pipeline_static_tests.rs"]
mod error_static_tests;

#[path = "input_pipeline.rs"]
pub mod input;
#[cfg(test)]
#[path = "input_pipeline_static_tests.rs"]
mod input_static_tests;

#[path = "reason_pipeline.rs"]
pub mod reason;
#[cfg(test)]
#[path = "reason_pipeline_static_tests.rs"]
mod reason_static_tests;

#[path = "feedback_pipeline.rs"]
pub mod feedback;
#[cfg(test)]
#[path = "feedback_pipeline_static_tests.rs"]
mod feedback_static_tests;
#[cfg(test)]
#[path = "naming_static_tests.rs"]
mod naming_static_tests;
