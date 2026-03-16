// src/policy/mod.rs
//
// Policy engine: command classification, file classification, and policy evaluation.
// Reads rules from workflow/*.yaml files and makes native Rust decisions.

pub mod types;
pub mod command;
pub mod file_class;
pub mod spec_loader;
pub mod evaluator;
