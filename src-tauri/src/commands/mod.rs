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

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use uuid::Uuid;

use crate::error::AppError;
use crate::state::Project;

/// Parse a UUID from a string, returning [`AppError::NotFound`] if the string
/// is not a valid UUID.
///
/// The `entity` parameter names the resource kind for the error message
/// (e.g. `"tool"`, `"operation"`).
pub(super) fn parse_entity_id(id: &str, entity: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(id)
        .map_err(|_| AppError::NotFound(format!("{entity} id '{id}' is not a valid UUID")))
}

/// Acquire a write lock on `project_lock`, mapping a poisoned-lock failure to
/// [`AppError::Io`].
pub(super) fn write_project(
    project_lock: &RwLock<Project>,
) -> Result<RwLockWriteGuard<'_, Project>, AppError> {
    project_lock
        .write()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))
}

/// Acquire a read lock on `project_lock`, mapping a poisoned-lock failure to
/// [`AppError::Io`].
pub(super) fn read_project(
    project_lock: &RwLock<Project>,
) -> Result<RwLockReadGuard<'_, Project>, AppError> {
    project_lock
        .read()
        .map_err(|e| AppError::Io(format!("project lock poisoned: {e}")))
}
