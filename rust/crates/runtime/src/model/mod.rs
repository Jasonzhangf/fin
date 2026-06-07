//! Model domain: model output parsing + model input assembly.
//! Owning layer: runtime.
pub mod input_assembler;
#[cfg(test)]
mod input_assembler_tests;
pub mod parser;
pub mod shapes;
#[cfg(test)]
mod tests;
