//! Prompt domain: role prompt assembly and prompt test fixtures.
//! Owning layer: runtime.
pub mod assembly;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod basics;
#[cfg(test)]
mod catalog;
#[cfg(test)]
mod role_policy;
