//! Tauri IPC command handlers.
//!
//! Sub-modules are grouped by concern:
//! - [`file`]       — open model, save / load / new project
//! - [`operations`] — machining operation CRUD and reorder
//! - [`project`]    — lightweight project state queries
//! - [`stock`]      — stock definition and WCS get/set
//! - [`tools`]      — tool library CRUD

pub mod file;
pub mod operations;
pub mod project;
pub mod stock;
pub mod tools;
