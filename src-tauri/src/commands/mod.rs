//! Tauri IPC command handlers.
//!
//! Sub-modules are grouped by concern:
//! - [`file`]       — open model, save / load / new project, export G-code
//! - [`operations`] — machining operation CRUD and reorder
//! - [`project`]    — lightweight project state queries
//! - [`stock`]      — stock definition and WCS get/set
//! - [`toolpath`]   — toolpath queries and post-processor management
//! - [`tools`]      — tool library CRUD

pub mod file;
pub mod operations;
pub mod project;
pub mod stock;
pub mod toolpath;
pub mod tools;

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use uuid::Uuid;

use crate::error::AppError;
use crate::postprocessor::ToolInfo;
use crate::state::Project;
use crate::toolpath::Toolpath;

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

/// Build [`ToolInfo`] entries for each toolpath by cross-referencing project
/// operations and tools.
///
/// For each toolpath, finds the matching [`crate::models::Operation`] (by `operation_id`) and
/// then the matching [`crate::models::Tool`] (by `tool_id`). If either lookup misses, the
/// toolpath is silently skipped — the assembler uses fallback values.
pub(super) fn build_tool_infos(toolpaths: &[Toolpath], project: &Project) -> Vec<ToolInfo> {
    toolpaths
        .iter()
        .filter_map(|tp| {
            let op = project
                .operations
                .iter()
                .find(|op| op.id == tp.operation_id)?;
            let tool = project.tools.iter().find(|t| t.id == op.tool_id)?;
            Some(ToolInfo {
                number: tp.tool_number,
                diameter: tool.diameter,
                description: tool.name.clone(),
            })
        })
        .collect()
}
