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

use uuid::Uuid;

use crate::error::AppError;

/// Parse a UUID from a string, returning [`AppError::NotFound`] if the string
/// is not a valid UUID.
///
/// The `entity` parameter names the resource kind for the error message
/// (e.g. `"tool"`, `"operation"`).
pub(super) fn parse_entity_id(id: &str, entity: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id)
        .map_err(|_| AppError::NotFound(format!("{entity} id '{id}' is not a valid UUID")))
}
