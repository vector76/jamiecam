//! Tauri IPC command handlers.
//!
//! Sub-modules are grouped by concern:
//! - [`file`]    — open model, save / load / new project
//! - [`project`] — lightweight project state queries
//! - [`stock`]   — stock definition and WCS get/set
//! - [`tools`]   — tool library CRUD

pub mod file;
pub mod project;
pub mod stock;
pub mod tools;
