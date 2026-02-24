//! Tauri IPC command handlers.
//!
//! Sub-modules are grouped by concern:
//! - [`file`]    — open model, save / load / new project
//! - [`project`] — lightweight project state queries

pub mod file;
pub mod project;
