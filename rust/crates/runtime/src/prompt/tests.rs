//! Prompt domain shared test types + sub-module declarations.
//! Sub-modules use `use super::*;` to pick up the cross-domain re-exports.
use super::*;

#[path = "basics.rs"]
mod basics;
#[path = "catalog.rs"]
mod catalog;
#[path = "role_policy.rs"]
mod role_policy;
